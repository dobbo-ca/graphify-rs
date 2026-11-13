//! Port of graphify-go `internal/model/code_test.go`. Not ported: the compile-time size check
//! `unsafe.Sizeof(Edge{}) == 64` and the field offsets of `TestEdgeLayout` (Go layout, and the
//! zero-size `typeOnlyJSON` trick that Rust does not need); `TestEdgeLayout`'s decode into a
//! sub-slice of a larger slice (Rust decodes into a fresh `Vec`). Its round trip is ported.

use std::collections::BTreeMap;

use gostd::json::{marshal, marshal_indent, unmarshal, unmarshal_into};

use crate::model::code::{LOC_DEFERRED, LOC_FLAGS, LOC_TEXT};
use crate::model::{Confidence, Edge, Loc, Relation, conf, file_of, line, loc_of, rel};

/// An edge as plain strings: what an Edge's JSON must equal.
#[derive(Clone, Debug, Default)]
struct Plain {
    source: String,
    target: String,
    relation: String,
    confidence: String,
    type_only: bool,
    source_file: String,
    source_location: String,
    weight: f64,
    confidence_score: f64,
}

gostd::json_fields!(Plain, "model.plain", {
    source => "source",
    target => "target",
    relation => "relation",
    confidence => "confidence",
    type_only => "type_only" omitempty,
    source_file => "source_file" omitempty,
    source_location => "source_location" omitempty,
    weight => "weight" omitempty,
    confidence_score => "confidence_score" omitempty,
});

fn plain(source: &str, target: &str, relation: &str, confidence: &str) -> Plain {
    Plain {
        source: source.into(),
        target: target.into(),
        relation: relation.into(),
        confidence: confidence.into(),
        ..Plain::default()
    }
}

fn json<T: gostd::json::Marshal + ?Sized>(v: &T) -> String {
    String::from_utf8(marshal(v).unwrap()).unwrap()
}

