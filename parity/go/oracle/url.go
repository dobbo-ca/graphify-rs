package main

import (
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"unicode/utf8"
)

// url: net/url Parse and PathUnescape. Inputs are 113 URLs from url_test.go (the `urltests`
// and TestParseErrors tables plus the strings of TestParseFailure, TestStarRequest,
// TestRejectControlCharacters, TestInvalidUserPassword and TestNilUser), 3,000 generated URLs
// `gNNNN` (schemes, userinfo, IPv4, IPv6 with and without %25 zones, bad ports, CTL bytes, ?,
// #, //, a colon in the first segment), and 2,000 `unescape-NNNN` %-strings. All are valid
// UTF-8 (gostd takes &str). Op `parse` (cases other than unescape-*) writes %q of Scheme,
// Hostname() and Port() on three lines, or the error text; op `path_unescape` (unescape-*)
// writes the bytes or the error text. Outputs: 113 + 3,000 + 2,000 = 5,113.

var urlTableCases = []string{
	"http://www.google.com",
	"http://www.google.com/",
	"http://www.google.com/file%20one%26two",
	"http://www.google.com/#file%20one%26two",
	"ftp://webmaster@www.google.com/",
	"ftp://john%20doe@www.google.com/",
	"http://www.google.com/?",
	"http://www.google.com/?foo=bar?",
	"http://www.google.com/?q=go+language",
	"http://www.google.com/?q=go%20language",
	"http://www.google.com/a%20b?q=c+d",
	"http:www.google.com/?q=go+language",
	"http:%2f%2fwww.google.com/?q=go+language",
	"mailto:/webmaster@golang.org",
	"mailto:webmaster@golang.org",
	"/foo?query=http://bad",
	"//foo",
	"//user@foo/path?a=b",
	"///threeslashes",
	"http://user:password@google.com",
	"http://j@ne:password@google.com",
	"http://jane:p@ssword@google.com",
	"http://j@ne:password@google.com/p@th?q=@go",
	"http://www.google.com/?q=go+language#foo",
	"http://www.google.com/?q=go+language#foo&bar",
	"http://www.google.com/?q=go+language#foo%26bar",
	"file:///home/adg/rabbits",
	"file:///C:/FooBar/Baz.txt",
	"MaIlTo:webmaster@golang.org",
	"a/b/c",
	"http://%3Fam:pa%3Fsword@google.com",
	"http://192.168.0.1/",
	"http://192.168.0.1:8080/",
	"http://[fe80::1]/",
	"http://[fe80::1]:8080/",
	"https://[2001:db8::1]:8443/test/path",
	"http://[fe80::1%25en0]/",
	"http://[fe80::1%25en0]:8080/",
	"http://[fe80::1%25%65%6e%301-._~]/",
	"http://[fe80::1%25%65%6e%301-._~]:8080/",
	"http://rest.rsc.io/foo%2fbar/baz%2Fquux?alt=media",
	"mysql://a,b,c/bar",
	"scheme://!$&'()*+,;=hello!:1/path",
	"http://host/!$&'()*+,;=:@[hello]",
	"http://example.com/oid/[order_id]",
	"http://192.168.0.2:8080/foo",
	"http://192.168.0.2:/foo",
	"http://[2b01:e34:ef40:7730:8e70:5aff:fefe:edac]:8080/foo",
	"http://[2b01:e34:ef40:7730:8e70:5aff:fefe:edac]:/foo",
	"http://hello.\u4e16\u754c.com/foo",
	"http://hello.%e4%b8%96%e7%95%8c.com/foo",
	"http://hello.%E4%B8%96%E7%95%8C.com/foo",
	"http://example.com//foo",
	"myscheme://authority<\"hi\">/foo",
	"tcp://[2020::2020:20:2020:2020%25Windows%20Loves%20Spaces]:2020",
	"magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn",
	"mailto:?subject=hi",
	"postgres://host1:1,host2:2,host3:3",
	"postgresql://host1:1,host2:2,host3:3",
	"mongodb://user:password@host1:1,host2:2,host3:3",
	"mongodb+srv://user:password@host1:1,host2:2,host3:3",
	// TestParseErrors (url_test.go:1775)
	"http://[]%20%48%54%54%50%2f%31%2e%31%0a%4d%79%48%65%61%64%65%72%3a%20%31%32%33%0a%0a/",
	"http://[::1]",
	"http://[::1]:80",
	"http://[::1]:namedport",
	"http://x:namedport",
	"http://[::1]/",
	"http://[::1]a",
	"http://[::1]%23",
	"http://[::1%25en0]",
	"http://[::1]:",
	"http://x:",
	"http://[::1]:%38%30",
	"http://[::1%25%41]",
	"http://[%10::1]",
	"http://[::1]/%48",
	"http://%41:8080/",
	"mysql://x@y(z:123)/foo",
	"mysql://x@y(1.2.3.4:123)/foo",
	" http://foo.com",
	"ht tp://foo.com",
	"ahttp://foo.com",
	"1http://foo.com",
	"http://a b.com/",
	"cache_object://foo",
	"cache_object:foo",
	"cache_object:foo/bar",
	"cache_object/:foo/bar",
	"http://[192.168.0.1]/",
	"http://[192.168.0.1]:8080/",
	"http://[::ffff:192.168.0.1]/",
	"http://[::ffff:192.168.0.1000]/",
	"http://[::ffff:192.168.0.1]:8080/",
	"http://[::ffff:c0a8:1]/",
	"http://[not-an-ip]/",
	"http://[fe80::1%foo]/",
	"http://[fe80::1",
	"http://fe80::1]/",
	"http://[test.com]/",
	"http://example.com[::1]",
	"http://example.com[::1",
	"http://[::1",
	"http://.[::1]",
	"http:// [::1]",
	"hxxp://mathepqo[.]serveftp(.)com:9059",
	// TestParseFailure, TestStarRequest, TestNilUser, TestInvalidUserPassword
	"%gh&%ij",
	"*",
	"http://foo.com/",
	"http://user^:passwo^rd@foo.com/",
	// TestRejectControlCharacters (the last is U+0080, a byte above 0x7f in UTF-8)
	"http://foo.com/?foo\nbar",
	"http\r://foo.com/",
	"http://foo\x7f.com/",
	"http://foo.com/ctl\u0080",
}

