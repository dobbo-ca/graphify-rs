//! Ported from encoding/xml/marshal_test.go (GOSRC/encoding/xml/marshal_test.go, Go go1.27.4).
//! Source licence: BSD-3-Clause (The Go Authors).
//!
//! - TestMarshal (1661): ported (marshalTests: 24 rows kept as `Node` values, the rest left out:
//!   Plain scalar/array/time rows, interface and pointer rows, innerxml, comment, int/float/bool
//!   attr, namespace, XMLName-from-field, Marshaler and generic rows are not reached by the
//!   `Node` writer)
//! - TestMarshalErrors (1758): not ported: the writer takes a `Node` and has no error path
//! - TestUnmarshal (1777): not ported: not reached by graphify-go (decodes marshalTests into typed structs)
//! - TestMarshalIndent (1824): not ported: its only row marshals an `,innerxml` field, which `Node` cannot express
//! - TestMarshalWriteErrors (1858): not ported: `marshal_indent` returns a `Vec<u8>`, no writer
//! - TestMarshalWriteIOErrors (1886): not ported: no writer
//! - TestMarshalFlush (1896): not ported: no Encoder
//! - TestStructPointerMarshal (1933): not ported: pointer-to-struct fields are not reached
//! - TestEncodeToken (2317): not ported: EncodeToken is not reached by graphify-go
//! - TestProcInstEncodeToken (2355): not ported: EncodeToken is not reached
//! - TestDecodeEncode (2372): not ported: Encoder is not reached
//! - TestRace9796 (2390): not ported: Go data-race test, no Encoder
//! - TestIsValidDirective (2406): not ported: EncodeToken is not reached
//! - TestSimpleUseOfEncodeToken (2439): not ported: EncodeToken is not reached
//! - TestIssue16158 (2470): not ported: `,attr` into a byte field, not reached by graphify-go
//! - TestInvalidXMLName (2489): not ported: `Node` has no XMLName validation path
//! - TestMarshalZeroValue (2511): not ported: omitempty and embedded pointers are not reached
//! - TestClose (2562): not ported: Encoder.Close is not reached
//!
//! Source funcs: 18 (plus 2 Benchmarks not counted). Ported: 1, plus the extra test escape_rules.
//!
//! Extra test (not in Go): escape_rules.

use gostd::xml::{Node, escape_attr, escape_text, marshal_indent};

fn el(name: &str, attrs: &[(&str, &str)], text: &str, children: Vec<Node>) -> Node {
    Node {
        name: name.to_string(),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        chardata: text.to_string(),
        children,
    }
}

fn leaf(name: &str, text: &str) -> Node {
    el(name, &[], text, vec![])
}

fn kids(name: &str, children: Vec<Node>) -> Node {
    el(name, &[], "", children)
}

fn passenger(names: &[&str], weight: &str) -> Node {
    let mut c: Vec<Node> = names.iter().map(|n| leaf("name", n)).collect();
    c.push(leaf("weight", weight));
    kids("passenger", c)
}