#[test]
#[ignore = "graphify-rs-ihc.7"]
fn test_edge_codes() {
    assert!(
        Relation::IMPLEMENTS.as_str() == "implements"
            && Confidence::AMBIGUOUS.as_str() == "AMBIGUOUS"
            && rel("calls") == Relation::CALLS
            && conf("") == Confidence(0),
        "constants and tables disagree"
    );
    // The JSON of an edge is the JSON of the same edge with string fields.
    const REL: &str = "rel<é>\x00\"x";
    let e = Edge {
        source: "a".into(),
        target: "b".into(),
        relation: rel(REL),
        confidence: conf("OTHER"),
        ..Edge::default()
    };
    let got = json(&e);
    assert_eq!(got, json(&plain("a", "b", REL, "OTHER")));
    let back: Edge = unmarshal(got.as_bytes()).expect("round trip");
    assert_eq!(back.relation.as_str(), REL);
    assert_eq!(back.confidence.as_str(), "OTHER");
    // New strings from many threads get one code each.
    let codes: Vec<Vec<Relation>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..8)
            .map(|_| {
                s.spawn(|| {
                    (0..500u32)
                        .map(|i| {
                            let s = format!(
                                "t{}{}",
                                char::from(b'a' + (i % 26) as u8),
                                char::from(b'a' + (i / 26) as u8)
                            );
                            let r = rel(&s);
                            assert_eq!(r.as_str(), s, "code {}", r.0);
                            r
                        })
                        .collect()
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (g, c) in codes.iter().enumerate() {
        assert_eq!(c, &codes[0], "thread {g} got another code for a string");
    }
}

#[test]
#[ignore = "graphify-rs-ihc.7"]
fn test_loc_and_file() {
    // Every text comes back as it went in, and equal texts have equal codes.
    let texts = [
        "",
        "L1",
        "L0",
        "L42",
        "L536870909",
        "L536870910",
        "L536870911",
        "L2147483646",
        "L2147483647",
        "L99999999999",
        "L01",
        "L-1",
        "L+1",
        "L",
        "l5",
        "L5 ",
        "12",
        "L1é",
        "a\x00\"<b>",
    ];
    let mut seen: BTreeMap<Loc, &str> = BTreeMap::new();
    for s in texts {
        let l = loc_of(s);
        assert!(
            l.to_string() == s && loc_of(s) == l,
            "loc_of({s:?}) = {}, text {:?}",
            l.0,
            l.to_string()
        );
        if let Some(prev) = seen.insert(l, s) {
            panic!("{prev:?} and {s:?} share code {}", l.0);
        }
        let mut got = Vec::new();
        l.append_json(&mut got);
        assert_eq!(got, marshal(s).unwrap(), "append_json({s:?})");
    }
    assert!(
        loc_of("") == Loc(0)
            && file_of("") == crate::model::File(0)
            && line(7) == loc_of("L7")
            && line(-3).to_string() == "L-3"
            && line(1 << 31).to_string() == "L2147483648",
        "zero values or line disagree with loc_of"
    );
    assert!(
        loc_of("L7").0 < LOC_DEFERRED
            && loc_of("L536870910").0 < LOC_DEFERRED
            && loc_of("L536870911").0 >= LOC_TEXT
            && loc_of("L01").0 >= LOC_TEXT,
        "a line is not held as a number, or a text is"
    );
    // Around the last line held as a number, with the flag bits and without.
    for n in [
        0i64,
        1,
        (1 << 29) - 3,
        (1 << 29) - 2,
        (1 << 29) - 1,
        (1 << 31) - 2,
    ] {
        let s = format!("L{n}");
        let l = line(n);
        let want = marshal(s.as_str()).unwrap();
        let mut e = Edge {
            source_location: l,
            ..Edge::default()
        };
        e.set_type_only(true);
        e.set_deferred(true);
        assert!(
            l == loc_of(&s)
                && l.0 & LOC_FLAGS == 0
                && (l.0 >= LOC_TEXT) == (n > (1 << 29) - 2)
                && e.loc() == l
                && e.source_location != l,
            "line {n}: loc {:#x}, with the bits {:#x}",
            l.0,
            e.source_location.0
        );
        for l in [l, e.source_location] {
            let mut j = Vec::new();
            l.append_json(&mut j);
            assert!(
                l.to_string() == s && j == want,
                "line {n}: {:#x} is {:?}",
                l.0,
                l.to_string()
            );
        }
        e.set_type_only(false);
        e.set_deferred(false);
        assert!(
            e.source_location == l && !e.type_only() && !e.deferred(),
            "line {n}: {:#x} after the bits are cleared",
            e.source_location.0
        );
    }
    // The JSON of an edge is the JSON of the same edge with string fields.
    for (file, loc) in [("", ""), ("d/<é>.go", "L12"), ("x.md", "sec & 2")] {
        let p = Plain {
            source_file: file.into(),
            source_location: loc.into(),
            ..plain("a", "b", "calls", "EXTRACTED")
        };
        let want = json(&p);
        let e: Edge = unmarshal(want.as_bytes()).unwrap();
        assert_eq!(json(&e), want);
        assert_eq!(e.source_file.as_str(), file);
        assert_eq!(e.source_location.to_string(), loc);
    }
}

/// v0.10.1 held confidence as a string, so any number of distinct values loads.
#[test]
#[ignore = "graphify-rs-ihc.7"]
fn test_many_confidences() {
    const N: usize = 70_000;
    let mut sb = String::from("[");
    for i in 0..N {
        if i > 0 {
            sb.push(',');
        }
        sb.push_str(&format!(
            r#"{{"source":"a","target":"b","relation":"calls","confidence":"many{i}"}}"#
        ));
    }
    sb.push(']');
    let es: Vec<Edge> = unmarshal(sb.as_bytes()).unwrap();
    assert_eq!(es.len(), N);
    for i in [0, 65534, 65535, N - 1] {
        let want = format!("many{i}");
        assert_eq!(es[i].confidence.as_str(), want, "edge {i}");
        assert_eq!(conf(&want), es[i].confidence, "edge {i}");
    }
    assert_eq!(
        json(&es),
        sb,
        "the edges do not marshal back to their input"
    );
}

#[test]
#[ignore = "graphify-rs-ihc.7"]
fn test_type_only_json() {
    const IN: &str = r#"{"source":"a","target":"b","relation":"imports_from","confidence":"EXTRACTED","type_only":true,"source_file":"a.ts","source_location":"L3"}"#;
    let mut e: Edge = unmarshal(IN.as_bytes()).unwrap();
    assert!(
        e.type_only()
            && e.confidence == Confidence::EXTRACTED
            && e.relation == Relation::IMPORTS_FROM
            && e.loc() == line(3),
        "unmarshal: {e:?}"
    );
    assert_eq!(json(&e), IN);
    e.set_type_only(false);
    assert!(
        !json(&e).contains("type_only"),
        "type_only written when false"
    );

    // The JSON of an edge is the JSON of the same edge with plain fields.
    for type_only in [false, true] {
        for loc in ["", "L0", "L12", "L1073741823", "sec <&> 2"] {
            let mut p1 = plain("a", "b", "imports_from", "EXTRACTED");
            p1.type_only = type_only;
            p1.source_location = loc.into();
            let mut p2 = plain("a<", "b\"", "rel é", "OTHER");
            p2.type_only = type_only;
            p2.source_file = "d/<é>.ts".into();
            p2.source_location = loc.into();
            p2.weight = 1.5;
            p2.confidence_score = -1e-7;
            for p in [p1, p2] {
                let want = json(&p);
                let e: Edge = unmarshal(want.as_bytes()).unwrap();
                assert!(
                    e.type_only() == type_only
                        && e.loc() == loc_of(loc)
                        && e.source_location.to_string() == loc
                        && e.source_file.as_str() == p.source_file
                        && e.source == p.source
                        && e.target == p.target
                        && e.relation.as_str() == p.relation
                        && e.confidence.as_str() == p.confidence
                        && e.weight == p.weight
                        && e.confidence_score == p.confidence_score,
                    "{want} decodes to {e:?}"
                );
                // Decoded behind a pointer and as a map value.
                let ptr: Option<Edge> = unmarshal(want.as_bytes()).unwrap();
                let by_key: BTreeMap<String, Edge> =
                    unmarshal(format!(r#"{{"k":{want}}}"#).as_bytes()).unwrap();
                assert!(
                    ptr.as_ref() == Some(&e) && by_key.get("k") == Some(&e),
                    "{want}"
                );
                // By value, by reference, in a slice and indented.
                let got_i = marshal_indent(&e, "", "  ").unwrap();
                let want_i = marshal_indent(&p, "", "  ").unwrap();
                assert!(
                    json(&e) == want
                        && json(&&e) == want
                        && json(std::slice::from_ref(&e)) == format!("[{want}]")
                        && got_i == want_i,
                    "{want}"
                );
                if loc.is_empty() {
                    assert!(
                        !json(&e).contains("source_location"),
                        "source_location written for no location"
                    );
                }
                // Two edges that differ only in type_only are different values.
                let mut other = e.clone();
                other.set_type_only(!type_only);
                assert!(
                    other != e
                        && other.loc() == e.loc()
                        && other.source_location.to_string() == e.source_location.to_string(),
                    "{e:?} and {other:?}"
                );
            }
        }
    }

    // As a struct with a bool field decodes: any key case, last key wins, and an absent key or a
    // null leaves the value alone.
    let mut e = Edge::default();
    for (input, want) in [
        (r#"{"TYPE_ONLY":true}"#, true),
        (r#"{"source_location":"L4"}"#, true),
        ("null", true),
        (r#"{"type_only":null}"#, true),
        (
            r#"{"type_only":true,"source_location":"x","type_only":false}"#,
            false,
        ),
        (
            r#"{"type_only":false,"type_only":true,"source_location":""}"#,
            true,
        ),
    ] {
        unmarshal_into(input.as_bytes(), &mut e).unwrap_or_else(|err| panic!("{input}: {err}"));
        assert_eq!(e.type_only(), want, "{input}");
    }
    assert!(
        e.loc() == Loc(0) && e.source_location != Loc(0),
        "location {:#x}",
        e.source_location.0
    );
    assert!(
        unmarshal_into(br#"{"type_only":1}"#, &mut e).is_err(),
        "no error for a number"
    );
}

/// The round-trip half of Go `TestEdgeLayout`: three edges, one type-only and one deferred, come
/// back equal, and each flag is written once.
#[test]
#[ignore = "graphify-rs-ihc.7"]
fn test_edge_layout_round_trip() {
    let mut want = vec![
        Edge {
            source: "a".into(),
            target: "b".into(),
            relation: Relation::CALLS,
            confidence: Confidence::EXTRACTED,
            source_file: file_of("a.go"),
            source_location: line(1),
            weight: 2.0,
            ..Edge::default()
        },
        Edge {
            source: "c".into(),
            target: "d".into(),
            relation: Relation::IMPORTS_FROM,
            confidence: Confidence::INFERRED,
            source_file: file_of("c.ts"),
            source_location: line(2),
            confidence_score: 0.25,
            ..Edge::default()
        },
        Edge {
            source: "e".into(),
            target: "f".into(),
            relation: Relation::IMPLEMENTS,
            confidence: Confidence::EXTRACTED,
            ..Edge::default()
        },
    ];
    want[1].set_type_only(true);
    want[2].set_deferred(true);
    let input = json(&want);
    assert_eq!(input.matches("type_only").count(), 1, "{input}");
    assert_eq!(input.matches("deferred").count(), 1, "{input}");
    let got: Vec<Edge> = unmarshal(input.as_bytes()).unwrap();
    assert_eq!(got, want);
}
