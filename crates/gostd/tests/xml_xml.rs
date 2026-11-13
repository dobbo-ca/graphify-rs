//! Ported from encoding/xml/xml_test.go (GOSRC/encoding/xml/xml_test.go, Go go1.27.4).
//! Source licence: BSD-3-Clause (Copyright 2009 The Go Authors).
//!
//! The Go decoder is driven through the test-only token API in
//! `gostd::xml` (strict, no CharsetReader). 42 Test funcs in the source,
//! 21 ported.
//!
//! Not ported:
//! - TestDecodeEOF: not reached by graphify-go (NewTokenDecoder, Decoder.Decode)
//! - TestDecodeNilToken: not reached by graphify-go (NewTokenDecoder, non-strict mode)
//! - TestNonStrictRawToken: not reached by graphify-go (non-strict mode)
//! - TestRawTokenAltEncoding: not reached by graphify-go (CharsetReader)
//! - TestAllScalars: not reached by graphify-go (Unmarshal into scalar struct fields)
//! - TestIssue569: not reached by graphify-go (Unmarshal into a named struct)
//! - TestUnquotedAttrs: not reached by graphify-go (non-strict mode)
//! - TestValuelessAttrs: not reached by graphify-go (non-strict mode)
//! - TestCopyTokenCharData: not reached by graphify-go (CopyToken)
//! - TestCopyTokenStartElement: not reached by graphify-go (CopyToken)
//! - TestCopyTokenComment: not reached by graphify-go (CopyToken)
//! - TestEscapeTextIOErrors: not reached by graphify-go (io.Writer EscapeText)
//! - TestIssue5880: not reached by graphify-go (Marshal of a byte slice)
//! - TestIssue8535: not reached by graphify-go (Decode into a struct with namespaced tags)
//! - TestEncodeXMLNS: not reached by graphify-go (Marshal with xmlns struct tags)
//! - TestIssue7113: not reached by graphify-go (struct XMLName round trip)
//! - TestNewTokenDecoderIdempotent: not reached by graphify-go (NewTokenDecoder)
//! - TestWrapDecoder: not reached by graphify-go (NewTokenDecoder)
//! - TestTokenUnmarshaler: not reached by graphify-go (NewTokenDecoder, UnmarshalXML)
//! - TestRoundTrip: not reached by graphify-go (EncodeToken, CopyToken)
//! - TestHTMLAutoClose: not reached by graphify-go (non-strict mode, AutoClose, HTMLEntity)
//!
//! Adaptations: TestIsInCharacterRange passes u32::MAX for Go's rune -1.
//! TestIssue20396 calls unmarshal_element where Go unmarshals into a string.

use gostd::xml::{
    Attr, Name, Token, escape_text, input_pos_after_tokens, is_in_character_range, proc_inst,
    raw_token_spans, raw_tokens, tokens, tokens_partial, tokens_with_entity, unmarshal_element,
};

fn nm(space: &str, local: &str) -> Name {
    Name {
        space: space.to_string(),
        local: local.to_string(),
    }
}

fn start(space: &str, local: &str, attr: &[(&str, &str, &str)]) -> Token {
    Token::StartElement {
        name: nm(space, local),
        attr: attr
            .iter()
            .map(|&(s, l, v)| Attr {
                name: nm(s, l),
                value: v.to_string(),
            })
            .collect(),
    }
}

fn end(space: &str, local: &str) -> Token {
    Token::EndElement {
        name: nm(space, local),
    }
}

fn cd(s: &str) -> Token {
    Token::CharData(s.as_bytes().to_vec())
}

fn directive(s: &str) -> Token {
    Token::Directive(s.as_bytes().to_vec())
}

