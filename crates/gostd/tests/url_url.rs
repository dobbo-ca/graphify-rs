//! Ported from net/url/url_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2009 The Go Authors).
//! - TestParse (url_test.go:705): ported (`urltests` rows, every `Url` field compared)
//! - TestUnescape (url_test.go:1009): ported, path mode only (`unescapeTests` through `path_unescape`)
//! - TestParseFailure (url_test.go:1765): ported, adapted (Go calls `ParseQuery`; here `parse` fails the same way)
//! - TestParseErrors (url_test.go:1775): ported
//! - TestStarRequest (url_test.go:1843): ported, adapted (`RequestURI` is not ported; asserts the parse result it reads)
//! - TestURLHostnameAndPort (url_test.go:2004): ported (all 21 rows)
//! - TestNilUser (url_test.go:2100): ported, adapted (Go's nil `*Userinfo` methods are `Option`)
//! - TestInvalidUserPassword (url_test.go:2126): ported
//! - TestRejectControlCharacters (url_test.go:2133): ported, adapted ("\x80" is "\u{80}" in a `&str`)
//!
//! Not ported (20 of 29). graphify-go calls `url.Parse` (`security.go:66`) and `url.PathUnescape`
//! (`resolve.go:1367`) only, so these test code it never reaches:
//! - TestParseRequestURI (:784): `ParseRequestURI`; its shared authority rows are already in TestParseErrors
//! - TestURLString (:841), TestURLRedacted (:869): `URL.String`, `URL.Redacted`
//! - TestQueryEscape (:1069), TestPathEscape (:1122): `QueryEscape`, `PathEscape` (escaping)
//! - TestEncodeQuery (:1171), TestQueryValues (:1424): `Values.Encode`, `Values` methods
//! - TestParseQuery (:1552), TestParseQueryLimits (:1586): `ParseQuery`
//! - TestResolvePath (:1207), TestResolveReference (:1376), TestJoinPath (:2257): `resolvePath`,
//!   `ResolveReference`, `JoinPath`
//! - TestRequestURI (:1756): `URL.RequestURI`
//! - TestShouldEscape (:1915): `shouldEscape`, an escaping helper
//! - TestURLErrorImplementsNetError (:1987): `net.Error` (`Timeout`, `Temporary`), no Rust equivalent
//! - TestJSON (:2052), TestGob (:2079): JSON and gob marshalling of `URL`
//! - TestURLClone (:2432), TestValuesClone (:2507): `URL.Clone`, `Values.Clone`
//!
//! Other differences: `urltests` has 62 rows; the row with `in` "" (OmitHost, `http:%2F/host/path`) is
//! skipped by Go itself, so 61 run. `OmitHost` (row `mailto:/webmaster@golang.org`) is not a `Url`
//! field (only `String` reads it), so it is not compared.

use gostd::url::{Url, Userinfo, parse, path_unescape};

struct T {
    input: &'static str,
    scheme: &'static str,
    opaque: &'static str,
    user: Option<(&'static str, Option<&'static str>)>,
    host: &'static str,
    path: &'static str,
    raw_path: &'static str,
    force_query: bool,
    raw_query: &'static str,
    fragment: &'static str,
    raw_fragment: &'static str,
}

const E: T = T {
    input: "",
    scheme: "",
    opaque: "",
    user: None,
    host: "",
    path: "",
    raw_path: "",
    force_query: false,
    raw_query: "",
    fragment: "",
    raw_fragment: "",
};

