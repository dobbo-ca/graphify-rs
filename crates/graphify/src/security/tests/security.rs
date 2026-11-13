//! Port of graphify-go `internal/security/security_test.go`.

use crate::security::security::max_graph_file_bytes_from;
use crate::security::{MAX_GRAPH_FILE_BYTES, sanitize_label, validate_graph_path, validate_url};
use crate::testutil::TempDir;

#[test]
fn test_validate_url_scheme() {
    for bad in ["file:///etc/passwd", "ftp://host/x", "data:text/plain,hi"] {
        assert!(
            validate_url(bad).is_err(),
            "validate_url({bad:?}) = Ok, want an error"
        );
    }
}

#[test]
fn test_validate_url_blocks_loopback() {
    // Resolves to 127.0.0.1 without a network call.
    assert!(
        validate_url("http://localhost/x").is_err(),
        "want an SSRF block"
    );
}

#[test]
fn test_validate_graph_path_containment() {
    let base = TempDir::new();
    let inside = base.write("graph.json", b"{}");
    let base_s = base.path().to_str().unwrap();
    if let Err(e) = validate_graph_path(inside.to_str().unwrap(), base_s) {
        panic!("inside path rejected: {e}");
    }
    let escape = format!("{base_s}/../etc");
    assert!(
        validate_graph_path(&escape, base_s).is_err(),
        "path escaping base was accepted"
    );
}

#[test]
fn test_sanitize_label() {
    assert_eq!(sanitize_label("a\x00b\x1fc"), "abc");
}

/// Go sets GRAPHIFY_MAX_GRAPH_BYTES per case; here the value goes to the parse directly.
#[test]
fn test_max_graph_file_bytes() {
    let cases: &[(&str, Option<&str>, i64)] = &[
        ("unset", None, MAX_GRAPH_FILE_BYTES),
        ("blank", Some("  "), MAX_GRAPH_FILE_BYTES),
        ("gb suffix", Some("1GB"), 1_073_741_824),
        ("mb suffix lowercase", Some("640mb"), 640 * 1024 * 1024),
        ("plain bytes", Some("123456"), 123_456),
        ("invalid", Some("not-a-number"), MAX_GRAPH_FILE_BYTES),
        ("zero", Some("0"), MAX_GRAPH_FILE_BYTES),
        ("negative", Some("-5"), MAX_GRAPH_FILE_BYTES),
    ];
    for &(name, env, want) in cases {
        assert_eq!(max_graph_file_bytes_from(env), want, "{name}");
    }
}
