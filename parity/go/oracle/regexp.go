package main

import (
	"encoding/binary"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"unicode/utf8"
)

// regexp: Go regexp against gostd::regexp (crates/gostd/examples/oracle/regexp.rs writes the
// same bytes; keep the two in step).
//
// Inputs. One file per pattern, c00..c42 (the constant patterns of graphify-go), i00..i42 (the
// same, prefixed "(?i)" as query.go does with user input) and g0000..g1999 (generated; every
// tenth is invalid). Each file is length-prefixed records (4-byte big-endian length, then the
// bytes): record 0 is the pattern (valid UTF-8), records 1..N are subjects (arbitrary bytes). The
// input "quote_meta" holds only subjects: every ASCII byte string of length 1, then 1,000
// generated valid UTF-8 strings.
//
// Outputs. One line per subject for each op.
//
//	compile                 "ok", or the Compile error text (an .err file, as for any error)
//	match                   MatchString: "1" or "0"
//	find_index              FindStringIndex: "lo,hi" or "none"
//	find_submatch           FindStringSubmatchIndex: "none" or "[g g ...]"
//	find_all                FindAllString, n = -1, 0, 2: "n=-1 3 q q q ; n=0 0 ; n=2 2 q q"
//	find_all_index          FindAllStringIndex, n = -1: "2 lo,hi lo,hi"
//	find_all_submatch       FindAllStringSubmatchIndex, n = -1: "2 [g g] [g g]"
//	bytes_submatch_index    FindSubmatchIndex on the bytes: "none" or "[lo,hi unmatched ...]"
//	bytes_all_submatch_index  FindAllSubmatchIndex, n = -1: "2 [lo,hi unmatched] [lo,hi unmatched]"
//	replace                 ReplaceAllString with "_", "", "$1x", "${1}", "$$", "${name}": six q, space apart
//	replace_func            ReplaceAllStringFunc with "<" + match + ">": one q
//	split                   Split, n = -1, 0, 3, as find_all
//	quote_meta              QuoteMeta: one q per subject
//
// A subject that is not valid UTF-8 gets the line "-" in every op that takes a string. A group g
// is q(text), or "unmatched" when its index is -1. q is the text in double quotes with each byte
// outside 0x20..0x7e, and each " and \, written \xHH (lower case hex); it does not depend on Go's
// %q. A line ends with \n.

// regexpConstants are the constant patterns of graphify-go, in the order of the call-site table
// (save_result.go, serve.go, security.go, detect.go, manifest.go, resolve.go, cpp.go, terraform.go,
// python.go, markdown.go, mcpconfig.go, javascript.go, csharp_resolve.go, component.go, ask.go,
// idutil.go), less the gitignore.go ones built at run time, the QuoteMeta call and query.go.
var regexpConstants = []string{
	`[^0-9a-zA-Z_]+`,
	`^[0-9a-f]{7,40}$`,
	`[\x00-\x1f\x7f]`,
	`(?i)(^|[\\/])\.(env|envrc)(\.|$)`,
	`(?i)\.(pem|key|p12|pfx|cert|crt|der|p8)$`,
	`(id_rsa|id_dsa|id_ecdsa|id_ed25519)(\.pub)?$`,
	`(?i)(\.netrc|\.pgpass|\.htpasswd)$`,
	`(?i)(?:credential|secret|passwd|password|private_key)s?`,
	`[-_\s]+`,
	`^module\s+(\S+)`,
	`^require\s*\(`,
	`^(\S+)\s+v\S+`,
	`^require\s+(\S+)\s+v\S+`,
	`[\s<>=!~;\[\(]`,
	`\$\{([^}]+)\}`,
	`^[A-Za-z_][A-Za-z0-9_]*(?:(?:::|\.)[A-Za-z_][A-Za-z0-9_]*)*$`,
	`::|\.`,
	`^(?:class|struct)\s+((?:[A-Z][A-Z0-9_]*\s+)+)([A-Za-z_]\w*(?:\s+final)?\s*)([:{])`,
	`(?i)(password|passwd|secret|token|api[-_]?key|access[-_]?key|private[-_]?key|credential|client[-_]?secret|connection[-_]?string|sas[-_]?token|auth|bearer|passphrase)`,
	`://[^/@\s]*:[^@\s]+@`,
	`^[A-Za-z_][\w.\[\]*-]*$`,
	`^[A-Z]\w*$`,
	`(?m)^revision\s*[:=]`,
	`\[[^\]]*\]\((?:<([^>]+)>|([^)\s]+))`,
	`^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?`,
	`\[\[([^\]|#\\]+)\\?(?:[#|][^\]]*)?\]\]`,
	`^(#{1,6})\s+(.+)`,
	"`([^`\n]+)`",
	`^@[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*(?:@[\w.\-+]+)?$`,
	`^[a-z0-9][a-z0-9._-]*-mcp(?:-[a-z0-9._-]+)?$|^mcp-[a-z0-9][a-z0-9._-]*$`,
	`^-{1,2}\w`,
	`^\s*(//|/\*|\*)`,
	`(?i)\b(ADR[- ]?\d{1,5}|RFC[- ]?\d{1,5})\b`,
	`([A-Za-z]+)[- ]?(\d+)`,
	`(?i)^test_|_test\..|\.test\..|\.spec\..|_spec\..|\.tests\.ps1$`,
	`Tests?\.java$|Tests\.cs$`,
	`(?i)(<script\b(?:"[^"]*"|'[^']*'|[^>"'])*>)([\s\S]*?)(</script\s*>)`,
	`(?i)\blang\s*=\s*['"]?([A-Za-z]+)['"]?`,
	`\A\s*---\s*\r?\n([\s\S]*?)\r?\n---\s*(?:\r?\n|\z)`,
	`import\(\s*['"]([^'"]+)['"]\s*\)`,
	`[\p{L}\p{N}_]+`,
	`[^\p{L}\p{N}_]+`,
	`_+`,
}

