//! Ported from encoding/xml/read_test.go (GOSRC/encoding/xml/read_test.go, Go 1.27.4,
//! BSD-3-Clause, Copyright 2009 The Go Authors).
//!
//! graphify-go decodes pom.xml into `xmlElem{XMLName; Chardata ",chardata"; Children ",any"}`
//! (manifest.go:315), so only tests that reach that shape are ported, through
//! `unmarshal_element`. The depth tests keep the Go inputs; their custom
//! `UnmarshalXML` / `,any` struct is the same recursive shape as `xmlElem`.
//!
//! - TestUnmarshalFeed: not ported: Feed uses attr, time and innerxml fields, not reached by graphify-go
//! - TestUnmarshalPaths: not ported: `a>b` path tags, not reached by graphify-go
//! - TestUnmarshalBadPaths: not ported: TagPathError from path tags, not reached by graphify-go
//! - TestUnmarshalWithoutNameType: not ported: only asserts a string `,attr` field, not reached by graphify-go
//! - TestUnmarshalAttr: not ported: `,attr` into int and pointer fields, not reached by graphify-go
//! - TestUnmarshalNS: not ported: namespaced struct tags and Decoder.DefaultSpace, not reached by graphify-go
//! - TestMarshalNS: not ported: marshal with namespaced tags, not reached by graphify-go
//! - TestUnmarshalNSAttr: not ported: namespaced `,attr` tags and DefaultSpace, not reached by graphify-go
//! - TestMarshalNSAttr: not ported: marshal with namespaced attrs, not reached by graphify-go
//! - TestUnmarshaler: not ported: Unmarshaler and UnmarshalerAttr interfaces, not reached by graphify-go
//! - TestUnmarshalIntoInterface: not ported: decode into an `any` field, not reached by graphify-go
//! - TestInvalidInnerXMLType: not ported: `,innerxml` field, not reached by graphify-go
//! - TestUnmarshalEmptyValues: not ported: scalar, pointer and slice fields, not reached by graphify-go
//! - TestUnmarshalWhitespaceValues: not ported: scalar fields, not reached by graphify-go
//! - TestUnmarshalWhitespaceAttrs: not ported: scalar `,attr` fields, not reached by graphify-go
//! - TestUnmarshalIntoNil: not ported: Go nil pointer argument has no Rust equivalent
//! - TestUnmarshalXMLRawToken: not ported: custom UnmarshalXML calling Decoder.RawToken, not reached by graphify-go
//!
//! Source funcs 23, ported 6 (the 17 above are not ported).
//! Extra test (not in Go): pom_shapes.

use gostd::xml::unmarshal_element;

// The deep-nesting inputs recurse; run them with room to spare.
fn big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

// Go maxUnmarshalDepth (read.go:319).
const MAX_UNMARSHAL_DEPTH: usize = 10000;

// Issue 11112. Unmarshal must reject invalid comments.
#[test]
fn test_malformed_comment() {
    let test_data = [
        "<X><!-- a---></X>",
        "<X><!-- -- --></X>",
        "<X><!-- a--b --></X>",
        "<X><!------></X>",
    ];
    for (i, test) in test_data.iter().enumerate() {
        assert!(
            unmarshal_element(test.as_bytes()).is_err(),
            "{i}: unmarshal should reject invalid comments"
        );
    }
}

#[test]
fn test_cve202228131() {
    big_stack(|| {
        let data = "<a>".repeat(MAX_UNMARSHAL_DEPTH + 1);
        let err = unmarshal_element(data.as_bytes()).expect_err("Unmarshal did not fail");
        assert_eq!(err.msg, "exceeded max depth");
    });
}

// Unmarshal must not panic on 17M unclosed elements.
#[test]
fn test_cve202230633() {
    big_stack(|| {
        let data = "<a>".repeat(17_000_000);
        let _ = unmarshal_element(data.as_bytes());
    });
}

#[test]
fn test_decode_element_recursion() {
    big_stack(|| {
        // (name, depth, want_err)
        let tests = [
            ("below limit", MAX_UNMARSHAL_DEPTH, false),
            ("above limit", MAX_UNMARSHAL_DEPTH + 1, true),
        ];
        for (name, depth, want_err) in tests {
            let payload = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
            let got = unmarshal_element(payload.as_bytes());
            assert_eq!(got.is_err(), want_err, "{name}: got {got:?}");
            if let Err(err) = got {
                assert_eq!(err.msg, "exceeded max depth", "{name}");
            }
        }
    });
}

#[test]
fn test_decode_element_depth_bypass() {
    big_stack(|| {
        // 3 blocks of 5,000 nested <section> tags, separated by <extension>
        // tags. Total XML nesting depth = 15,003 (maxUnmarshalDepth is 10,000).
        let open_sections = "<section>".repeat(5000);
        let close_sections = "</section>".repeat(5000);
        let mut buf = String::new();
        for _ in 0..3 {
            buf.push_str(&open_sections);
            buf.push_str("<extension>");
        }
        for _ in 0..3 {
            buf.push_str("</extension>");
            buf.push_str(&close_sections);
        }
        let err = unmarshal_element(buf.as_bytes()).expect_err("Unmarshal did not fail");
        assert_eq!(err.msg, "exceeded max depth");
    });
}

#[test]
fn test_recursive_unmarshal_interface_depth() {
    big_stack(|| {
        let depth = MAX_UNMARSHAL_DEPTH + 1;
        let payload = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
        let err = unmarshal_element(payload.as_bytes()).expect_err("Unmarshal did not fail");
        assert_eq!(err.msg, "exceeded max depth");
    });
}

// Extra (Review Focus: pom.xml shapes): the three pom.xml inputs the plan names.
#[test]
fn pom_shapes() {
    // A declared encoding with no CharsetReader is an error.
    let err = unmarshal_element(
        b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<project><modelVersion>4.0.0</modelVersion></project>",
    )
    .unwrap_err();
    assert_eq!(
        err.msg,
        "xml: encoding \"ISO-8859-1\" declared but Decoder.CharsetReader is nil"
    );

    // A default xmlns keeps local = "project" (the namespace goes in space).
    let e = unmarshal_element(
        b"<project xmlns=\"http://maven.apache.org/POM/4.0.0\"><modelVersion>4.0.0</modelVersion></project>",
    )
    .unwrap();
    assert_eq!(e.name.local, "project");
    assert_eq!(e.name.space, "http://maven.apache.org/POM/4.0.0");
    assert_eq!(e.children.len(), 1);
    assert_eq!(e.children[0].name.local, "modelVersion");
    assert_eq!(e.children[0].chardata, "4.0.0");

    // Text after the root element is ignored.
    let e = unmarshal_element(b"<project><a>x</a></project>\ntrailing text <junk").unwrap();
    assert_eq!(e.name.local, "project");
    assert_eq!(e.children.len(), 1);
    let e = unmarshal_element(b"<project/><second/>").unwrap();
    assert_eq!(e.name.local, "project");
    assert!(e.children.is_empty());
}
