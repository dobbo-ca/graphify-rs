use super::*;
use std::time::{Duration, UNIX_EPOCH};

fn at(secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

// New: the AWS SigV4 test-suite timestamp.
#[test]
fn amz_date_vector() {
    let u = Utc::from_system(at(1_440_938_160));
    assert_eq!(
        u,
        Utc {
            year: 2015,
            month: 8,
            day: 30,
            hour: 12,
            min: 36,
            sec: 0,
        }
    );
    assert_eq!(u.amz_date(), "20150830T123600Z");
    assert_eq!(u.short_date(), "20150830");
}

// New: epoch, a leap day, and 2100-03-01 (2100 is not a leap year).
#[test]
fn civil_round_trip() {
    let cases = [
        (
            0u64,
            Utc {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                min: 0,
                sec: 0,
            },
        ),
        (
            951_782_400,
            Utc {
                year: 2000,
                month: 2,
                day: 29,
                hour: 0,
                min: 0,
                sec: 0,
            },
        ),
        (
            4_107_542_400,
            Utc {
                year: 2100,
                month: 3,
                day: 1,
                hour: 0,
                min: 0,
                sec: 0,
            },
        ),
    ];
    for (secs, want) in cases {
        let u = Utc::from_system(at(secs));
        assert_eq!(u, want, "from_system {secs}");
        assert_eq!(u.to_system(), at(secs), "to_system {secs}");
    }
}

// New: Go time.Parse(time.RFC3339, ...) behaviour.
#[test]
fn rfc3339_parse() {
    assert_eq!(
        parse_rfc3339("2014-02-27T00:00:00Z"),
        Some(at(1_393_459_200)),
        "Z"
    );
    // 23:00:00.123 at +02:00 is 21:00:00.123 UTC; keep whole seconds only.
    let got = parse_rfc3339("2009-11-10T23:00:00.123+02:00").expect("fractional with offset");
    let secs = got
        .duration_since(UNIX_EPOCH)
        .expect("after epoch")
        .as_secs();
    assert_eq!(secs, 1_257_886_800, "offset applied");
    assert_eq!(parse_rfc3339("2014-02-27"), None, "date only");
}

// New: Go time.Parse(time.RFC1123, ...) behaviour (requestconfig.go:299-310).
#[test]
fn rfc1123_parse() {
    let want = Some(at(1_136_214_245));
    assert_eq!(parse_rfc1123("Mon, 02 Jan 2006 15:04:05 GMT"), want, "GMT");
    assert_eq!(parse_rfc1123("Mon, 02 Jan 2006 15:04:05 UTC"), want, "UTC");
    assert_eq!(
        parse_rfc1123("Mon, 02 Jan 2006 15:04:05 EST"),
        want,
        "unknown 3-letter zone is UTC"
    );
    assert_eq!(
        parse_rfc1123("Mon, 02 Jan 2006 15:04:05 CEST"),
        want,
        "unknown 4-letter zone is UTC"
    );
    assert_eq!(
        parse_rfc1123("Monday, 02-Jan-06 15:04:05 GMT"),
        None,
        "RFC 850 form rejected"
    );
}
