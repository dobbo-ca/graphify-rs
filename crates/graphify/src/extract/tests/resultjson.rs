//! graphify-go `internal/extract/resultjson_test.go`. Go compares `AppendJSON` with
//! `json.Marshal(r)`, which fills every exported field by reflection. The Rust `Result` has no
//! reflective marshal (and no nil slice: an empty `Vec` is both Go's nil and empty), so the same
//! guard is a round trip: every field is set, `append_json` then `unmarshal` gives the value back,
//! so a field `append_json` does not write shows up as a difference. The text checks of the Go
//! test are kept as they are. The Go string `odd` has `\xff`, which a Rust `String` cannot hold:
//! it is U+FFFD here (what Go's decoder makes of it).

use std::collections::BTreeMap;

use gostd::json::unmarshal;

use crate::extract::{
    Call, Def, Imp, ImportAlias, LabelInputs, MdRef, ModInvoke, ModRef, NullLabelRef, Result,
    SegVal, TypeRef,
};
use crate::model::{Edge, Node, conf, file_of, line, rel};

/// Go's `fillJSON` result: every field of `Result` and of everything under it is non-zero.
fn full() -> Result {
    let node = |n: u32| Node {
        id: format!("id{n}"),
        label: format!("label{n}"),
        file_type: format!("code{n}"),
        source_file: format!("f{n}.go"),
        source_location: format!("L{n}"),
        computed_name: format!("cn{n}"),
        attributes: BTreeMap::from([(format!("k{n}"), format!("v{n}"))]),
    };
    let edge = |n: u32| Edge {
        source: format!("s{n}"),
        target: format!("t{n}"),
        relation: rel(["contains", "calls", "imports"][n as usize % 3]),
        confidence: conf(["EXTRACTED", "INFERRED", "AMBIGUOUS"][n as usize % 3]),
        source_file: file_of(["a.go", "b.go", "c.go"][n as usize % 3]),
        source_location: line(i64::from(n)),
        weight: f64::from(n) / 4.0,
        confidence_score: f64::from(n) / 8.0,
    };
    let s = |n: u32| format!("s{n}");
    Result {
        nodes: vec![node(1), node(2)],
        edges: vec![edge(1), edge(2)],
        defs: vec![
            Def {
                id: s(1),
                name: s(2),
                file: s(3),
                owner: s(4),
                is_abstract: true,
                value: true,
                export: true,
            },
            Def {
                id: s(5),
                is_abstract: true,
                value: true,
                export: true,
                ..Def::default()
            },
        ],
        calls: vec![
            Call {
                caller_id: 1,
                callee: 2,
                loc: line(3),
                recv: 4,
            },
            Call {
                caller_id: 5,
                callee: 6,
                loc: line(7),
                recv: 8,
            },
        ],
        call_text: vec![String::new(), s(9)],
        file: s(10),
        call_mods: vec![s(11), s(12)],
        type_refs: vec![
            TypeRef {
                from_id: s(1),
                name: s(2),
                relation: s(3),
                file: s(4),
                loc: line(5),
                stub: true,
                deco: true,
                qual: s(6),
                arg: true,
            },
            TypeRef {
                from_id: s(7),
                loc: line(8),
                qual: s(9),
                stub: true,
                deco: true,
                arg: true,
                ..TypeRef::default()
            },
        ],
        imps: vec![
            Imp {
                file_id: s(1),
                file: s(2),
                spec: s(3),
                loc: line(4),
                type_only: true,
                rel: true,
                export: true,
                dynamic: 2,
                names: vec![s(5), s(6)],
            },
            Imp {
                spec: s(7),
                dynamic: 1,
                names: vec![s(8)],
                ..Imp::default()
            },
        ],
        mod_refs: vec![
            ModRef {
                from_id: s(1),
                source: s(2),
                file: s(3),
                loc: line(4),
            },
            ModRef {
                from_id: s(5),
                ..ModRef::default()
            },
        ],
        null_labels: vec![
            NullLabelRef {
                node_id: s(1),
                scope: s(2),
                inputs: LabelInputs::default(),
            },
            NullLabelRef {
                node_id: s(3),
                ..NullLabelRef::default()
            },
        ],
        mod_invokes: vec![
            ModInvoke {
                node_id: s(1),
                dir: s(2),
                args: BTreeMap::from([(s(3), SegVal::default()), (s(4), SegVal::default())]),
                arg_var_refs: BTreeMap::from([(s(5), s(6)), (s(7), s(8))]),
            },
            ModInvoke {
                node_id: s(9),
                ..ModInvoke::default()
            },
        ],
        import_aliases: vec![
            ImportAlias {
                local: s(1),
                imported: s(2),
                module_stem: s(3),
                spec: s(4),
                loc: line(5),
                in_func: true,
            },
            ImportAlias {
                local: s(6),
                in_func: true,
                ..ImportAlias::default()
            },
        ],
        md_refs: vec![
            MdRef {
                from_id: s(1),
                target: s(2),
                file: s(3),
                loc: line(4),
            },
            MdRef {
                from_id: s(5),
                ..MdRef::default()
            },
        ],
        go_mods: BTreeMap::from([(s(1), s(2)), (s(3), s(4))]),
        default: s(13),
        exports: vec![s(14), s(15)],
        type_aliases: vec![s(16), s(17)],
        java_rets: vec![s(18), s(19)],
        cs_types: vec![s(20), s(21)],
        over_budget: true,
    }
}