var (
	urlSchemes   = []string{"", "", "http", "https", "HTTP", "ftp", "mailto", "postgres", "mongodb+srv", "a1+-.", "1x", "ht tp", "cache_object", "file", "tcp", "_x"}
	urlSeps      = []string{"//", "//", "//", "", "/", "///"}
	urlUserinfos = []string{"", "", "", "user@", "user:pw@", "j@ne:p@ss@", "%3Fam:pa%3Fsw@", "us^er@", "a%zz@", ":@", "u:@", "%41@", "\u00e9@", "a:b:c@", "@"}
	urlHosts     = []string{
		"", "example.com", "EXAMPLE.com", "192.168.0.1", "[::1]", "[fe80::1]", "[fe80::1%25en0]",
		"[fe80::1%25%65%6e%30]", "[fe80::1%25]", "[fe80::1%en0]", "[::ffff:1.2.3.4]", "[1.2.3.4]",
		"[1:2:3:4:5:6:7:8]", "[1:2:3:4:5:6:7:8:9]", "[::1::2]", "[12345::1]", "[:1]", "[1::1.2.3]",
		"[fe80::1%25a b]", "[::1", "::1]", "a[::1]", "hello.\u4e16\u754c.com", "hello.%e4%b8%96.com",
		"a b.com", "%41.com", "h<>.com", "host1:1,host2:2", "[]", "[%25]", "[::1%25%zz]",
		"[::1%25%7f]", "[::01]", "[1.2.3.04]", "[::1.2.3.4.5]", "[0:0:0:0:0:0:0:0:0]", "[1:2:3:4:5:6:7::]",
		"[::1%25%ff]", "[::g]", "[%25en0]", "[fe80::1%25%41]", "[12:34]", "[a:b:c:d:e:f:1:2:3]",
	}
	urlPorts = []string{"", "", "", ":", ":80", ":8080", ":x", ":80x", ":-1", ":%38%30", "::1", ":99999999999"}
	urlPaths = []string{
		"", "/", "/a/b", "/a%20b", "/a%2fb", "/%zz", "/%", "/a b", "//x", "/[x]", "/%48",
		"/!$&'()*+,;=:@", "/\u00fc", "/%e4%b8%96", "/a:b", "x/y", "x:y", "x:y/z", "a%41", "opaque",
	}
	urlQueries   = []string{"", "", "", "?", "?a=b", "?a=b?", "?a=%zz", "??", "?a=c d"}
	urlFragments = []string{"", "", "", "#", "#f", "#f%20g", "#%zz", "#a&b", "#\u00e9", "#'", "#a#b"}
	urlCTLs      = "\x00\x01\x08\n\r\t\x1f\x7f"
	urlMutations = " %[]@:/?#\"<>^"
	urlEscapes   = []string{
		"%", "%4", "%41", "%zz", "%2F", "%2f", "%25", "%00", "%ff", "%C3%A9", "%e4%b8%96", "+", "a",
		"\u00e9", "%\u00e9", "%a\u00e9", "%4\u00e9", " ", "/", "%%", "%0", "abc", "%4g", "%g4", "?", "#",
	}
)

