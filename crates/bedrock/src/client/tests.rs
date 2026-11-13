use std::time::{Duration, UNIX_EPOCH};

use crate::Config;
use crate::client::{join_base, retry_delay, should_retry};
use crate::http::Response;

fn response(status: u16, headers: &[(&str, &str)]) -> Response {
    Response {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: Vec::new(),
    }
}

// Port of message_test.go:547 TestCalculateNonStreamingTimeout (anthropic-sdk-go v1.51.0),
// adapted: max_tokens is always 1024, so the timeout is the fixed 10 minutes the Go default
// gives (`Config::new`); the `X-Stainless-Timeout: 600` header half is
// tests/client.rs non_streaming_timeout_header.
#[test]
#[ignore = "graphify-rs-u2o.8"]
fn calculate_non_streaming_timeout() {
    let cfg = Config::new("us-east-1");
    assert_eq!(cfg.attempt_timeout, Duration::from_secs(600));
    assert_eq!(cfg.max_retries, 2);
    assert_eq!(cfg.region, "us-east-1");
}

// Port of bedrock/bedrockmantle_test.go:229 TestMantleBaseURLDerivedFromRegion
// (anthropic-sdk-go v1.51.0), adapted: this only joins a hard-coded URL. Region derivation
// itself is NOT tested here (no mock can receive a request for the derived host).
#[test]
#[ignore = "graphify-rs-u2o.8"]
fn mantle_base_url_derived_from_region() {
    assert_eq!(
        join_base("https://bedrock-mantle.us-west-2.api.aws/anthropic").as_deref(),
        Ok("https://bedrock-mantle.us-west-2.api.aws/anthropic/v1/messages")
    );
}

#[test]
#[ignore = "graphify-rs-u2o.8"]
fn base_url_join() {
    const CASES: &[(&str, &str)] = &[
        ("https://h/anthropic", "https://h/anthropic/v1/messages"),
        ("https://h/anthropic/", "https://h/anthropic/v1/messages"),
        ("https://h", "https://h/v1/messages"),
        ("https://h/", "https://h/v1/messages"),
        ("http://127.0.0.1:9/x", "http://127.0.0.1:9/x/v1/messages"),
    ];
    for (base, want) in CASES {
        assert_eq!(join_base(base).as_deref(), Ok(*want), "base {base}");
    }
}

#[test]
#[ignore = "graphify-rs-u2o.8"]
fn should_retry_table() {
    struct Case {
        name: &'static str,
        status: Option<u16>,
        headers: &'static [(&'static str, &'static str)],
        want: bool,
    }
    const CASES: &[Case] = &[
        Case {
            name: "connection error",
            status: None,
            headers: &[],
            want: true,
        },
        Case {
            name: "200",
            status: Some(200),
            headers: &[],
            want: false,
        },
        Case {
            name: "400",
            status: Some(400),
            headers: &[],
            want: false,
        },
        Case {
            name: "401",
            status: Some(401),
            headers: &[],
            want: false,
        },
        Case {
            name: "408",
            status: Some(408),
            headers: &[],
            want: true,
        },
        Case {
            name: "409",
            status: Some(409),
            headers: &[],
            want: true,
        },
        Case {
            name: "429",
            status: Some(429),
            headers: &[],
            want: true,
        },
        Case {
            name: "499",
            status: Some(499),
            headers: &[],
            want: false,
        },
        Case {
            name: "500",
            status: Some(500),
            headers: &[],
            want: true,
        },
        Case {
            name: "503",
            status: Some(503),
            headers: &[],
            want: true,
        },
        Case {
            name: "529",
            status: Some(529),
            headers: &[],
            want: true,
        },
        Case {
            name: "x-should-retry true on 400",
            status: Some(400),
            headers: &[("x-should-retry", "true")],
            want: true,
        },
        Case {
            name: "x-should-retry true on 200",
            status: Some(200),
            headers: &[("X-Should-Retry", "true")],
            want: true,
        },
        Case {
            name: "x-should-retry false on 503",
            status: Some(503),
            headers: &[("x-should-retry", "false")],
            want: false,
        },
        Case {
            name: "x-should-retry false on 429",
            status: Some(429),
            headers: &[("x-should-retry", "false")],
            want: false,
        },
        Case {
            name: "x-should-retry other value on 400",
            status: Some(400),
            headers: &[("x-should-retry", "maybe")],
            want: false,
        },
        Case {
            name: "x-should-retry other value on 503",
            status: Some(503),
            headers: &[("x-should-retry", "maybe")],
            want: true,
        },
    ];
    for case in CASES {
        let res = case.status.map(|status| response(status, case.headers));
        assert_eq!(should_retry(res.as_ref()), case.want, "{}", case.name);
    }
}