// Go `urltests`; the `roundtrip` column is for `String`, which is not ported.
const URL_TESTS: &[T] = &[
    // no path
    T {
        input: "http://www.google.com",
        scheme: "http",
        host: "www.google.com",
        ..E
    },
    // path
    T {
        input: "http://www.google.com/",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        ..E
    },
    // path with hex escaping
    T {
        input: "http://www.google.com/file%20one%26two",
        scheme: "http",
        host: "www.google.com",
        path: "/file one&two",
        raw_path: "/file%20one%26two",
        ..E
    },
    // fragment with hex escaping
    T {
        input: "http://www.google.com/#file%20one%26two",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        fragment: "file one&two",
        raw_fragment: "file%20one%26two",
        ..E
    },
    // user
    T {
        input: "ftp://webmaster@www.google.com/",
        scheme: "ftp",
        user: Some(("webmaster", None)),
        host: "www.google.com",
        path: "/",
        ..E
    },
    // escape sequence in username
    T {
        input: "ftp://john%20doe@www.google.com/",
        scheme: "ftp",
        user: Some(("john doe", None)),
        host: "www.google.com",
        path: "/",
        ..E
    },
    // empty query
    T {
        input: "http://www.google.com/?",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        force_query: true,
        ..E
    },
    // query ending in question mark (Issue 14573)
    T {
        input: "http://www.google.com/?foo=bar?",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        raw_query: "foo=bar?",
        ..E
    },
    // query
    T {
        input: "http://www.google.com/?q=go+language",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        raw_query: "q=go+language",
        ..E
    },
    // query with hex escaping: NOT parsed
    T {
        input: "http://www.google.com/?q=go%20language",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        raw_query: "q=go%20language",
        ..E
    },
    // %20 outside query
    T {
        input: "http://www.google.com/a%20b?q=c+d",
        scheme: "http",
        host: "www.google.com",
        path: "/a b",
        raw_query: "q=c+d",
        ..E
    },
    // path without leading /, so no parsing
    T {
        input: "http:www.google.com/?q=go+language",
        scheme: "http",
        opaque: "www.google.com/",
        raw_query: "q=go+language",
        ..E
    },
    // path without leading /, so no parsing
    T {
        input: "http:%2f%2fwww.google.com/?q=go+language",
        scheme: "http",
        opaque: "%2f%2fwww.google.com/",
        raw_query: "q=go+language",
        ..E
    },
    // non-authority with path; see golang.org/issue/46059 (Go also sets OmitHost: true, not a field here)
    T {
        input: "mailto:/webmaster@golang.org",
        scheme: "mailto",
        path: "/webmaster@golang.org",
        ..E
    },
    // non-authority
    T {
        input: "mailto:webmaster@golang.org",
        scheme: "mailto",
        opaque: "webmaster@golang.org",
        ..E
    },
    // unescaped :// in query should not create a scheme
    T {
        input: "/foo?query=http://bad",
        path: "/foo",
        raw_query: "query=http://bad",
        ..E
    },
    // leading // without scheme should create an authority
    T {
        input: "//foo",
        host: "foo",
        ..E
    },
    // leading // without scheme, with userinfo, path, and query
    T {
        input: "//user@foo/path?a=b",
        user: Some(("user", None)),
        host: "foo",
        path: "/path",
        raw_query: "a=b",
        ..E
    },
    // Three leading slashes isn't an authority, but doesn't return an error.
    T {
        input: "///threeslashes",
        path: "///threeslashes",
        ..E
    },
    T {
        input: "http://user:password@google.com",
        scheme: "http",
        user: Some(("user", Some("password"))),
        host: "google.com",
        ..E
    },
    // unescaped @ in username should not confuse host
    T {
        input: "http://j@ne:password@google.com",
        scheme: "http",
        user: Some(("j@ne", Some("password"))),
        host: "google.com",
        ..E
    },
    // unescaped @ in password should not confuse host
    T {
        input: "http://jane:p@ssword@google.com",
        scheme: "http",
        user: Some(("jane", Some("p@ssword"))),
        host: "google.com",
        ..E
    },
    T {
        input: "http://j@ne:password@google.com/p@th?q=@go",
        scheme: "http",
        user: Some(("j@ne", Some("password"))),
        host: "google.com",
        path: "/p@th",
        raw_query: "q=@go",
        ..E
    },
    T {
        input: "http://www.google.com/?q=go+language#foo",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        raw_query: "q=go+language",
        fragment: "foo",
        ..E
    },
    T {
        input: "http://www.google.com/?q=go+language#foo&bar",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        raw_query: "q=go+language",
        fragment: "foo&bar",
        ..E
    },
    T {
        input: "http://www.google.com/?q=go+language#foo%26bar",
        scheme: "http",
        host: "www.google.com",
        path: "/",
        raw_query: "q=go+language",
        fragment: "foo&bar",
        raw_fragment: "foo%26bar",
        ..E
    },
    T {
        input: "file:///home/adg/rabbits",
        scheme: "file",
        host: "",
        path: "/home/adg/rabbits",
        ..E
    },
    // "Windows" paths are no exception to the rule.
    // See golang.org/issue/6027, especially comment #9.
    T {
        input: "file:///C:/FooBar/Baz.txt",
        scheme: "file",
        host: "",
        path: "/C:/FooBar/Baz.txt",
        ..E
    },
    // case-insensitive scheme
    T {
        input: "MaIlTo:webmaster@golang.org",
        scheme: "mailto",
        opaque: "webmaster@golang.org",
        ..E
    },
    // Relative path
    T {
        input: "a/b/c",
        path: "a/b/c",
        ..E
    },
    // escaped '?' in username and password
    T {
        input: "http://%3Fam:pa%3Fsword@google.com",
        scheme: "http",
        user: Some(("?am", Some("pa?sword"))),
        host: "google.com",
        ..E
    },
    // host subcomponent; IPv4 address in RFC 3986
    T {
        input: "http://192.168.0.1/",
        scheme: "http",
        host: "192.168.0.1",
        path: "/",
        ..E
    },
    // host and port subcomponents; IPv4 address in RFC 3986
    T {
        input: "http://192.168.0.1:8080/",
        scheme: "http",
        host: "192.168.0.1:8080",
        path: "/",
        ..E
    },
    // host subcomponent; IPv6 address in RFC 3986
    T {
        input: "http://[fe80::1]/",
        scheme: "http",
        host: "[fe80::1]",
        path: "/",
        ..E
    },
    // host and port subcomponents; IPv6 address in RFC 3986
    T {
        input: "http://[fe80::1]:8080/",
        scheme: "http",
        host: "[fe80::1]:8080",
        path: "/",
        ..E
    },
    // valid IPv6 host with port and path
    T {
        input: "https://[2001:db8::1]:8443/test/path",
        scheme: "https",
        host: "[2001:db8::1]:8443",
        path: "/test/path",
        ..E
    },
    // host subcomponent; IPv6 address with zone identifier in RFC 6874
    T {
        input: "http://[fe80::1%25en0]/", // alphanum zone identifier
        scheme: "http",
        host: "[fe80::1%en0]",
        path: "/",
        ..E
    },
    // host and port subcomponents; IPv6 address with zone identifier in RFC 6874
    T {
        input: "http://[fe80::1%25en0]:8080/", // alphanum zone identifier
        scheme: "http",
        host: "[fe80::1%en0]:8080",
        path: "/",
        ..E
    },
    // host subcomponent; IPv6 address with zone identifier in RFC 6874
    T {
        input: "http://[fe80::1%25%65%6e%301-._~]/", // percent-encoded+unreserved zone identifier
        scheme: "http",
        host: "[fe80::1%en01-._~]",
        path: "/",
        ..E
    },
    // host and port subcomponents; IPv6 address with zone identifier in RFC 6874
    T {
        input: "http://[fe80::1%25%65%6e%301-._~]:8080/", // percent-encoded+unreserved zone identifier
        scheme: "http",
        host: "[fe80::1%en01-._~]:8080",
        path: "/",
        ..E
    },
    // alternate escapings of path survive round trip
    T {
        input: "http://rest.rsc.io/foo%2fbar/baz%2Fquux?alt=media",
        scheme: "http",
        host: "rest.rsc.io",
        path: "/foo/bar/baz/quux",
        raw_path: "/foo%2fbar/baz%2Fquux",
        raw_query: "alt=media",
        ..E
    },
    // issue 12036
    T {
        input: "mysql://a,b,c/bar",
        scheme: "mysql",
        host: "a,b,c",
        path: "/bar",
        ..E
    },
    // worst case host, still round trips
    T {
        input: "scheme://!$&'()*+,;=hello!:1/path",
        scheme: "scheme",
        host: "!$&'()*+,;=hello!:1",
        path: "/path",
        ..E
    },
    // worst case path, still round trips
    T {
        input: "http://host/!$&'()*+,;=:@[hello]",
        scheme: "http",
        host: "host",
        path: "/!$&'()*+,;=:@[hello]",
        raw_path: "/!$&'()*+,;=:@[hello]",
        ..E
    },
    // golang.org/issue/5684
    T {
        input: "http://example.com/oid/[order_id]",
        scheme: "http",
        host: "example.com",
        path: "/oid/[order_id]",
        raw_path: "/oid/[order_id]",
        ..E
    },
    // golang.org/issue/12200 (colon with empty port)
    T {
        input: "http://192.168.0.2:8080/foo",
        scheme: "http",
        host: "192.168.0.2:8080",
        path: "/foo",
        ..E
    },
    T {
        input: "http://192.168.0.2:/foo",
        scheme: "http",
        host: "192.168.0.2:",
        path: "/foo",
        ..E
    },
    T {
        input: "http://[2b01:e34:ef40:7730:8e70:5aff:fefe:edac]:8080/foo",
        scheme: "http",
        host: "[2b01:e34:ef40:7730:8e70:5aff:fefe:edac]:8080",
        path: "/foo",
        ..E
    },
    T {
        input: "http://[2b01:e34:ef40:7730:8e70:5aff:fefe:edac]:/foo",
        scheme: "http",
        host: "[2b01:e34:ef40:7730:8e70:5aff:fefe:edac]:",
        path: "/foo",
        ..E
    },
    // golang.org/issue/7991 and golang.org/issue/12719 (non-ascii %-encoded in host)
    T {
        input: "http://hello.世界.com/foo",
        scheme: "http",
        host: "hello.世界.com",
        path: "/foo",
        ..E
    },
    T {
        input: "http://hello.%e4%b8%96%e7%95%8c.com/foo",
        scheme: "http",
        host: "hello.世界.com",
        path: "/foo",
        ..E
    },
    T {
        input: "http://hello.%E4%B8%96%E7%95%8C.com/foo",
        scheme: "http",
        host: "hello.世界.com",
        path: "/foo",
        ..E
    },
    // golang.org/issue/10433 (path beginning with //)
    T {
        input: "http://example.com//foo",
        scheme: "http",
        host: "example.com",
        path: "//foo",
        ..E
    },
    // test that we can reparse the host names we accept.
    T {
        input: "myscheme://authority<\"hi\">/foo",
        scheme: "myscheme",
        host: "authority<\"hi\">",
        path: "/foo",
        ..E
    },
    // spaces in hosts are disallowed but escaped spaces in IPv6 scope IDs are grudgingly OK.
    // This happens on Windows.
    // golang.org/issue/14002
    T {
        input: "tcp://[2020::2020:20:2020:2020%25Windows%20Loves%20Spaces]:2020",
        scheme: "tcp",
        host: "[2020::2020:20:2020:2020%Windows Loves Spaces]:2020",
        ..E
    },
    // test we can roundtrip magnet url
    // fix issue https://golang.org/issue/20054
    T {
        input: "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn",
        scheme: "magnet",
        host: "",
        path: "",
        raw_query: "xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn",
        ..E
    },
    T {
        input: "mailto:?subject=hi",
        scheme: "mailto",
        host: "",
        path: "",
        raw_query: "subject=hi",
        ..E
    },
    // PostgreSQL URLs can include a comma-separated list of host:post hosts.
    // https://go.dev/issue/75859
    T {
        input: "postgres://host1:1,host2:2,host3:3",
        scheme: "postgres",
        host: "host1:1,host2:2,host3:3",
        path: "",
        ..E
    },
    T {
        input: "postgresql://host1:1,host2:2,host3:3",
        scheme: "postgresql",
        host: "host1:1,host2:2,host3:3",
        path: "",
        ..E
    },
    // Mongodb URLs can include a comma-separated list of host:post hosts.
    T {
        input: "mongodb://user:password@host1:1,host2:2,host3:3",
        scheme: "mongodb",
        user: Some(("user", Some("password"))),
        host: "host1:1,host2:2,host3:3",
        path: "",
        ..E
    },
    T {
        input: "mongodb+srv://user:password@host1:1,host2:2,host3:3",
        scheme: "mongodb+srv",
        user: Some(("user", Some("password"))),
        host: "host1:1,host2:2,host3:3",
        path: "",
        ..E
    },
    // Go's last row (in "", OmitHost with Path "//host/path") is skipped by TestParse itself.
];

