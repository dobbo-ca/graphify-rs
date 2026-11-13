//! Upstream graphify v0.9.107 `tests/test_security.py` and
//! `tests/test_frontmatter_and_ssrf_hardening.py`: the cases that hold for graphify-go, with
//! Go's error texts. Not ported (Go differs or has no such function): `safe_fetch*`,
//! `sanitize_metadata*`, `redact_secrets*`, embedded credentials, ports 0 and 65536 (Go's
//! `url.Parse` takes any digits), the accept cases (they need DNS for example.com), the default
//! graph base, `sanitize_label(None)`, the monkeypatched stat error, `239.1.1.1` (Go blocks only
//! link-local multicast), and the Obsidian/YAML helpers.

use std::net::IpAddr;

use crate::security::security::{check_size, is_blocked_ip, max_graph_file_bytes_from};
use crate::security::{MAX_GRAPH_FILE_BYTES, sanitize_label, validate_graph_path, validate_url};
use crate::testutil::TempDir;

fn err_text(r: Result<(), crate::security::Error>) -> String {
    r.expect_err("want an error").to_string()
}

#[test]
fn validate_url_rejects_schemes() {
    for (url, scheme) in [
        ("file:///etc/passwd", "file"),
        ("ftp://files.example.com/data.zip", "ftp"),
        ("data:text/html,<script>alert(1)</script>", "data"),
        ("//no-scheme.example.com", ""),
    ] {
        assert_eq!(
            err_text(validate_url(url)),
            format!("blocked URL scheme \"{scheme}\" - only http and https are allowed"),
            "{url}"
        );
    }
}

#[test]
fn validate_url_rejects_missing_hostname() {
    for url in [
        "http://",
        "https://",
        "http:///etc/passwd",
        "https:///example.com/path",
    ] {
        assert_eq!(
            err_text(validate_url(url)),
            format!("URL \"{url}\" has no host")
        );
    }
}

#[test]
fn validate_url_rejects_invalid_port() {
    let url = "http://example.com:abc/path";
    assert_eq!(
        err_text(validate_url(url)),
        format!("invalid URL \"{url}\": parse \"{url}\": invalid port \":abc\" after host")
    );
}

#[test]
fn validate_graph_path_cases() {
    let tmp = TempDir::new();
    let base = tmp.join("graphify-out");
    std::fs::create_dir(&base).unwrap();
    let base_s = base.to_str().unwrap();
    // allows inside base: the absolute path comes back
    let graph = tmp.write("graphify-out/graph.json", b"{}");
    assert_eq!(
        validate_graph_path(graph.to_str().unwrap(), base_s).unwrap(),
        graph.to_str().unwrap()
    );
    // blocks traversal
    let evil = format!("{base_s}/../etc_passwd");
    let e = validate_graph_path(&evil, base_s).unwrap_err().to_string();
    assert_eq!(
        e,
        format!("path \"{evil}\" escapes the allowed directory {base_s}")
    );
    // requires base to exist
    let missing_base = tmp.join("nope");
    let mb = missing_base.to_str().unwrap();
    let e = validate_graph_path(&format!("{mb}/graph.json"), mb)
        .unwrap_err()
        .to_string();
    assert_eq!(e, format!("graph base directory does not exist: {mb}"));
    // the file must exist
    let missing = format!("{base_s}/missing.json");
    let e = validate_graph_path(&missing, base_s)
        .unwrap_err()
        .to_string();
    assert_eq!(e, format!("graph file not found: {missing}"));
}

#[test]
fn sanitize_label_cases() {
    // no HTML escaping
    assert_eq!(sanitize_label("<script>"), "<script>");
    assert_eq!(sanitize_label("foo & bar"), "foo & bar");
    assert_eq!(sanitize_label("hello\x00\x1fworld"), "helloworld");
    assert_eq!(sanitize_label(&"a".repeat(300)).len(), 256);
    assert_eq!(sanitize_label("MyClass"), "MyClass");
    assert_eq!(sanitize_label("extract_python"), "extract_python");
}

#[test]
fn graph_size_cap_default_is_512_mib() {
    assert_eq!(MAX_GRAPH_FILE_BYTES, 512 * 1024 * 1024);
}

#[test]
fn max_graph_bytes_parse() {
    const G: i64 = 1024 * 1024 * 1024;
    let cases: &[(Option<&str>, i64)] = &[
        (None, MAX_GRAPH_FILE_BYTES),
        (Some("   "), MAX_GRAPH_FILE_BYTES),
        (Some("671088640"), 671_088_640),
        (Some("640MB"), 640 * 1024 * 1024),
        (Some("2GB"), 2 * G),
        (Some("3gb"), 3 * G),
        (Some("5 GB"), 5 * G),
        (Some("not-a-number"), MAX_GRAPH_FILE_BYTES),
        (Some("1.5GB"), MAX_GRAPH_FILE_BYTES),
        (Some("0x10"), MAX_GRAPH_FILE_BYTES),
        (Some("640KB"), MAX_GRAPH_FILE_BYTES),
        (Some("0"), MAX_GRAPH_FILE_BYTES),
        (Some("-1"), MAX_GRAPH_FILE_BYTES),
        (Some("-4GB"), MAX_GRAPH_FILE_BYTES),
    ];
    for &(env, want) in cases {
        assert_eq!(max_graph_file_bytes_from(env), want, "{env:?}");
    }
}

#[test]
fn graph_size_cap_cases() {
    let tmp = TempDir::new();
    let p = tmp.write("graph.json", b"AAAAAAAAAAAAAAAA"); // 16 bytes
    assert!(
        check_size(&p, MAX_GRAPH_FILE_BYTES).is_ok(),
        "under the limit"
    );
    assert_eq!(
        check_size(&p, 8).unwrap_err().to_string(),
        format!("graph file {} is 16 bytes, exceeds 8-byte cap", p.display())
    );
    assert!(check_size(&p, 16).is_ok(), "equal to the cap passes");
    assert!(check_size(&p, 15).is_err(), "one byte over fails");
    assert!(
        check_size(&tmp.join("does_not_exist.json"), 8).is_ok(),
        "a missing file is the caller's concern"
    );
}

/// test_ip_is_blocked_rejects_multicast_and_unspecified, the rows that hold for Go.
#[test]
fn ip_blocklist() {
    for bad in [
        "224.0.0.1",
        "ff02::1",
        "0.0.0.0",
        "::",
        "127.0.0.1",
        "10.1.2.3",
        "169.254.169.254",
        "100.64.0.1",
    ] {
        assert!(
            is_blocked_ip(bad.parse::<IpAddr>().unwrap()),
            "{bad} should be blocked"
        );
    }
    for good in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111", "239.1.1.1"] {
        assert!(
            !is_blocked_ip(good.parse::<IpAddr>().unwrap()),
            "{good} should pass"
        );
    }
}