#[test]
#[ignore = "graphify-rs-u2o.8"]
fn retry_delay_table() {
    let now = UNIX_EPOCH + Duration::from_secs(1_136_214_243); // 2006-01-02 15:04:03 UTC
    let ms = Duration::from_millis;
    let ns = Duration::from_nanos;

    // No usable header: 0.5 s * 2^retry, capped at 8 s, minus jitter(delay / 4).
    // (retry, delay before jitter in ms)
    for (retry, base_ms) in [
        (0u32, 500u64),
        (1, 1000),
        (2, 2000),
        (3, 4000),
        (4, 8000),
        (5, 8000),
        (10, 8000),
    ] {
        let quarter = base_ms * 1_000_000 / 4;
        let mut seen = None;
        let d = retry_delay(None, retry, now, &mut |n| {
            seen = Some(n);
            0
        });
        assert_eq!(seen, Some(quarter), "retry {retry}: jitter bound");
        assert_eq!(d, ms(base_ms), "retry {retry}: no jitter");
        let d = retry_delay(None, retry, now, &mut |n| n - 1);
        assert_eq!(
            d,
            ms(base_ms) - ns(quarter - 1),
            "retry {retry}: max jitter"
        );
        // the plan's ranges: (0.75 * delay, delay]
        assert!(
            d > ms(base_ms * 3 / 4) && d <= ms(base_ms),
            "retry {retry}: {d:?}"
        );
    }
    // a 502 response without headers backs off the same way
    let res = response(502, &[]);
    assert_eq!(retry_delay(Some(&res), 1, now, &mut |_| 0), ms(1000));

    // Headers decide the delay and the jitter source is never asked.
    struct Case {
        name: &'static str,
        headers: &'static [(&'static str, &'static str)],
        now_offset_s: u64,
        want_ms: u64,
    }
    const CASES: &[Case] = &[
        Case {
            name: "Retry-After 0.1",
            headers: &[("Retry-After", "0.1")],
            now_offset_s: 0,
            want_ms: 100,
        },
        Case {
            name: "Retry-After 5",
            headers: &[("Retry-After", "5")],
            now_offset_s: 0,
            want_ms: 5000,
        },
        Case {
            name: "Retry-After-Ms 100",
            headers: &[("Retry-After-Ms", "100")],
            now_offset_s: 0,
            want_ms: 100,
        },
        Case {
            name: "Retry-After-Ms wins over Retry-After",
            headers: &[("Retry-After", "5"), ("Retry-After-Ms", "100")],
            now_offset_s: 0,
            want_ms: 100,
        },
        Case {
            name: "header names ignore case",
            headers: &[("retry-after-ms", "250")],
            now_offset_s: 0,
            want_ms: 250,
        },
        Case {
            name: "Retry-After -3",
            headers: &[("Retry-After", "-3")],
            now_offset_s: 0,
            want_ms: 0,
        },
        Case {
            name: "Retry-After-Ms -5",
            headers: &[("Retry-After-Ms", "-5")],
            now_offset_s: 0,
            want_ms: 0,
        },
        Case {
            name: "no upper cap",
            headers: &[("Retry-After", "600")],
            now_offset_s: 0,
            want_ms: 600_000,
        },
        Case {
            name: "RFC1123 date 2 s ahead",
            headers: &[("Retry-After", "Mon, 02 Jan 2006 15:04:05 GMT")],
            now_offset_s: 0,
            want_ms: 2000,
        },
        Case {
            name: "RFC1123 date in the past",
            headers: &[("Retry-After", "Mon, 02 Jan 2006 15:04:05 GMT")],
            now_offset_s: 10,
            want_ms: 0,
        },
        Case {
            name: "bad Retry-After-Ms falls to Retry-After",
            headers: &[("Retry-After-Ms", "soon"), ("Retry-After", "1")],
            now_offset_s: 0,
            want_ms: 1000,
        },
    ];
    for case in CASES {
        let res = response(429, case.headers);
        let at = now + Duration::from_secs(case.now_offset_s);
        let got = retry_delay(Some(&res), 0, at, &mut |_| {
            panic!("{}: jitter must not be used", case.name)
        });
        assert_eq!(got, ms(case.want_ms), "{}", case.name);
    }

    // An unparseable Retry-After is ignored: back to the backoff.
    let res = response(429, &[("Retry-After", "tomorrow")]);
    assert_eq!(
        retry_delay(Some(&res), 0, now, &mut |_| 0),
        ms(500),
        "unparseable Retry-After"
    );
}
