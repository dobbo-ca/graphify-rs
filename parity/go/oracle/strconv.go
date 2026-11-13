package main

import (
	"encoding/binary"
	"fmt"
	"math"
	"math/big"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// strconv: FormatFloat, ParseFloat, ParseInt, ParseUint, Atoi and Quote against gostd::strconv.
// Inputs: "floats" (200,000 bit patterns, one 16-digit hex number per line), "parse-NNN" (20
// files of 1,000 ParseFloat strings, one per line), "ints" (3,000 lines), "quote-NNN" (5 files
// of 1,000 length-prefixed byte strings: 4-byte big-endian length, then the bytes) and an
// empty "sweep" for the one-rune Quote of every scalar value. The parse and ints inputs are
// ASCII (plus a few non-ASCII digits) with no newline inside a line.
//
// The "E" and "G" float ops are named upper_e and upper_g so that the output names differ
// without regard to case.

func init() {
	register("strconv", scGen, scRun)
}

func scWrite(dir, name string, b []byte) error {
	return os.WriteFile(filepath.Join(dir, name), b, 0o644)
}

func scGen(dir string) error {
	if err := scWrite(dir, "floats", scFloats()); err != nil {
		return err
	}
	for i := 0; i < 20; i++ {
		name := fmt.Sprintf("parse-%03d", i)
		r := seed("strconv/" + name)
		var b strings.Builder
		for j := 0; j < 1000; j++ {
			b.WriteString(scParseLine(r))
			b.WriteByte('\n')
		}
		if err := scWrite(dir, name, []byte(b.String())); err != nil {
			return err
		}
	}
	if err := scWrite(dir, "ints", scInts()); err != nil {
		return err
	}
	for i := 0; i < 5; i++ {
		name := fmt.Sprintf("quote-%03d", i)
		r := seed("strconv/" + name)
		var b []byte
		for j := 0; j < 1000; j++ {
			var s []byte
			for n := r.intn(13); n > 0; n-- {
				if r.intn(3) == 0 {
					s = append(s, byte(r.next()))
				} else {
					s = append(s, scQuoteAlphabet[r.intn(len(scQuoteAlphabet))]...)
				}
			}
			b = binary.BigEndian.AppendUint32(b, uint32(len(s)))
			b = append(b, s...)
		}
		if err := scWrite(dir, name, b); err != nil {
			return err
		}
	}
	return scWrite(dir, "sweep", nil)
}

// scQuoteAlphabet: printable and control ASCII, the quotes and the backslash, runes the
// escaper treats specially (soft hyphen, line separator, BOM, private use, unassigned,
// noncharacter, replacement character, emoji) and invalid sequences.
var scQuoteAlphabet = []string{
	"a", "Z", "0", " ", "\t", "\n", "\r", "\a", "\b", "\f", "\v", "\x00", "\x1b", "\x7f",
	"\"", "'", "\\", "\u00e9", "\u00a0", "\u00ad", "\u0085", "\u2028", "\u2029", "\ufeff",
	"\ue000", "\u0378", "\uffff", "\ufffd", "\U0001F600", "\U0010FFFF", "\U000E0001",
	"\x80", "\xff", "\xe2\x82", "\xc0\x80", "\xed\xa0\x80", "\xf0\x9f\x98", "\xf4\x90\x80\x80",
}

// scFloats is the 200,000 float64 bit patterns, one 16-digit hex number per line: the special
// values first, then random bit patterns.
func scFloats() []byte {
	const total = 200000
	r := seed("strconv/floats")
	var v []uint64
	bits := math.Float64bits
	around := func(f float64) {
		b := bits(f)
		v = append(v, b, b|1<<63)
		for d := uint64(1); d <= 3; d++ {
			v = append(v, b+d, b-d)
		}
	}
	for k := 0; k < 52; k++ {
		v = append(v, 1<<k)
	}
	for i := 0; i < 1000; i++ {
		v = append(v, r.next()&(1<<52-1))
	}
	for e := -1074; e <= 1023; e++ {
		v = append(v, bits(math.Ldexp(1, e)))
	}
	for n := -323; n <= 308; n++ {
		f, _ := strconv.ParseFloat("1e"+strconv.Itoa(n), 64)
		around(f)
	}
	around(1e-6)
	around(1e21)
	v = append(v, bits(math.MaxFloat64), bits(math.SmallestNonzeroFloat64), 0, 1<<63,
		bits(math.Inf(1)), bits(math.Inf(-1)), bits(math.NaN()))
	for len(v) < total {
		v = append(v, r.next())
	}
	var b strings.Builder
	for _, x := range v {
		fmt.Fprintf(&b, "%016x\n", x)
	}
	return []byte(b.String())
}

var scFixedParse = []string{
	"", ".", "+", "-", " 1", "1 ", "1e", "e1", "1e+", "1e-", ".e1", "0", "-0", "+0", "0.0", "00",
	"inf", "Inf", "INF", "INFINITY", "Infinity", "infinity", "nan", "NaN", "NAN", "+inf", "-inf",
	"+Inf", "-Inf", "-NaN", "+nan", "infinit", "infinityx", "nan1", "in", "na",
	"0x1.8p1", "0X1.8P1", "0x_1p0", "0X_1p-2", "0x1p-1074", "0x1.fffffffffffff8p1023",
	"0x1.fffffffffffff7p1023", "0x1p1024", "0x1p-1075", "0x1.8p-1075", "0x0p0", "-0x0p0",
	"0x1", "0x", "0x.p1", "0x.8p1", "0x1.p1", "0xp1", "0x1p", "0x1p+", "0x1_0p0", "0x1__0p0",
	"0x1p1_0", "0x_p1", "0x1_.8p0", "0xAbCp-3", "0x1.fffffffffffffp1023", "0x1.0000000000001p0",
	"1_000", "1__0", "_1", "1_", "1_000.5", "1e1_0", "1_0e1", "0_1", "1._5", "1.5_", "0b1", "0o7",
	"1e308", "1.7976931348623157e308", "1.7976931348623159e308", "1.797693134862315807e308",
	"1e309", "-1e309", "4.9e-324", "2.4703282292062327e-324", "2.4703282292062328e-324", "1e-325",
	"5e-324", "1e-400", "9007199254740993", "9007199254740992.5", "0.1", "1e23", "8.41e21",
	"123456789012345678901234567890", "1.", ".1", "-.1", "+.1e1", "1e+400", "1E5", "1d5", "1f",
	"1,5", "１", "1e５", "0x1p0 ", " 0x1p0", "+-1", "--1", "++1", "1e1.5", "1.5.5", "1ee5",
}

func scParseLine(r *rng) string {
	switch k := r.intn(10); {
	case k == 0:
		return scFixedParse[r.intn(len(scFixedParse))]
	case k <= 3:
		return scDecimal(r)
	case k == 4:
		return scHalfway(r)
	case k == 5:
		return scShort(r)
	case k <= 7:
		return scHex(r)
	default:
		s := scDecimal(r)
		if r.intn(2) == 0 {
			s = scHex(r)
		}
		return scUnderscores(r, s)
	}
}

func scSign(r *rng) string {
	if r.intn(4) == 0 {
		return string("+-"[r.intn(2)])
	}
	return ""
}

func scDigits(r *rng, n, dot int, digits string) string {
	var b strings.Builder
	for i := 0; i < n; i++ {
		if i == dot {
			b.WriteByte('.')
		}
		b.WriteByte(digits[r.intn(len(digits))])
	}
	if dot == n {
		b.WriteByte('.')
	}
	return b.String()
}

// scDecimal: 1 to 800 digits (short ones half the time) with an optional point and an
// optional exponent in -400..400.
func scDecimal(r *rng) string {
	n := 1 + r.intn(800)
	if r.intn(2) == 0 {
		n = 1 + r.intn(25)
	}
	dot := -1
	if r.intn(2) == 0 {
		dot = r.intn(n + 1)
	}
	s := scSign(r) + scDigits(r, n, dot, "0123456789")
	if r.intn(2) == 0 {
		s += fmt.Sprintf("%c", "eE"[r.intn(2)])
		if r.intn(2) == 0 {
			s += fmt.Sprintf("%+d", r.intn(801)-400)
		} else {
			s += fmt.Sprintf("%d", r.intn(801)-400)
		}
	}
	return s
}

// scShort is the atof1k style: a short mantissa and an exponent over the whole float64 range.
func scShort(r *rng) string {
	n := 1 + r.intn(20)
	return scSign(r) + scDigits(r, n, r.intn(n+1), "0123456789") + "e" + strconv.Itoa(r.intn(641)-330)
}

// scHalfway is the exact decimal of (2m+1) * 2^(e-1): the midpoint between two adjacent
// float64 values (a subnormal pair a quarter of the time), as digits with a point or as an
// integer with a negative exponent.
func scHalfway(r *rng) string {
	m := r.next()>>11 | 1<<52
	e := r.intn(2046) - 1074
	if r.intn(4) == 0 {
		m, e = r.next()>>12, -1074
	}
	n := new(big.Int).SetUint64(2*m + 1)
	p := e - 1
	if p >= 0 {
		return n.Lsh(n, uint(p)).String()
	}
	q := -p
	n.Mul(n, new(big.Int).Exp(big.NewInt(5), big.NewInt(int64(q)), nil))
	s := n.String()
	if r.intn(2) == 0 {
		return s + "e-" + strconv.Itoa(q)
	}
	if len(s) <= q {
		s = strings.Repeat("0", q-len(s)+1) + s
	}
	return s[:len(s)-q] + "." + s[len(s)-q:]
}

// scHex: a hex float, usually with the mandatory p exponent, sometimes underscores after the
// prefix.
func scHex(r *rng) string {
	n := 1 + r.intn(20)
	s := scSign(r) + []string{"0x", "0X"}[r.intn(2)]
	if r.intn(4) == 0 {
		s += "_"
	}
	s += scDigits(r, n, r.intn(n+1), "0123456789abcdefABCDEF")
	if r.intn(10) != 0 {
		s += fmt.Sprintf("%c%+d", "pP"[r.intn(2)], r.intn(2201)-1100)
	}
	return s
}

// scUnderscores inserts an underscore after a random digit in a third of the strings; Go
// accepts some of them (after a base prefix) and rejects the rest.
func scUnderscores(r *rng, s string) string {
	if r.intn(3) != 0 {
		return s
	}
	var at []int
	for i := 0; i < len(s); i++ {
		if strings.IndexByte("0123456789abcdefABCDEF", s[i]) >= 0 {
			at = append(at, i+1)
		}
	}
	if len(at) == 0 {
		return s
	}
	i := at[r.intn(len(at))]
	return s[:i] + "_" + s[i:]
}

var scFixedInts = []string{
	"", "+", "-", "0x_1", "1_000", "0b101", "0o17", "017", "08", "0x", "0b", "0o", "0_7", "0x_",
	"_1", "1_", "0x1_", "+0", "-0", "+-1", "--1", " 1", "1 ", "1 2", "\t1", "00", "000", "0_0",
	"1__0", "0x1p3", "1e3", "1.0", "0xg", "0XFF", "0B11", "0O7", "0x_ff", "0b_1", "0o_7", "0_17",
	"９", "１２３", "٣", "१२", "1٣", "−1", "१_२", "0x१",
	"9223372036854775807", "18446744073709551615", "-9223372036854775808",
	"-9223372036854775809", "+9223372036854775808", "0xFFFFFFFFFFFFFFFF", "0x8000000000000000",
	"-0x8000000000000000", "4294967295", "4294967296", "-2147483648", "2147483647",
}

// scInts is 3,000 lines: the fixed list, every boundary +-1 in several bases and prefixes, and
// random numbers.
func scInts() []byte {
	const total = 3000
	r := seed("strconv/ints")
	v := append([]string(nil), scFixedInts...)
	one := big.NewInt(1)
	for _, bound := range []struct {
		exp uint
		neg bool
	}{{31, false}, {31, true}, {63, false}, {63, true}, {32, false}, {64, false}, {64, true},
		{15, false}, {16, false}, {7, true}, {8, false}} {
		b := new(big.Int).Lsh(one, bound.exp)
		if bound.neg {
			b.Neg(b)
		}
		for d := int64(-1); d <= 1; d++ {
			n := new(big.Int).Add(b, big.NewInt(d))
			sign, abs := "", new(big.Int).Abs(n)
			if n.Sign() < 0 {
				sign = "-"
			}
			v = append(v, n.String())
			for _, f := range []struct {
				prefix string
				base   int
			}{{"0x", 16}, {"0X", 16}, {"0b", 2}, {"0o", 8}, {"0", 8}, {"0x_", 16}, {"0b_", 2}} {
				v = append(v, sign+f.prefix+abs.Text(f.base))
			}
		}
	}
	prefixes := []string{"", "", "", "0x", "0X", "0b", "0B", "0o", "0O", "0", "0x_", "0b_", "0_"}
	for len(v) < total {
		p := prefixes[r.intn(len(prefixes))]
		alpha := "0123456789abcdef"
		if p == "" {
			alpha = "0123456789"
		}
		var b strings.Builder
		b.WriteString(scSign(r) + p)
		for n := 1 + r.intn(22); n > 0; n-- {
			switch r.intn(40) {
			case 0, 1:
				b.WriteByte('_')
			case 2:
				b.WriteByte('z')
			default:
				b.WriteByte(alpha[r.intn(len(alpha))])
			}
		}
		v = append(v, b.String())
	}
	return []byte(strings.Join(v[:total], "\n") + "\n")
}

// scLines splits an input into its lines; the last line ends with a newline.
func scLines(in []byte) []string {
	return strings.Split(strings.TrimSuffix(string(in), "\n"), "\n")
}

func scRun(name string, in []byte) []op {
	var ops []op
	switch {
	case name == "floats":
		for _, f := range []struct {
			op  string
			fmt byte
		}{{"g", 'g'}, {"e", 'e'}, {"f", 'f'}, {"upper_e", 'E'}, {"upper_g", 'G'}} {
			var b strings.Builder
			for _, l := range scLines(in) {
				x, err := strconv.ParseUint(l, 16, 64)
				if err != nil {
					panic(err)
				}
				b.WriteString(strconv.FormatFloat(math.Float64frombits(x), f.fmt, -1, 64) + "\n")
			}
			ops = append(ops, op{f.op, result{out: []byte(b.String())}})
		}
	case strings.HasPrefix(name, "parse-"):
		var b strings.Builder
		for _, l := range scLines(in) {
			x, err := strconv.ParseFloat(l, 64)
			if err != nil {
				b.WriteString(err.Error() + "\n")
			} else {
				fmt.Fprintf(&b, "%016x\n", math.Float64bits(x))
			}
		}
		ops = append(ops, op{"parse_float", result{out: []byte(b.String())}})
	case name == "ints":
		each := func(name string, f func(s string) (string, error)) {
			var b strings.Builder
			for _, l := range scLines(in) {
				v, err := f(l)
				if err != nil {
					v = err.Error()
				}
				b.WriteString(v + "\n")
			}
			ops = append(ops, op{name, result{out: []byte(b.String())}})
		}
		signed := func(base, bits int) func(string) (string, error) {
			return func(s string) (string, error) {
				v, err := strconv.ParseInt(s, base, bits)
				return strconv.FormatInt(v, 10), err
			}
		}
		unsigned := func(base, bits int) func(string) (string, error) {
			return func(s string) (string, error) {
				v, err := strconv.ParseUint(s, base, bits)
				return strconv.FormatUint(v, 10), err
			}
		}
		each("atoi", func(s string) (string, error) {
			v, err := strconv.Atoi(s)
			return strconv.Itoa(v), err
		})
		each("parse_int_0_64", signed(0, 64))
		each("parse_int_10_32", signed(10, 32))
		each("parse_uint_10_32", unsigned(10, 32))
		each("parse_uint_0_64", unsigned(0, 64))
	case strings.HasPrefix(name, "quote-"):
		var b strings.Builder
		for len(in) >= 4 {
			n := int(binary.BigEndian.Uint32(in))
			b.WriteString(strconv.Quote(string(in[4:4+n])) + "\n")
			in = in[4+n:]
		}
		ops = append(ops, op{"quote", result{out: []byte(b.String())}})
	case name == "sweep":
		var b strings.Builder
		for r := rune(0); r <= 0x10FFFF; r++ {
			if r < 0xD800 || r > 0xDFFF {
				b.WriteString(strconv.Quote(string(r)) + "\n")
			}
		}
		ops = append(ops, op{"quote_runes", result{out: []byte(b.String())}})
	}
	return ops
}