// regexpAlphabet is the subject alphabet: ASCII letters, digits, spaces and punctuation, \n and
// \r\n, the case-folding specials, digits of other scripts, U+2028, an emoji, invalid UTF-8, and
// a few tokens the constant patterns look for (so that they match sometimes).
var regexpAlphabet = []string{
	"a", "b", "z", "A", "Z", "k", "s", "0", "1", "9", "_", " ", "  ", "\t", "\n", "\r\n",
	"-", ".", ",", ":", ";", "/", "\\", "\"", "'", "(", ")", "[", "]", "{", "}", "<", ">",
	"=", "!", "@", "#", "$", "*", "+", "?", "|", "^", "~", "&",
	"é", "ß", "Σ", "σ", "ς", "K", "ſ", "٣", "४",
	"４", " ", "\U0001F600", "\xff", "\x80", "\xe2\x82",
	"module ", "require (", "require ", " v1.2.3", "ADR-12", "rfc 7", "<script lang=\"ts\">",
	"</script>", "---\n", "[t](u)", "[[w|h]]", "`c`", "# H", "password", "id_rsa", ".env",
	"x.pem", "import('a')", "://u:p@", "//", "revision:", "@a/b", "x-mcp", "--f", "Test.java",
	"${p}", "::", "[ref]: <u>",
}

func regexpSubjects(r *rng) [][]byte {
	var subs [][]byte
	for i := 0; i < 20; i++ {
		var b strings.Builder
		for n := 1 + r.intn(14); n > 0; n-- {
			b.WriteString(regexpAlphabet[r.intn(len(regexpAlphabet))])
		}
		subs = append(subs, []byte(b.String()))
	}
	var long strings.Builder
	for long.Len() < 4096 {
		long.WriteString(regexpAlphabet[r.intn(len(regexpAlphabet))])
	}
	return append(subs, []byte{}, []byte(long.String()))
}