#[test]
fn test_marshal() {
    // GOSRC marshal_test.go:542 marshalTests; Go line of each row in the comment.
    let ship = {
        let mut c = vec![leaf("drive", "1"), leaf("age", "1")];
        c.push(passenger(&["Zaphod", "Beeblebrox"], "7.25"));
        c.push(passenger(&["Trisha", "McMillen"], "5.5"));
        c.push(passenger(&["Ford", "Prefect"], "7"));
        c.push(passenger(&["Arthur", "Dent"], "6.75"));
        el(
            "spaceship",
            &[("name", "Heart of Gold"), ("pilot", "Computer")],
            "",
            c,
        )
    };
    let embed_a = kids(
        "EmbedA",
        vec![
            leaf("FieldB", "A.C.B"),
            leaf("FieldC", "A.C.C"),
            kids(
                "EmbedB",
                vec![
                    leaf("FieldB", "A.B.B"),
                    kids(
                        "FieldA",
                        vec![leaf("A1", "A.B.C.A1"), leaf("A2", "A.B.C.A2")],
                    ),
                    leaf("FieldC", "A.B.C.C"),
                ],
            ),
            leaf("FieldA", "A.A"),
            leaf("FieldE", "A.D.E"),
        ],
    );
    let rows: Vec<(u32, Node, &str)> = vec![
        // 568
        (
            568,
            kids("Plain", vec![leaf("V", "gopher")]),
            "<Plain><V>gopher</V></Plain>",
        ),
        // 570
        (
            570,
            kids("Plain", vec![leaf("V", "</>")]),
            "<Plain><V>&lt;/&gt;</V></Plain>",
        ),
        // 590
        (
            590,
            kids("PresenceTest", vec![]),
            "<PresenceTest></PresenceTest>",
        ),
        // 608
        (
            608,
            el(
                "Data",
                &[("Attr", "v")],
                "",
                vec![leaf("Bytes", "ab"), leaf("Custom", "cd")],
            ),
            "<Data Attr=\"v\"><Bytes>ab</Bytes><Custom>cd</Custom></Data>",
        ),
        // 633
        (
            633,
            el("port", &[("type", "ssl")], "443", vec![]),
            "<port type=\"ssl\">443</port>",
        ),
        // 634
        (634, leaf("port", "443"), "<port>443</port>"),
        // 635
        (
            635,
            el("port", &[("type", "<unix>")], "", vec![]),
            "<port type=\"&lt;unix&gt;\"></port>",
        ),
        // 638
        (
            638,
            leaf("domain", "google.com&friends"),
            "<domain>google.com&amp;friends</domain>",
        ),
        // 640
        (
            640,
            leaf("book", "Pride & Prejudice"),
            "<book>Pride &amp; Prejudice</book>",
        ),
        // 650
        (
            650,
            ship,
            concat!(
                "<spaceship name=\"Heart of Gold\" pilot=\"Computer\">",
                "<drive>1</drive>",
                "<age>1</age>",
                "<passenger>",
                "<name>Zaphod</name>",
                "<name>Beeblebrox</name>",
                "<weight>7.25</weight>",
                "</passenger>",
                "<passenger>",
                "<name>Trisha</name>",
                "<name>McMillen</name>",
                "<weight>5.5</weight>",
                "</passenger>",
                "<passenger>",
                "<name>Ford</name>",
                "<name>Prefect</name>",
                "<weight>7</weight>",
                "</passenger>",
                "<passenger>",
                "<name>Arthur</name>",
                "<name>Dent</name>",
                "<weight>6.75</weight>",
                "</passenger>",
                "</spaceship>",
            ),
        ),
        // 702
        (
            702,
            kids("result", vec![kids("Items", vec![])]),
            "<result><Items></Items></result>",
        ),
        // 717
        (
            717,
            kids("result", vec![kids("Items", vec![leaf("item1", "A")])]),
            "<result><Items><item1>A</item1></Items></result>",
        ),
        // 725
        (
            725,
            kids(
                "result",
                vec![kids("Items", vec![leaf("item", "A"), leaf("item", "B")])],
            ),
            "<result><Items><item>A</item><item>B</item></Items></result>",
        ),
        // 734
        (
            734,
            kids(
                "result",
                vec![kids(
                    "Items",
                    vec![leaf("item", "A"), leaf("item", "B"), leaf("item1", "C")],
                )],
            ),
            "<result><Items><item>A</item><item>B</item><item1>C</item1></Items></result>",
        ),
        // 744
        (
            744,
            kids(
                "result",
                vec![kids(
                    "parent",
                    vec![leaf("c", "C"), leaf("b", "B"), leaf("a", "A")],
                )],
            ),
            "<result><parent><c>C</c><b>B</b><a>A</a></parent></result>",
        ),
        // 764
        (
            764,
            kids(
                "result",
                vec![
                    kids("parent1", vec![leaf("a", "A")]),
                    leaf("b", "B"),
                    kids(
                        "parent1",
                        vec![kids("parent2", vec![leaf("c", "C")]), leaf("d", "D")],
                    ),
                ],
            ),
            concat!(
                "<result>",
                "<parent1><a>A</a></parent1>",
                "<b>B</b>",
                "<parent1>",
                "<parent2><c>C</c></parent2>",
                "<d>D</d>",
                "</parent1>",
                "</result>",
            ),
        ),
        // 775
        (
            775,
            kids("service", vec![kids("host", vec![leaf("port", "80")])]),
            "<service><host><port>80</port></host></service>",
        ),
        // 779
        (779, kids("service", vec![]), "<service></service>"),
        // 865
        (
            865,
            embed_a,
            concat!(
                "<EmbedA>",
                "<FieldB>A.C.B</FieldB>",
                "<FieldC>A.C.C</FieldC>",
                "<EmbedB>",
                "<FieldB>A.B.B</FieldB>",
                "<FieldA>",
                "<A1>A.B.C.A1</A1>",
                "<A2>A.B.C.A2</A2>",
                "</FieldA>",
                "<FieldC>A.B.C.C</FieldC>",
                "</EmbedB>",
                "<FieldA>A.A</FieldA>",
                "<FieldE>A.D.E</FieldE>",
                "</EmbedA>",
            ),
        ),
        // 904
        (
            904,
            kids("EmbedB", vec![leaf("FieldB", "")]),
            "<EmbedB><FieldB></FieldB></EmbedB>",
        ),
        // 910
        (
            910,
            kids("PointerAnonFields", vec![]),
            "<PointerAnonFields></PointerAnonFields>",
        ),
        // 916
        (
            916,
            el(
                "casing",
                &[("Xy", "mixedA"), ("XY", "upperA")],
                "",
                vec![leaf("Xy", "mixed"), leaf("XY", "upper")],
            ),
            "<casing Xy=\"mixedA\" XY=\"upperA\"><Xy>mixed</Xy><XY>upper</XY></casing>",
        ),
        // 1178
        (
            1178,
            kids(
                "a",
                vec![
                    kids(
                        "nested",
                        vec![leaf(
                            "value",
                            "dquote: \"; squote: '; ampersand: &; less: <; greater: >;",
                        )],
                    ),
                    kids("empty", vec![]),
                ],
            ),
            "<a><nested><value>dquote: &#34;; squote: &#39;; ampersand: &amp;; less: &lt;; greater: &gt;;</value></nested><empty></empty></a>",
        ),
        // 1185
        (
            1185,
            kids(
                "a",
                vec![
                    kids(
                        "nested",
                        vec![leaf("value", "newline: \n; cr: \r; tab: \t;")],
                    ),
                    kids("AnyField", vec![]),
                ],
            ),
            "<a><nested><value>newline: &#xA;; cr: &#xD;; tab: &#x9;;</value></nested><AnyField></AnyField></a>",
        ),
    ];
    for (line, node, want) in &rows {
        let got = marshal_indent(node, "", "");
        assert_eq!(
            String::from_utf8(got).unwrap(),
            *want,
            "marshal_test.go:{line}"
        );
    }
}

#[test]
fn escape_rules() {
    let mut dst = Vec::new();
    escape_text(&mut dst, b"a\nb\t\"'<&\x00\xE2\x82");
    assert_eq!(
        dst,
        "a\nb&#x9;&#34;&#39;&lt;&amp;\u{FFFD}\u{FFFD}\u{FFFD}".as_bytes()
    );

    let mut dst = Vec::new();
    escape_attr(&mut dst, b"\n");
    assert_eq!(dst, b"&#xA;");

    let n = el("node", &[("id", "x")], "", vec![]);
    assert_eq!(marshal_indent(&n, "", "  "), b"<node id=\"x\"></node>");
}
