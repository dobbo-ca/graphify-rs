//! UTC civil time from `SystemTime`, and the three formats the client needs: `X-Amz-Date`,
//! RFC 3339 (`credential_process` `Expiration`) and RFC 1123 (`Retry-After`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Utc {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
}

impl Utc {
    /// Whole seconds; sub-second parts are dropped. Times before 1970 are not needed.
    pub(crate) fn from_system(t: SystemTime) -> Utc {
        let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
        let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
        let rem = secs.rem_euclid(86_400) as u32;
        Utc {
            year,
            month,
            day,
            hour: rem / 3600,
            min: rem % 3600 / 60,
            sec: rem % 60,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn to_system(self) -> SystemTime {
        match u64::try_from(self.epoch_secs()) {
            Ok(s) => UNIX_EPOCH + Duration::from_secs(s),
            Err(_) => UNIX_EPOCH,
        }
    }

    fn epoch_secs(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.min) * 60
            + i64::from(self.sec)
    }

    /// `20150830T123600Z`.
    pub(crate) fn amz_date(&self) -> String {
        format!(
            "{}T{:02}{:02}{:02}Z",
            self.short_date(),
            self.hour,
            self.min,
            self.sec
        )
    }

    /// `20150830`.
    pub(crate) fn short_date(&self) -> String {
        format!("{:04}{:02}{:02}", self.year, self.month, self.day)
    }
}

// Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms".
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Fixed-width digits at `b[start..start + len]`.
fn num(b: &[u8], start: usize, len: usize) -> Option<u32> {
    let digits = b.get(start..start + len)?;
    digits.iter().try_fold(0u32, |acc, c| {
        c.is_ascii_digit().then(|| acc * 10 + u32::from(c - b'0'))
    })
}

/// Range checks as Go's `Parse` does (day within the month, hour < 24, min and sec < 60), then
/// shifts by the zone offset to UTC.
fn build(
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    off: i64,
) -> Option<SystemTime> {
    if !(1..=12).contains(&month) || hour >= 24 || min >= 60 || sec >= 60 {
        return None;
    }
    let dim = match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=dim).contains(&day) {
        return None;
    }
    let local = Utc {
        year,
        month,
        day,
        hour,
        min,
        sec,
    }
    .epoch_secs();
    // Go returns valid times before 1970 too.
    let secs = local - off;
    Some(if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs.unsigned_abs())
    } else {
        UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs())
    })
}

/// Go `time.Parse(time.RFC3339, s)`: `2006-01-02T15:04:05Z07:00`, fractional seconds allowed.
pub(crate) fn parse_rfc3339(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') || b.get(10) != Some(&b'T') {
        return None;
    }
    if b.get(13) != Some(&b':') || b.get(16) != Some(&b':') {
        return None;
    }
    let mut i = 19;
    // Go accepts a fraction (`.` or `,`, one or more digits) after the seconds even though the
    // layout has none (time/format.go, "Special case: do we have a fractional second").
    if matches!(b.get(i), Some(b'.' | b',')) {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let off = match b.get(i..)? {
        b"Z" => 0,
        [sign @ (b'+' | b'-'), rest @ ..] if rest.len() == 5 && rest[2] == b':' => {
            let hh = i64::from(num(rest, 0, 2)?);
            let mm = i64::from(num(rest, 3, 2)?);
            // Go: hour > 24 or minute > 60 is a range error.
            if hh > 24 || mm > 60 {
                return None;
            }
            let o = hh * 3600 + mm * 60;
            if *sign == b'-' { -o } else { o }
        }
        _ => return None,
    };
    build(
        i64::from(num(b, 0, 4)?),
        num(b, 5, 2)?,
        num(b, 8, 2)?,
        num(b, 11, 2)?,
        num(b, 14, 2)?,
        num(b, 17, 2)?,
        off,
    )
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Go `time.Parse(time.RFC1123, s)`: `Mon, 02 Jan 2006 15:04:05 MST`. Any zone abbreviation is
/// read as UTC (Go gives an unknown abbreviation offset 0).
pub(crate) fn parse_rfc1123(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    // Zone is 3 or 4 uppercase letters, so the whole string is 29 or 30 bytes.
    if !(29..=30).contains(&b.len()) {
        return None;
    }
    let name_at = |start: usize, names: &[&str]| {
        let w = b.get(start..start + 3)?;
        names
            .iter()
            .position(|n| n.as_bytes().eq_ignore_ascii_case(w))
    };
    // Go does not check the weekday against the date, only that it is a weekday name.
    name_at(0, &WEEKDAYS)?;
    let month = name_at(8, &MONTHS)? as u32 + 1;
    if &b[3..5] != b", " || b[7] != b' ' || b[11] != b' ' || b[16] != b' ' || b[19] != b':' {
        return None;
    }
    if b[22] != b':' || b[25] != b' ' || !b[26..].iter().all(u8::is_ascii_uppercase) {
        return None;
    }
    build(
        i64::from(num(b, 12, 4)?),
        month,
        num(b, 5, 2)?,
        num(b, 17, 2)?,
        num(b, 20, 2)?,
        num(b, 23, 2)?,
        0,
    )
}