/// `append_json` after a prefix, as Go's test calls it, and the value decoded back.
fn round_trip(r: &Result) -> (Vec<u8>, Result) {
    let mut b = b"x".to_vec();
    r.append_json(&mut b)
        .unwrap_or_else(|e| panic!("append_json: {e}"));
    let back = unmarshal::<Result>(&b[1..])
        .unwrap_or_else(|e| panic!("unmarshal of {}: {e}", text(&b[1..])));
    (b[1..].to_vec(), back)
}

/// JSON bytes for an assert message, one char per byte (`from_utf8_lossy` is banned, plan R7).
fn text(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

#[test]
fn test_result_append_json_matches_marshal() {
    let odd = "<\u{e9}&> \"q\\\u{0}\u{7f}\u{fffd}\u{2028}\u{65e5}\u{672c}";
    let mut type_only = Edge {
        weight: 3.0,
        ..Edge::default()
    };
    type_only.set_type_only(true);
    let mut type_only_at = Edge {
        source_location: line(7),
        ..Edge::default()
    };
    type_only_at.set_type_only(true);
    let mut deferred = Edge {
        source_location: line(4),
        ..Edge::default()
    };
    deferred.set_deferred(true);
    let cases: [(&str, Result); 4] = [
        ("zero", Result::default()),
        ("full", full()),
        (
            "empty slices",
            Result {
                imps: vec![Imp {
                    names: Vec::new(),
                    ..Imp::default()
                }],
                ..Result::default()
            },
        ),
        (
            "omitted and escaped",
            Result {
                nodes: vec![
                    Node {
                        id: odd.into(),
                        label: odd.into(),
                        ..Node::default()
                    },
                    Node::default(),
                    Node {
                        attributes: BTreeMap::from([
                            ("b".to_string(), odd.to_string()),
                            ("a".to_string(), "1".to_string()),
                        ]),
                        ..Node::default()
                    },
                ],
                edges: vec![
                    Edge {
                        source: odd.into(),
                        ..Edge::default()
                    },
                    Edge {
                        weight: 1e-7,
                        confidence_score: 1e21,
                        ..Edge::default()
                    },
                    Edge {
                        weight: -0.0,
                        confidence_score: -2.5,
                        ..Edge::default()
                    },
                    type_only.clone(),
                    type_only_at.clone(),
                    deferred,
                ],
                defs: vec![
                    Def {
                        id: odd.into(),
                        is_abstract: true,
                        value: true,
                        ..Def::default()
                    },
                    Def::default(),
                ],
                calls: vec![Call {
                    callee: 1,
                    ..Call::default()
                }],
                call_text: vec![String::new(), odd.into()],
                imps: vec![
                    Imp {
                        spec: odd.into(),
                        names: vec![odd.into(), String::new()],
                        ..Imp::default()
                    },
                    Imp {
                        rel: true,
                        ..Imp::default()
                    },
                    Imp {
                        type_only: true,
                        ..Imp::default()
                    },
                    Imp {
                        dynamic: 2,
                        ..Imp::default()
                    },
                ],
                ..Result::default()
            },
        ),
    ];
    for (name, r) in &cases {
        let (json, back) = round_trip(r);
        assert!(
            back == *r,
            "{name}:\n json: {}\n got: {back:?}\nwant: {r:?}",
            text(&json)
        );
    }

    let r = Result {
        edges: vec![type_only, type_only_at],
        ..Result::default()
    };
    let mut got = Vec::new();
    r.append_json(&mut got)
        .unwrap_or_else(|e| panic!("append_json: {e}"));
    let has = |needle: &[u8]| got.windows(needle.len()).any(|w| w == needle);
    assert!(
        has(br#""type_only":true,"weight":3}"#)
            && has(br#""type_only":true,"source_location":"L7"}"#),
        "type-only edges: {}",
        text(&got)
    );

    // An unencodable float is an error, as it is for json.Marshal.
    for e in [
        Edge {
            weight: f64::NAN,
            ..Edge::default()
        },
        Edge {
            confidence_score: f64::INFINITY,
            ..Edge::default()
        },
    ] {
        let r = Result {
            edges: vec![e.clone()],
            ..Result::default()
        };
        assert!(r.append_json(&mut Vec::new()).is_err(), "{e:?}: no error");
    }
}