func init() {
	register("url", 5113, func(dir string) error {
		write := func(name, s string) error {
			if !utf8.ValidString(s) {
				return fmt.Errorf("%s: input is not valid UTF-8", name)
			}
			return os.WriteFile(filepath.Join(dir, name), []byte(s), 0o644)
		}
		for i, s := range urlTableCases {
			if err := write(fmt.Sprintf("t%03d", i), s); err != nil {
				return err
			}
		}
		pick := func(r *rng, a []string) string { return a[r.intn(len(a))] }
		for i := 0; i < 3000; i++ {
			name := fmt.Sprintf("g%04d", i)
			r := seed("url/" + name)
			var b strings.Builder
			if scheme := pick(r, urlSchemes); scheme != "" {
				b.WriteString(scheme + ":")
			}
			sep := pick(r, urlSeps)
			b.WriteString(sep)
			if sep == "//" {
				b.WriteString(pick(r, urlUserinfos))
				b.WriteString(pick(r, urlHosts))
				b.WriteString(pick(r, urlPorts))
			}
			b.WriteString(pick(r, urlPaths))
			b.WriteString(pick(r, urlQueries))
			b.WriteString(pick(r, urlFragments))
			runes := []rune(b.String())
			// About 1 in 8 gets a CTL byte and 1 in 4 an extra delimiter, at a rune boundary.
			if r.intn(8) == 0 {
				at := r.intn(len(runes) + 1)
				runes = append(runes[:at], append([]rune{rune(urlCTLs[r.intn(len(urlCTLs))])}, runes[at:]...)...)
			}
			if r.intn(4) == 0 {
				at := r.intn(len(runes) + 1)
				runes = append(runes[:at], append([]rune{rune(urlMutations[r.intn(len(urlMutations))])}, runes[at:]...)...)
			}
			if err := write(name, string(runes)); err != nil {
				return err
			}
		}
		for i := 0; i < 2000; i++ {
			name := fmt.Sprintf("unescape-%04d", i)
			r := seed("url/" + name)
			var b strings.Builder
			for n := r.intn(9); n > 0; n-- {
				b.WriteString(pick(r, urlEscapes))
			}
			if err := write(name, b.String()); err != nil {
				return err
			}
		}
		return nil
	}, func(name string, in []byte) []op {
		if strings.HasPrefix(name, "unescape-") {
			s, err := url.PathUnescape(string(in))
			if err != nil {
				return []op{{"path_unescape", result{err: err.Error()}}}
			}
			return []op{{"path_unescape", result{out: []byte(s)}}}
		}
		u, err := url.Parse(string(in))
		if err != nil {
			return []op{{"parse", result{err: err.Error()}}}
		}
		out := strconv.Quote(u.Scheme) + "\n" + strconv.Quote(u.Hostname()) + "\n" + strconv.Quote(u.Port()) + "\n"
		return []op{{"parse", result{out: []byte(out)}}}
	})
}