impl T {
    fn want(&self) -> Url {
        Url {
            scheme: self.scheme.to_string(),
            opaque: self.opaque.as_bytes().to_vec(),
            user: self.user.map(|(u, p)| Userinfo {
                username: u.as_bytes().to_vec(),
                password: p.map(|p| p.as_bytes().to_vec()),
            }),
            host: self.host.as_bytes().to_vec(),
            path: self.path.as_bytes().to_vec(),
            raw_path: self.raw_path.to_string(),
            force_query: self.force_query,
            raw_query: self.raw_query.to_string(),
            fragment: self.fragment.as_bytes().to_vec(),
            raw_fragment: self.raw_fragment.to_string(),
        }
    }
}

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_parse() {
    assert_eq!(URL_TESTS.len(), 61);
    for tt in URL_TESTS {
        match parse(tt.input) {
            Ok(u) => assert_eq!(u, tt.want(), "Parse({:?})", tt.input),
            Err(err) => panic!("Parse({:?}) returned error {err}", tt.input),
        }
    }
}

// (in, out, EscapeError text when Go's `err` is non-nil)
const UNESCAPE_TESTS: &[(&str, &str, Option<&str>)] = &[
    ("", "", None),
    ("abc", "abc", None),
    ("1%41", "1A", None),
    ("1%41%42%43", "1ABC", None),
    ("%4a", "J", None),
    ("%6F", "o", None),
    ("%", "", Some("%")),         // not enough characters after %
    ("%a", "", Some("%a")),       // not enough characters after %
    ("%1", "", Some("%1")),       // not enough characters after %
    ("123%45%6", "", Some("%6")), // not enough characters after %
    ("%zzzzz", "", Some("%zz")),  // invalid hex digits
    ("a+b", "a b", None),
    ("a%20b", "a b", None),
];