const TEST_INPUT: &str = concat!(
    r##"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<body xmlns:foo="ns1" xmlns="ns2" xmlns:tag="ns3" "##,
    "\r\n\t",
    r##"  >
  <hello lang="en">World &lt;&gt;&apos;&quot; &#x767d;&#40300;翔</hello>
  <query>&何; &is-it;</query>
  <goodbye />
  <outer foo:attr="value" xmlns:tag="ns4">
    <inner/>
  </outer>
  <tag:name>
    <![CDATA[Some text here.]]>
  </tag:name>
</body><!-- missing final newline -->"##
);

const TEST_ENTITY: [(&str, &str); 2] = [("何", "What"), ("is-it", "is it?")];

fn cooked_tokens() -> Vec<Token> {
    vec![
        cd("\n"),
        Token::ProcInst {
            target: "xml".to_string(),
            inst: br#"version="1.0" encoding="UTF-8""#.to_vec(),
        },
        cd("\n"),
        directive(
            r##"DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd""##,
        ),
        cd("\n"),
        start(
            "ns2",
            "body",
            &[
                ("xmlns", "foo", "ns1"),
                ("", "xmlns", "ns2"),
                ("xmlns", "tag", "ns3"),
            ],
        ),
        cd("\n  "),
        start("ns2", "hello", &[("", "lang", "en")]),
        cd("World <>'\" 白鵬翔"),
        end("ns2", "hello"),
        cd("\n  "),
        start("ns2", "query", &[]),
        cd("What is it?"),
        end("ns2", "query"),
        cd("\n  "),
        start("ns2", "goodbye", &[]),
        end("ns2", "goodbye"),
        cd("\n  "),
        start(
            "ns2",
            "outer",
            &[("ns1", "attr", "value"), ("xmlns", "tag", "ns4")],
        ),
        cd("\n    "),
        start("ns2", "inner", &[]),
        end("ns2", "inner"),
        cd("\n  "),
        end("ns2", "outer"),
        cd("\n  "),
        start("ns3", "name", &[]),
        cd("\n    "),
        cd("Some text here."),
        cd("\n  "),
        end("ns3", "name"),
        cd("\n"),
        end("ns2", "body"),
        Token::Comment(b" missing final newline ".to_vec()),
    ]
}

const TEST_INPUT_ALT_ENCODING: &str = r##"
<?xml version="1.0" encoding="x-testing-uppercase"?>
<TAG>VALUE</TAG>"##;

const XML_INPUT: [&str; 39] = [
    // unexpected EOF cases
    "<",
    "<t",
    "<t ",
    "<t/",
    "<!",
    "<!-",
    "<!--",
    "<!--c-",
    "<!--c--",
    "<!d",
    "<t></",
    "<t></t",
    "<?",
    "<?p",
    "<t a",
    "<t a=",
    "<t a='",
    "<t a=''",
    "<t/><![",
    "<t/><![C",
    "<t/><![CDATA[d",
    "<t/><![CDATA[d]",
    "<t/><![CDATA[d]]",
    // other Syntax errors
    "<>",
    "<t/a",
    "<0 />",
    "<?0 >",
    //	"<!0 >",	// let the Token() caller handle
    "</0>",
    "<t 0=''>",
    "<t a='&'>",
    "<t a='<'>",
    "<t>&nbspc;</t>",
    "<t a>",
    "<t a=>",
    "<t a=v>",
    //	"<![CDATA[d]]>",	// let the Token() caller handle
    "<t></e>",
    "<t></>",
    "<t></t!",
    "<t>cdata]]></t>",
];

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_raw_token_alt_encoding_no_converter() {
    // Go: first RawToken returns a token, second returns nil and an error
    // naming the encoding.
    let (toks, err) = tokens_partial(TEST_INPUT_ALT_ENCODING.as_bytes(), true);
    assert_eq!(toks, vec![cd("\n")], "expected one token before the error");
    let err = err.expect("expected an error on second RawToken call");
    const ENCODING: &str = "x-testing-uppercase";
    assert!(
        err.to_string().contains(ENCODING),
        "expected error to contain {ENCODING:?}; got error: {err}"
    );
}