// The generated patterns. Every tenth is invalid, from regexpInvalid; the rest are valid by
// construction (a repeat is never applied to a repeat, a count above 4 only goes on a single
// atom outside any group, and at most twice, so the product of nested counts stays below 1000).
var (
	regexpLits = []string{
		"a", "b", "c", "x", "Z", "0", "9", "_", "abc", "foo", "é", "ß", "Σ",
		"σ", "ς", "K", "ſ", "日", "\U0001F600", `\.`, `\*`, `\-`, `\/`,
		`\x41`, `\n`, `\t`, `\x{212a}`, `\x{17f}`,
	}
	regexpClasses = []string{
		".", "[a-z]", "[^x]", "[[:alpha:]]", "[[:^digit:]]", `\d`, `\D`, `\w`, `\W`, `\s`, `\S`,
		`\pL`, `\p{Greek}`, `\p{^Lu}`, `\P{N}`, `\p{Lu}`, `[\d_]`, `[a-f\pL]`, `[^\s]`,
		`[\x00-\x1f]`, `[\x{3b1}-\x{3c9}]`,
	}
	regexpAnchors    = []string{"^", "$", `\A`, `\z`, `\b`, `\B`}
	regexpFlags      = []string{"(?i)", "(?m)", "(?s)", "(?U)", "(?i-m)", "(?-i)"}
	regexpGroupFlags = []string{"i", "s", "m", "U", "i-s", "m-i", "is"}
	regexpBig        = []string{"{1000}", "{999}", "{1000,}", "{998,1000}", "{500,1000}", "{0,1000}"}
)

// regexpBad is an invalid pattern, or the tail of one: unless bare, a valid group goes before it.
type regexpBad struct {
	re   string
	bare bool
}