// Go checks `actual != out || (err != nil) != (tt.err != nil)`; the payload is checked too.
fn check_path_unescape(input: &str, out: &str, err: Option<&str>) {
    match (path_unescape(input), err) {
        (Ok(got), None) => assert_eq!(got, out.as_bytes(), "PathUnescape({input:?})"),
        (Err(got), Some(want)) => {
            assert_eq!(got.0, want.as_bytes(), "PathUnescape({input:?})");
            assert_eq!(
                out, "",
                "PathUnescape({input:?}): rows with an error have empty out"
            );
        }
        (got, want) => panic!("PathUnescape({input:?}) = {got:?}; want error {want:?}"),
    }
}

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_unescape() {
    for &(input, out, err) in UNESCAPE_TESTS {
        // Go also runs QueryUnescape here (not ported). With a "+", Go first checks the "+" as
        // "%20", then that PathUnescape keeps "+" (via the XXX round trip through QueryUnescape).
        let mut out = out.to_string();
        if input.contains('+') {
            check_path_unescape(&input.replace('+', "%20"), &out, err);
            if err.is_none() {
                let s = path_unescape(&input.replace('+', "XXX")).unwrap();
                out = String::from_utf8(s).unwrap().replace("XXX", "+");
            }
        }
        check_path_unescape(input, &out, err);
    }
}

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_parse_failure() {
    // Test that the first parse error is returned.
    const URL: &str = "%gh&%ij";
    let err = parse(URL).expect_err("Parse should fail");
    let err_str = err.to_string();
    assert!(
        err_str.contains("%gh"),
        "Parse({URL:?}) returned error {err_str:?}, want something containing \"%gh\""
    );
}