const NESTED_DIRECTIVES_INPUT: &str = r##"
<!DOCTYPE [<!ENTITY rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#">]>
<!DOCTYPE [<!ENTITY xlt ">">]>
<!DOCTYPE [<!ENTITY xlt "<">]>
<!DOCTYPE [<!ENTITY xlt '>'>]>
<!DOCTYPE [<!ENTITY xlt '<'>]>
<!DOCTYPE [<!ENTITY xlt '">'>]>
<!DOCTYPE [<!ENTITY xlt "'<">]>
"##;

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_nested_directives() {
    let want = vec![
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#">]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY xlt ">">]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY xlt "<">]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY xlt '>'>]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY xlt '<'>]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY xlt '">'>]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY xlt "'<">]"##),
        cd("\n"),
    ];
    let have = tokens(NESTED_DIRECTIVES_INPUT.as_bytes()).expect("unexpected error");
    assert_eq!(have, want);
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_token() {
    let have = tokens_with_entity(TEST_INPUT.as_bytes(), &TEST_ENTITY).expect("unexpected error");
    let want = cooked_tokens();
    for (i, w) in want.iter().enumerate() {
        assert_eq!(have.get(i), Some(w), "token {i}");
    }
}
fn raw_tokens_table() -> Vec<Token> {
    vec![
        cd("\n"),
        Token::ProcInst {
            target: "xml".to_string(),
            inst: br#"version="1.0" encoding="UTF-8""#.to_vec(),
        },
        cd("\n"),
        directive(
            r##"DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd""##,
        ),
        cd("\n"),
        start(
            "",
            "body",
            &[
                ("xmlns", "foo", "ns1"),
                ("", "xmlns", "ns2"),
                ("xmlns", "tag", "ns3"),
            ],
        ),
        cd("\n  "),
        start("", "hello", &[("", "lang", "en")]),
        cd("World <>'\" 白鵬翔"),
        end("", "hello"),
        cd("\n  "),
        start("", "query", &[]),
        cd("What is it?"),
        end("", "query"),
        cd("\n  "),
        start("", "goodbye", &[]),
        end("", "goodbye"),
        cd("\n  "),
        start(
            "",
            "outer",
            &[("foo", "attr", "value"), ("xmlns", "tag", "ns4")],
        ),
        cd("\n    "),
        start("", "inner", &[]),
        end("", "inner"),
        cd("\n  "),
        end("", "outer"),
        cd("\n  "),
        start("tag", "name", &[]),
        cd("\n    "),
        cd("Some text here."),
        cd("\n  "),
        end("tag", "name"),
        cd("\n"),
        end("", "body"),
        Token::Comment(b" missing final newline ".to_vec()),
    ]
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_raw_token() {
    let raw = TEST_INPUT;
    let have = raw_token_spans(raw.as_bytes(), &TEST_ENTITY).expect("unexpected error");
    let mut last_end = 0i64;
    for (i, want) in raw_tokens_table().iter().enumerate() {
        let (tok, start, end) = have.get(i).unwrap_or_else(|| panic!("token {i}: missing"));
        assert_eq!(tok, want, "token {i}");
        // Check that InputOffset returned actual token.
        if *start < last_end {
            panic!("token {i}: position [{start},{end}) for {tok:?} is before previous token");
        } else if *start >= *end {
            // Special case: EndElement can be synthesized.
            if *start != *end || *end != last_end {
                panic!("token {i}: position [{start},{end}) for {tok:?} is empty");
            }
        } else if *end > raw.len() as i64 {
            panic!("token {i}: position [{start},{end}) for {tok:?} extends beyond input");
        } else {
            let text = &raw[*start as usize..*end as usize];
            if text.contains(['<', '>']) && (!text.starts_with('<') || !text.ends_with('>')) {
                panic!("token {i}: misaligned raw token {text:?} for {tok:?}");
            }
        }
        last_end = *end;
    }
}

const PROC_INST_TESTS: [(&str, [&str; 2]); 11] = [
    (r#"version="1.0" encoding="utf-8""#, ["1.0", "utf-8"]),
    (r#"version="1.0" encoding='utf-8'"#, ["1.0", "utf-8"]),
    (r#"version="1.0" encoding='utf-8' "#, ["1.0", "utf-8"]),
    (r#"version="1.0" encoding=utf-8"#, ["1.0", ""]),
    (r#"encoding="FOO" "#, ["", "FOO"]),
    (
        r#"version=2.0 version="1.0" encoding=utf-7 encoding='utf-8'"#,
        ["1.0", "utf-8"],
    ),
    (r#"version= encoding="#, ["", ""]),
    (r#"encoding="version=1.0""#, ["", "version=1.0"]),
    ("", ["", ""]),
    // TODO: what's the right approach to handle these nested cases?
    (r#"encoding="version='1.0'""#, ["1.0", "version='1.0'"]),
    (
        r#"version="encoding='utf-8'""#,
        ["encoding='utf-8'", "utf-8"],
    ),
];

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_proc_inst_encoding() {
    for (input, expect) in PROC_INST_TESTS {
        assert_eq!(
            proc_inst("version", input),
            expect[0],
            "proc_inst(version, {input:?})"
        );
        assert_eq!(
            proc_inst("encoding", input),
            expect[1],
            "proc_inst(encoding, {input:?})"
        );
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_syntax() {
    for input in XML_INPUT {
        assert!(
            tokens(input.as_bytes()).is_err(),
            r#"xmlInput "{input}": expected SyntaxError not received"#
        );
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_input_line_pos() {
    let test_input = "<root>
<?pi
 ?>  <elt
att
=
\"val\">
<![CDATA[
]]><!--

--></elt>
</root>";
    let line_pos = vec![
        (1, 7),
        (2, 1),
        (3, 4),
        (3, 6),
        (6, 7),
        (7, 1),
        (8, 4),
        (10, 4),
        (10, 10),
        (11, 1),
        (11, 8),
    ];
    let got = input_pos_after_tokens(test_input.as_bytes()).expect("unexpected error");
    assert_eq!(got, line_pos);
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_issue_68387() {
    let got = raw_tokens(b"<item b=']]>'/>").expect("RawToken() failed");
    assert_eq!(
        got,
        vec![start("", "item", &[("", "b", "]]>")]), end("", "item")]
    );
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_syntax_error_line_num() {
    let (_, err) = tokens_partial(b"<P>Foo<P>\n\n<P>Bar</>\n", false);
    let synerr = err.expect("Expected SyntaxError.");
    assert_eq!(
        synerr.line, 3,
        "SyntaxError didn't have correct line number."
    );
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_trailing_raw_token() {
    let (_, err) = tokens_partial(b"<FOO></FOO>  ", true);
    assert_eq!(err, None, "d.RawToken() want io.EOF");
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_trailing_token() {
    let (_, err) = tokens_partial(b"<FOO></FOO>  ", false);
    assert_eq!(err, None, "d.Token() want io.EOF");
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_entity_inside_cdata() {
    let (_, err) = tokens_partial(b"<test><![CDATA[ &val=foo ]]></test>", false);
    assert_eq!(err, None, "d.Token() want io.EOF");
}

const CHARACTER_TESTS: [(&[u8], &str); 9] = [
    (b"\x12<doc/>", "illegal character code U+0012"),
    (
        b"<?xml version=\"1.0\"?>\x0b<doc/>",
        "illegal character code U+000B",
    ),
    (b"\xef\xbf\xbe<doc/>", "illegal character code U+FFFE"),
    (
        b"<?xml version=\"1.0\"?><doc>\r\n<hiya/>\x07<toots/></doc>",
        "illegal character code U+0007",
    ),
    (
        b"<?xml version=\"1.0\"?><doc \x12='value'>what's up</doc>",
        "expected attribute name in element",
    ),
    (
        b"<doc>&abc\x01;</doc>",
        "invalid character entity &abc (no semicolon)",
    ),
    (
        b"<doc>&\x01;</doc>",
        "invalid character entity & (no semicolon)",
    ),
    (
        b"<doc>&\xef\xbf\xbe;</doc>",
        "invalid character entity &\u{FFFE};",
    ),
    (b"<doc>&hello;</doc>", "invalid character entity &hello;"),
];

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_disallowed_characters() {
    for (i, (input, want)) in CHARACTER_TESTS.iter().enumerate() {
        let (_, err) = tokens_partial(input, false);
        let synerr = err.unwrap_or_else(|| panic!("input {i}: want *SyntaxError, got EOF"));
        assert_eq!(synerr.msg, *want, "input {i} synerr.Msg wrong");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_is_in_character_range() {
    // utf8.MaxRune+1, surrogate min, surrogate max, and u32::MAX for Go's -1.
    for r in [0x110000u32, 0xD800, 0xDFFF, u32::MAX] {
        assert!(!is_in_character_range(r), "rune {r:#X} considered valid");
    }
}

const DIRECTIVES_WITH_COMMENTS_INPUT: &str = r##"
<!DOCTYPE [<!-- a comment --><!ENTITY rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#">]>
<!DOCTYPE [<!ENTITY go "Golang"><!-- a comment-->]>
<!DOCTYPE <!-> <!> <!----> <!-->--> <!--->--> [<!ENTITY go "Golang"><!-- a comment-->]>
"##;

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_directives_with_comments() {
    let want = vec![
        cd("\n"),
        directive(r##"DOCTYPE [ <!ENTITY rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#">]"##),
        cd("\n"),
        directive(r##"DOCTYPE [<!ENTITY go "Golang"> ]"##),
        cd("\n"),
        directive(r##"DOCTYPE <!-> <!>       [<!ENTITY go "Golang"> ]"##),
        cd("\n"),
    ];
    let have = tokens(DIRECTIVES_WITH_COMMENTS_INPUT.as_bytes()).expect("unexpected error");
    assert_eq!(have, want);
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_escape_text_invalid_char() {
    let input = b"A \x00 terminated string.";
    let expected = "A \u{FFFD} terminated string.";
    let mut dst = Vec::new();
    escape_text(&mut dst, input);
    assert_eq!(dst, expected.as_bytes());
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_issue_11405() {
    for tc in ["<root>", "<root><foo>", "<root><foo></foo>"] {
        assert!(tokens(tc.as_bytes()).is_err(), "{tc}: want SyntaxError");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_issue_12417() {
    let test_cases = [
        (r#"<?xml encoding="UtF-8" version="1.0"?><root/>"#, true),
        (r#"<?xml encoding="UTF-8" version="1.0"?><root/>"#, true),
        (r#"<?xml encoding="utf-8" version="1.0"?><root/>"#, true),
        (r#"<?xml encoding="uuu-9" version="1.0"?><root/>"#, false),
    ];
    for (s, ok) in test_cases {
        assert_eq!(tokens(s.as_bytes()).is_ok(), ok, "{s:?}: Encoding charset");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_issue_20396() {
    const ATTR_ERROR: &str = "XML syntax error on line 1: expected attribute name in element";
    const NAME_ERROR: &str = "XML syntax error on line 1: expected element name after <";
    let test_cases: [(&str, Option<&str>); 8] = [
        (r#"<a:te:st xmlns:a="abcd"/>"#, Some(NAME_ERROR)),
        (r#"<a:te=st xmlns:a="abcd"/>"#, Some(ATTR_ERROR)),
        (r#"<a:te&st xmlns:a="abcd"/>"#, Some(ATTR_ERROR)),
        (r#"<a:test xmlns:a="abcd"/>"#, None),
        (r#"<a:te:st xmlns:a="abcd">1</a:te:st>"#, Some(NAME_ERROR)),
        (r#"<a:te=st xmlns:a="abcd">1</a:te=st>"#, Some(ATTR_ERROR)),
        (r#"<a:te&st xmlns:a="abcd">1</a:te&st>"#, Some(ATTR_ERROR)),
        (r#"<a:test xmlns:a="abcd">1</a:test>"#, None),
    ];
    for (s, want) in test_cases {
        let got = unmarshal_element(s.as_bytes()).err().map(|e| e.to_string());
        assert_eq!(got.as_deref(), want, "{s}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_issue_20685() {
    let test_cases = [
        (
            r#"<x:book xmlns:x="abcd" xmlns:y="abcd"><unclosetag>one</x:book>"#,
            false,
        ),
        (
            r#"<x:book xmlns:x="abcd" xmlns:y="abcd">one</x:book>"#,
            true,
        ),
        (
            r#"<x:book xmlns:x="abcd" xmlns:y="abcd">one</y:book>"#,
            false,
        ),
        (
            r#"<x:book xmlns:y="abcd" xmlns:x="abcd">one</y:book>"#,
            false,
        ),
        (r#"<x:book xmlns:x="abcd">one</y:book>"#, false),
        (r#"<x:book>one</y:book>"#, false),
        (r#"<xbook>one</ybook>"#, false),
    ];
    for (s, ok) in test_cases {
        assert_eq!(
            tokens(s.as_bytes()).is_ok(),
            ok,
            "{s:?}: Closing tag with namespace"
        );
    }
}

#[test]
#[ignore = "graphify-rs-aa6.12"]
fn test_parse_errors() {
    let with_default_header = |s: &str| format!(r#"<?xml version="1.0" encoding="UTF-8"?>{s}"#);
    let tests: Vec<(Vec<u8>, &str)> = vec![
        (
            with_default_header("</foo>").into_bytes(),
            "unexpected end element </foo>",
        ),
        (
            with_default_header("<x:foo></y:foo>").into_bytes(),
            "element <foo> in space x closed by </foo> in space y",
        ),
        (
            with_default_header("<? not ok ?>").into_bytes(),
            "expected target name after <?",
        ),
        (
            with_default_header("<!- not ok -->").into_bytes(),
            "invalid sequence <!- not part of <!--",
        ),
        (
            with_default_header("<!-? not ok -->").into_bytes(),
            "invalid sequence <!- not part of <!--",
        ),
        (
            with_default_header("<![not ok]>").into_bytes(),
            "invalid <![ sequence",
        ),
        (
            with_default_header(r#"<zzz:foo xmlns:zzz="http://example.com"><bar>baz</bar></foo>"#)
                .into_bytes(),
            r#"element <foo> in space zzz closed by </foo> in space """#,
        ),
        (
            [with_default_header("").into_bytes(), vec![0xf1]].concat(),
            "invalid UTF-8",
        ),
        // Header-related errors.
        (
            br#"<?xml version="1.1" encoding="UTF-8"?>"#.to_vec(),
            r#"unsupported version "1.1"; only version 1.0 is supported"#,
        ),
        // Cases below are for "no errors".
        (with_default_header("<?ok?>").into_bytes(), ""),
        (
            with_default_header(r#"<?ok version="ok"?>"#).into_bytes(),
            "",
        ),
    ];
    for (src, want) in &tests {
        let src_s = String::from_utf8_lossy(src);
        match tokens(src) {
            Ok(_) => assert_eq!(*want, "", "parse {src_s}: unexpected EOF"),
            Err(e) => {
                assert_ne!(*want, "", "parse {src_s}: have {e:?} error, expected none");
                assert!(
                    e.to_string().contains(want),
                    "parse {src_s}: can't find {want:?} error substring\nerror: {e:?}"
                );
            }
        }
    }
}
