//! graphify-go `internal/extract/gocalls_test.go`. (`gotargets_test.go` tests `resolve.go` and is
//! ported with the resolve tests, graphify-rs-ihc.18.)

use crate::extract::kinds;
use crate::extract::{Call, Result, file_from_bytes};

// goCalls types receivers from the whole function in one walk: a local declared
// after the call still types it, and only calls inside the body count.
#[test]
fn test_go_calls_one_walk() {
    kinds::register_kinds();
    let src = concat!(
        "package p\n",
        "\n",
        "type Foo struct{}\n",
        "\n",
        "func newFoo() *Foo { return &Foo{} }\n",
        "\n",
        "func (s *Foo) run(a [len(\"xy\")]int, w Writer) {\n",
        "\tearly.Do()\n",
        "\ts.step()\n",
        "\tw.Write()\n",
        "\tplain()\n",
        "\tlate.Close()\n",
        "\tvar early Foo\n",
        "\tlate := newFoo()\n",
        "\tfor _, w := range a {\n",
        "\t\tw.Skip()\n",
        "\t}\n",
        "}\n"
    );
    let res = file_from_bytes("p/a.go", src.as_bytes());
    let last = res.defs.last().expect("defs").id.clone();
    let got: Vec<(String, String, String)> = res
        .calls
        .iter()
        .map(|n| res.call(n))
        .filter(|c| c.caller_id == last)
        .map(|c| (c.callee, c.recv, c.loc.to_string()))
        .collect();
    // w is a parameter and a range variable, so it stays untyped; len in the
    // parameter list is not a call of the body.
    let want = [
        ("Do", "Foo", "L8"),
        ("step", "self", "L9"),
        ("Write", "w", "L10"),
        ("plain", "", "L11"),
        ("Close", "Foo", "L12"),
        ("newFoo", "", "L14"),
        ("Skip", "w", "L16"),
    ];
    assert_eq!(got.len(), want.len(), "calls = {got:?}, want {want:?}");
    for (i, w) in want.iter().enumerate() {
        let g = &got[i];
        assert_eq!((g.0.as_str(), g.1.as_str(), g.2.as_str()), *w, "call {i}");
    }
}

// A text is held once per file, and a call that numbers a text the result
// does not have is found.
#[test]
fn test_call_text() {
    kinds::register_kinds();
    let res = file_from_bytes(
        "p/a.go",
        b"package p\n\nfunc f() {\n\tx.g()\n\tx.g()\n\tg()\n}\n",
    );
    assert!(
        res.calls.len() == 3 && std::mem::size_of::<Call>() == 16,
        "calls = {:?}, size {}",
        res.calls,
        std::mem::size_of::<Call>()
    );
    let want = ["", res.defs[0].id.as_str(), "g", "x"];
    assert_eq!(res.call_text, want, "CallText");
    assert!(res.calls_ok(), "CallsOK = false for an extracted result");
    let bad = [
        Result {
            calls: vec![Call {
                callee: 1,
                ..Call::default()
            }],
            ..Result::default()
        },
        Result {
            calls: vec![Call {
                recv: 2,
                ..Call::default()
            }],
            call_text: vec!["".into(), "a".into()],
            ..Result::default()
        },
        Result {
            calls: vec![Call {
                caller_id: 1,
                ..Call::default()
            }],
            call_text: vec!["x".into(), "a".into()],
            ..Result::default()
        },
    ];
    for b in &bad {
        assert!(!b.calls_ok(), "CallsOK = true for {b:?}");
    }
}

/// smoke; the real checks are the ihc.18-ignored tests
#[test]
fn test_extract_golang_smoke() {
    kinds::register_kinds();
    let srcs: [(&str, &[u8]); 2] = [
        (
            "p/a.go",
            b"package p\n\ntype Foo struct{}\n\nfunc newFoo() *Foo { return &Foo{} }\n\nfunc (s *Foo) run(a [len(\"xy\")]int, w Writer) {\n\tearly.Do()\n\ts.step()\n\tvar early Foo\n\tlate := newFoo()\n\tfor _, w := range a {\n\t\tw.Skip()\n\t}\n}\n",
        ),
        ("p/b.go", b"package p\n\nfunc f() {\n\tx.g()\n\tx.g()\n\tg()\n}\n"),
    ];
    for (rel, src) in srcs {
        assert!(super::extract_go(rel, src).is_some(), "{rel}");
        let res = file_from_bytes(rel, src);
        assert!(!res.nodes.is_empty(), "{rel}: no file node");
    }
}