// (in, want_err)
const PARSE_ERRORS: &[(&str, bool)] = &[
    ("http://[::1]", false),
    ("http://[::1]:80", false),
    ("http://[::1]:namedport", true), // rfc3986 3.2.3
    ("http://x:namedport", true),     // rfc3986 3.2.3
    ("http://[::1]/", false),
    ("http://[::1]a", true),
    ("http://[::1]%23", true),
    ("http://[::1%25en0]", false),    // valid zone id
    ("http://[::1]:", false),         // colon, but no port OK
    ("http://x:", false),             // colon, but no port OK
    ("http://[::1]:%38%30", true),    // not allowed: % encoding only for non-ASCII
    ("http://[::1%25%41]", false),    // RFC 6874 allows over-escaping in zone
    ("http://[%10::1]", true),        // no %xx escapes in IP address
    ("http://[::1]/%48", false),      // %xx in path is fine
    ("http://%41:8080/", true),       // not allowed: % encoding only for non-ASCII
    ("mysql://x@y(z:123)/foo", true), // not well-formed per RFC 3986, golang.org/issue/33646
    ("mysql://x@y(1.2.3.4:123)/foo", true),
    (" http://foo.com", true),  // invalid character in schema
    ("ht tp://foo.com", true),  // invalid character in schema
    ("ahttp://foo.com", false), // valid schema characters
    ("1http://foo.com", true),  // invalid character in schema
    (
        "http://[]%20%48%54%54%50%2f%31%2e%31%0a%4d%79%48%65%61%64%65%72%3a%20%31%32%33%0a%0a/",
        true,
    ), // golang.org/issue/11208
    ("http://a b.com/", true),  // no space in host name please
    ("cache_object://foo", true), // scheme cannot have _, relative path cannot have : in first segment
    ("cache_object:foo", true),
    ("cache_object:foo/bar", true),
    ("cache_object/:foo/bar", false),
    ("http://[192.168.0.1]/", true),              // IPv4 in brackets
    ("http://[192.168.0.1]:8080/", true),         // IPv4 in brackets with port
    ("http://[::ffff:192.168.0.1]/", false),      // IPv4-mapped IPv6 in brackets
    ("http://[::ffff:192.168.0.1000]/", true),    // Out of range IPv4-mapped IPv6 in brackets
    ("http://[::ffff:192.168.0.1]:8080/", false), // IPv4-mapped IPv6 in brackets with port
    ("http://[::ffff:c0a8:1]/", false),           // IPv4-mapped IPv6 in brackets (hex)
    ("http://[not-an-ip]/", true),                // invalid IP string in brackets
    ("http://[fe80::1%foo]/", true),              // invalid zone format in brackets
    ("http://[fe80::1", true),                    // missing closing bracket
    ("http://fe80::1]/", true),                   // missing opening bracket
    ("http://[test.com]/", true),                 // domain name in brackets
    ("http://example.com[::1]", true),            // IPv6 literal doesn't start with '['
    ("http://example.com[::1", true),
    ("http://[::1", true),
    ("http://.[::1]", true),
    ("http:// [::1]", true),
    ("hxxp://mathepqo[.]serveftp(.)com:9059", true),
];

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_parse_errors() {
    for &(input, want_err) in PARSE_ERRORS {
        let res = parse(input);
        if want_err {
            assert!(res.is_err(), "Parse({input:?}) = {res:?}; want an error");
        } else if let Err(err) = res {
            panic!("Parse({input:?}) = {err}; want no error");
        }
    }
}

