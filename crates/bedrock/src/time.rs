//! UTC civil time from `SystemTime`, and the three formats the client needs: `X-Amz-Date`,
//! RFC 3339 (`credential_process` `Expiration`) and RFC 1123 (`Retry-After`).

use std::time::SystemTime;

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
        todo!("graphify-rs-u2o.3")
    }

    pub(crate) fn to_system(self) -> SystemTime {
        todo!("graphify-rs-u2o.3")
    }

    /// `20150830T123600Z`.
    pub(crate) fn amz_date(&self) -> String {
        todo!("graphify-rs-u2o.3")
    }

    /// `20150830`.
    pub(crate) fn short_date(&self) -> String {
        todo!("graphify-rs-u2o.3")
    }
}

/// Go `time.Parse(time.RFC3339, s)`: `2006-01-02T15:04:05Z07:00`, fractional seconds allowed.
pub(crate) fn parse_rfc3339(s: &str) -> Option<SystemTime> {
    todo!("graphify-rs-u2o.3")
}

/// Go `time.Parse(time.RFC1123, s)`: `Mon, 02 Jan 2006 15:04:05 MST`. Any zone abbreviation is
/// read as UTC (Go gives an unknown abbreviation offset 0).
pub(crate) fn parse_rfc1123(s: &str) -> Option<SystemTime> {
    todo!("graphify-rs-u2o.3")
}