var regexpInvalid = []regexpBad{
	{"(", false}, {")", false}, {"(a", false}, {"a)", false}, {"(a|b|", false},
	{"[a-z", false}, {"([a-z)", false}, {"x{1001}", false}, {"x{2,1}", false},
	{"x{9876543210}", false}, {"a{100000}", false}, {"[a-Z]", false}, {"(?i)[a-Z]", false},
	{"(?P<name>a", false}, {"(?P<name", false}, {"(?P<x y>a)", false}, {"(?P<>a)", false},
	{`\8`, false}, {`\Z`, false}, {"[z-a]", false}, {`\p{Nope}`, false}, {"a**", false},
	{"a++", false}, {"[[:foo:]]", false}, {"(?z)", false}, {`a\`, false},
	{"*", true}, {"+a", true}, {"?", true}, {`\Q\E*`, true}, {"|*", true}, {"(*)", true},
}

type regexpGen struct {
	r     *rng
	named int
	big   int
}

func (g *regexpGen) pick(l []string) string { return l[g.r.intn(len(l))] }

func (g *regexpGen) expr(depth int) string {
	alts := make([]string, 1)
	if g.r.intn(5) == 0 {
		alts = make([]string, 2+g.r.intn(2))
	}
	for i := range alts {
		alts[i] = g.seq(depth)
	}
	return strings.Join(alts, "|")
}

func (g *regexpGen) seq(depth int) string {
	var b strings.Builder
	for n := 1 + g.r.intn(4); n > 0; n-- {
		b.WriteString(g.piece(depth))
	}
	return b.String()
}

func (g *regexpGen) piece(depth int) string {
	switch g.r.intn(20) {
	case 0:
		return g.pick(regexpAnchors)
	case 1:
		return g.pick(regexpFlags)
	}
	atom, leaf := g.atom(depth)
	return atom + g.repeat(depth, leaf)
}

// atom returns a literal, a class or a group, and whether it is a literal or class.
func (g *regexpGen) atom(depth int) (string, bool) {
	k := g.r.intn(10)
	if depth >= 3 {
		k %= 5
	}
	switch k {
	case 0, 1, 2:
		return g.pick(regexpLits), true
	case 3, 4:
		return g.pick(regexpClasses), true
	case 5:
		return "(" + g.expr(depth+1) + ")", false
	case 6:
		return "(?:" + g.expr(depth+1) + ")", false
	case 7:
		g.named++
		name := "name"
		if g.named > 1 {
			name = fmt.Sprintf("n%d", g.named)
		}
		return "(?P<" + name + ">" + g.expr(depth+1) + ")", false
	}
	return "(?" + g.pick(regexpGroupFlags) + ":" + g.expr(depth+1) + ")", false
}

func (g *regexpGen) repeat(depth int, leaf bool) string {
	var op string
	switch k := g.r.intn(12); {
	case k < 3:
		op = []string{"*", "+", "?"}[k]
	case k == 3:
		op = fmt.Sprintf("{%d}", g.r.intn(5))
	case k == 4:
		op = fmt.Sprintf("{%d,}", g.r.intn(4))
	case k == 5:
		lo := g.r.intn(4)
		op = fmt.Sprintf("{%d,%d}", lo, lo+g.r.intn(3))
	default:
		return ""
	}
	if depth == 0 && leaf && g.big < 2 && g.r.intn(8) == 0 {
		g.big++
		op = g.pick(regexpBig)
	}
	if g.r.intn(3) == 0 {
		op += "?"
	}
	return op
}

func regexpPattern(i int, r *rng) string {
	g := &regexpGen{r: r}
	if i%10 != 9 {
		return g.expr(0)
	}
	t := regexpInvalid[r.intn(len(regexpInvalid))]
	if t.bare {
		return t.re
	}
	return "(?:" + g.expr(1) + ")" + t.re
}

func regexpFrame(recs [][]byte) []byte {
	var b []byte
	for _, rec := range recs {
		b = binary.BigEndian.AppendUint32(b, uint32(len(rec)))
		b = append(b, rec...)
	}
	return b
}

func regexpUnframe(b []byte) [][]byte {
	var recs [][]byte
	for len(b) > 0 {
		n := int(binary.BigEndian.Uint32(b))
		recs = append(recs, b[4:4+n])
		b = b[4+n:]
	}
	return recs
}

func regexpRune(r *rng) rune {
	switch r.intn(4) {
	case 0:
		return rune(0x20 + r.intn(0x5f))
	case 1:
		return rune(0x80 + r.intn(0x780))
	case 2:
		return rune(0x800 + r.intn(0xD000-0x800))
	}
	return rune(0x10000 + r.intn(0x10000))
}

func regexpGenerate(dir string) error {
	put := func(name, pat string, r *rng) error {
		recs := append([][]byte{[]byte(pat)}, regexpSubjects(r)...)
		return os.WriteFile(filepath.Join(dir, name), regexpFrame(recs), 0o644)
	}
	for i, p := range regexpConstants {
		for _, c := range []struct{ prefix, pat string }{{"c", p}, {"i", "(?i)" + p}} {
			name := fmt.Sprintf("%s%02d", c.prefix, i)
			if err := put(name, c.pat, seed("regexp/"+name)); err != nil {
				return err
			}
		}
	}
	for i := 0; i < 2000; i++ {
		name := fmt.Sprintf("g%04d", i)
		r := seed("regexp/" + name)
		if err := put(name, regexpPattern(i, r), r); err != nil {
			return err
		}
	}
	var recs [][]byte
	for c := 0; c < 128; c++ {
		recs = append(recs, []byte{byte(c)})
	}
	const metas = `\.+*?()|[]{}^$`
	r := seed("regexp/quote_meta")
	for i := 0; i < 1000; i++ {
		var b strings.Builder
		for n := r.intn(10); n > 0; n-- {
			if r.intn(3) == 0 {
				b.WriteByte(metas[r.intn(len(metas))])
			} else {
				b.WriteRune(regexpRune(r))
			}
		}
		recs = append(recs, []byte(b.String()))
	}
	return os.WriteFile(filepath.Join(dir, "quote_meta"), regexpFrame(recs), 0o644)
}

// regexpQ is the q of the comment at the top.
func regexpQ(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	for i := 0; i < len(s); i++ {
		if c := s[i]; c >= 0x20 && c < 0x7f && c != '"' && c != '\\' {
			b.WriteByte(c)
		} else {
			fmt.Fprintf(&b, "\\x%02x", c)
		}
	}
	b.WriteByte('"')
	return b.String()
}

// regexpGroups writes the groups of one match, from its index pairs over s.
func regexpGroups(s string, loc []int) string {
	parts := make([]string, len(loc)/2)
	for i := range parts {
		parts[i] = "unmatched"
		if loc[2*i] >= 0 {
			parts[i] = regexpQ(s[loc[2*i]:loc[2*i+1]])
		}
	}
	return "[" + strings.Join(parts, " ") + "]"
}

// regexpSpans writes the index pairs of one match.
func regexpSpans(loc []int) string {
	parts := make([]string, len(loc)/2)
	for i := range parts {
		parts[i] = "unmatched"
		if loc[2*i] >= 0 {
			parts[i] = fmt.Sprintf("%d,%d", loc[2*i], loc[2*i+1])
		}
	}
	return "[" + strings.Join(parts, " ") + "]"
}

// regexpCount writes the number of items and then each item.
func regexpCount(items []string) string {
	return strings.Join(append([]string{fmt.Sprint(len(items))}, items...), " ")
}

func regexpList(n int, items []string) string { return fmt.Sprintf("n=%d %s", n, regexpCount(items)) }

func regexpQuoted(l []string) []string {
	q := make([]string, len(l))
	for i, s := range l {
		q[i] = regexpQ(s)
	}
	return q
}

func init() {
	register("regexp", regexpGenerate, func(name string, in []byte) []op {
		recs := regexpUnframe(in)
		if name == "quote_meta" {
			var b strings.Builder
			for _, s := range recs {
				b.WriteString(regexpQ(regexp.QuoteMeta(string(s))) + "\n")
			}
			return []op{{"quote_meta", result{out: []byte(b.String())}}}
		}
		re, err := regexp.Compile(string(recs[0]))
		if err != nil {
			return []op{{"compile", result{err: err.Error()}}}
		}
		subs := recs[1:]
		strOp := func(f func(s string) string) result {
			var b strings.Builder
			for _, sub := range subs {
				if utf8.Valid(sub) {
					b.WriteString(f(string(sub)))
				} else {
					b.WriteString("-")
				}
				b.WriteByte('\n')
			}
			return result{out: []byte(b.String())}
		}
		byteOp := func(f func(b []byte) string) result {
			var b strings.Builder
			for _, sub := range subs {
				b.WriteString(f(sub) + "\n")
			}
			return result{out: []byte(b.String())}
		}
		each := func(ns []int, f func(n int) string) string {
			parts := make([]string, len(ns))
			for i, n := range ns {
				parts[i] = f(n)
			}
			return strings.Join(parts, " ; ")
		}
		return []op{
			{"compile", result{out: []byte("ok\n")}},
			{"match", strOp(func(s string) string { return string(bit(re.MatchString(s))) })},
			{"find_index", strOp(func(s string) string {
				if loc := re.FindStringIndex(s); loc != nil {
					return fmt.Sprintf("%d,%d", loc[0], loc[1])
				}
				return "none"
			})},
			{"find_submatch", strOp(func(s string) string {
				if loc := re.FindStringSubmatchIndex(s); loc != nil {
					return regexpGroups(s, loc)
				}
				return "none"
			})},
			{"find_all", strOp(func(s string) string {
				return each([]int{-1, 0, 2}, func(n int) string {
					return regexpList(n, regexpQuoted(re.FindAllString(s, n)))
				})
			})},
			{"find_all_index", strOp(func(s string) string {
				var items []string
				for _, loc := range re.FindAllStringIndex(s, -1) {
					items = append(items, fmt.Sprintf("%d,%d", loc[0], loc[1]))
				}
				return regexpCount(items)
			})},
			{"find_all_submatch", strOp(func(s string) string {
				var items []string
				for _, loc := range re.FindAllStringSubmatchIndex(s, -1) {
					items = append(items, regexpGroups(s, loc))
				}
				return regexpCount(items)
			})},
			{"bytes_submatch_index", byteOp(func(b []byte) string {
				if loc := re.FindSubmatchIndex(b); loc != nil {
					return regexpSpans(loc)
				}
				return "none"
			})},
			{"bytes_all_submatch_index", byteOp(func(b []byte) string {
				var items []string
				for _, loc := range re.FindAllSubmatchIndex(b, -1) {
					items = append(items, regexpSpans(loc))
				}
				return regexpCount(items)
			})},
			{"replace", strOp(func(s string) string {
				var items []string
				for _, repl := range []string{"_", "", "$1x", "${1}", "$$", "${name}"} {
					items = append(items, regexpQ(re.ReplaceAllString(s, repl)))
				}
				return strings.Join(items, " ")
			})},
			{"replace_func", strOp(func(s string) string {
				return regexpQ(re.ReplaceAllStringFunc(s, func(m string) string { return "<" + m + ">" }))
			})},
			{"split", strOp(func(s string) string {
				return each([]int{-1, 0, 3}, func(n int) string {
					return regexpList(n, regexpQuoted(re.Split(s, n)))
				})
			})},
		}
	})
}