// Issue 11202
#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_star_request() {
    // Go checks `RequestURI() == "*"`, which is `EscapedPath` special-casing Path "*" (not escaped to %2A).
    let u = parse("*").unwrap();
    assert_eq!(u.path, b"*");
    assert_eq!(u.raw_path, "");
    assert_eq!(u.scheme, "");
    assert!(u.opaque.is_empty());
    assert!(u.host.is_empty());
}

// (Host field, Hostname, Port)
const HOSTNAME_AND_PORT: &[(&str, &str, &str)] = &[
    ("foo.com:80", "foo.com", "80"),
    ("foo.com", "foo.com", ""),
    ("foo.com:", "foo.com", ""),
    ("FOO.COM", "FOO.COM", ""), // no canonicalization
    ("1.2.3.4", "1.2.3.4", ""),
    ("1.2.3.4:80", "1.2.3.4", "80"),
    ("[1:2:3:4]", "1:2:3:4", ""),
    ("[1:2:3:4]:80", "1:2:3:4", "80"),
    ("[::1]:80", "::1", "80"),
    ("[::1]", "::1", ""),
    ("[::1]:", "::1", ""),
    ("localhost", "localhost", ""),
    ("localhost:443", "localhost", "443"),
    (
        "some.super.long.domain.example.org:8080",
        "some.super.long.domain.example.org",
        "8080",
    ),
    (
        "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:17000",
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
        "17000",
    ),
    (
        "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]",
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
        "",
    ),
    // Ensure that even when not valid, Host is one of "Hostname",
    // "Hostname:Port", "[Hostname]" or "[Hostname]:Port".
    // See https://golang.org/issue/29098.
    ("[google.com]:80", "google.com", "80"),
    ("google.com]:80", "google.com]", "80"),
    (
        "google.com:80_invalid_port",
        "google.com:80_invalid_port",
        "",
    ),
    ("[::1]extra]:80", "::1]extra", "80"),
    ("google.com]extra:extra", "google.com]extra:extra", ""),
];

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_url_hostname_and_port() {
    assert_eq!(HOSTNAME_AND_PORT.len(), 21);
    for &(input, host, port) in HOSTNAME_AND_PORT {
        let u = Url {
            host: input.as_bytes().to_vec(),
            ..Url::default()
        };
        assert_eq!(u.hostname(), host.as_bytes(), "Hostname for Host {input:?}");
        assert_eq!(u.port(), port.as_bytes(), "Port for Host {input:?}");
    }
}

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_nil_user() {
    // Go's nil *Userinfo answers Username() "", Password() ("", false), String() ""; here `user` is None.
    let u = parse("http://foo.com/").unwrap();
    assert_eq!(u.user, None);
}

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_invalid_user_password() {
    let err = parse("http://user^:passwo^rd@foo.com/").expect_err("Parse should fail");
    let got = err.to_string();
    assert!(
        got.contains("net/url: invalid userinfo"),
        "error = {got:?}; want substring \"net/url: invalid userinfo\""
    );
}

#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_reject_control_characters() {
    let tests = [
        "http://foo.com/?foo\nbar",
        "http\r://foo.com/",
        "http://foo\x7f.com/",
    ];
    for s in tests {
        const WANT_SUB: &str = "net/url: invalid control character in URL";
        let err = parse(s).expect_err("Parse should fail");
        let got = err.to_string();
        assert!(
            got.contains(WANT_SUB),
            "Parse({s:?}) error = {got:?}; want substring {WANT_SUB:?}"
        );
    }

    // But don't reject non-ASCII CTLs, at least for now. Go's "\x80" is one byte; a `&str` cannot
    // hold it, so this is U+0080 (C2 80): still a byte above 0x7f, which Go's CTL check ignores.
    if let Err(err) = parse("http://foo.com/ctl\u{80}") {
        panic!("error parsing URL with non-ASCII control byte: {err}");
    }
}

// Port of url_test.go:2410 TestParseStrictIpv6 (go1.27.4). graphify-go's go.mod says `go 1.25.0`,
// so its GODEBUG default is urlstrictcolons=0 (url.go:621): the port after the last colon, as for
// every non-http scheme. gostd ports that lenient parse, so these hosts parse without error.
#[test]
#[ignore = "graphify-rs-aa6.14"]
fn test_parse_strict_ipv6() {
    for input in [
        // Malformed URLs that used to parse.
        "https://1:2:3:4:5:6:7:8",
        "https://1:2:3:4:5:6:7:8:80",
        "https://example.com:80:",
    ] {
        if let Err(err) = parse(input) {
            panic!("Parse({input:?}) = {err}; want no error (urlstrictcolons=0)");
        }
    }
}
