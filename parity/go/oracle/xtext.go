package main

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/text/cases"
	"golang.org/x/text/unicode/norm"
)

// xtext: cases.Fold, norm.NFKC and norm.NFKD on strings, plus the idutil fixpoint loop.
// The "sweep" case runs every scalar value through all three.

var xtextFoldCaser = cases.Fold()

type xtextRange struct{ lo, hi rune }

var xtextPools = []xtextRange{
	{0x61, 0x7A}, {0x41, 0x5A}, {0x30, 0x39}, {0x5F, 0x5F},
	{0xC0, 0x24F}, {0x1E00, 0x1EFF},
	{0x370, 0x3FF}, {0x1F00, 0x1FFF},
	{0x400, 0x4FF},
	{0x531, 0x587}, {0xFB13, 0xFB17},
	{0x13A0, 0x13FD}, {0xAB70, 0xABBF},
	{0x10400, 0x1044F},
	{0xF900, 0xFAFF}, {0x2F800, 0x2FA1D}, {0x3200, 0x33FF},
	{0xAC00, 0xD7A3}, {0x1100, 0x1112}, {0x1161, 0x1175}, {0x11A8, 0x11C2}, {0x3131, 0x318E}, {0xFFA0, 0xFFDC},
	{0xFB00, 0xFB06}, {0xFF01, 0xFF5E}, {0x2460, 0x24FF}, {0x2150, 0x218F}, {0x1D400, 0x1D7FF}, {0xFDF0, 0xFDFD}, {0xA0, 0xBF},
}

var xtextMarks = []xtextRange{
	{0x300, 0x36F}, {0x591, 0x5BD}, {0x64B, 0x65F}, {0xF71, 0xF84}, {0x1DC0, 0x1DFF}, {0x20D0, 0x20F0},
	{0x302A, 0x302F}, {0x3099, 0x309A}, {0x1D165, 0x1D169}, {0x1D16D, 0x1D172}, {0x344, 0x345}, {0xFF9E, 0xFF9F},
}

var xtextSpecials = []rune{
	0xDF, 0x130, 0xFB03, 0x390, 0x1F0, 0x345, 0x3C2, 0x3A3, 0x1E9E, 0x212A, 0x212B, 0x17F, 0x131, 0x49, 0xB5,
	0x1F88, 0x1FBC, 0x1C5, 0x13A0, 0xAB70,
}

// xtextPick picks a range uniformly, then a rune uniformly within it.
func xtextPick(r *rng, rs []xtextRange) rune {
	g := rs[r.intn(len(rs))]
	return g.lo + rune(r.intn(int(g.hi-g.lo)+1))
}

func xtextRandom(name string) string {
	r := seed("xtext/" + name)
	var b []rune
	if r.intn(4) == 0 {
		b = append(b, xtextPick(r, xtextPools))
		for n := 25 + r.intn(14); n > 0; n-- {
			b = append(b, xtextPick(r, xtextMarks))
		}
		return string(b)
	}
	n := 2 + r.intn(39)
	for i := 0; i < n; i++ {
		switch k := r.intn(10); {
		case k < 6:
			b = append(b, xtextPick(r, xtextPools))
		case k < 8:
			b = append(b, xtextPick(r, xtextMarks))
		case k < 9:
			b = append(b, xtextSpecials[r.intn(len(xtextSpecials))])
		default:
			m := 1 + r.intn(8)
			if len(b)+m > 40 {
				m = 40 - len(b)
			}
			for ; m > 0; m-- {
				b = append(b, xtextPick(r, xtextMarks))
			}
		}
	}
	return string(b)
}

func xtextHex(s string) string {
	if s == "" {
		return "-"
	}
	var b strings.Builder
	for i, c := range s {
		if i > 0 {
			b.WriteByte(',')
		}
		fmt.Fprintf(&b, "%04X", c)
	}
	return b.String()
}

func init() {
	register("xtext", 20053, func(dir string) error {
		w := func(name, s string) error { return os.WriteFile(filepath.Join(dir, name), []byte(s), 0o644) }
		fixed := []string{
			"\U00010041́",
			"႙\U000110BA",
			"가" + strings.Repeat("́", 30),
			"각" + strings.Repeat("̖", 29),
			"a" + strings.Repeat("ᅡ", 31),
			strings.Repeat("ཱི", 16),
			strings.Repeat("ﾞ", 31),
			"ẛ̣",
			"İ́",
			"㈎㈝",
			"Ꭰꭰ",
			"ßẞﬃΐǰͅςΣİ",
		}
		if err := w("sweep", ""); err != nil {
			return err
		}
		if err := w("cgj-31", "a"+strings.Repeat("́", 31)); err != nil {
			return err
		}
		for i, s := range fixed {
			if err := w(fmt.Sprintf("q%02d", i), s); err != nil {
				return err
			}
		}
		for i := 0; i < 5000; i++ {
			name := fmt.Sprintf("g%04d", i)
			if err := w(name, xtextRandom(name)); err != nil {
				return err
			}
		}
		return nil
	}, func(name string, input []byte) []op {
		if name == "sweep" {
			var b bytes.Buffer
			for r := rune(0); r <= 0x10FFFF; r++ {
				if r >= 0xD800 && r <= 0xDFFF {
					continue
				}
				s := string(r)
				fmt.Fprintf(&b, "%04X %s %s %s\n", r, xtextHex(xtextFoldCaser.String(s)), xtextHex(norm.NFKC.String(s)), xtextHex(norm.NFKD.String(s)))
			}
			return []op{{"sweep", result{out: b.Bytes()}}}
		}
		s := string(input)
		fix := ""
		cur := s
		for i := 0; i < 6; i++ {
			next := norm.NFKC.String(xtextFoldCaser.String(cur))
			fix += next + "\n"
			if next == cur {
				break
			}
			cur = next
		}
		return []op{
			{"fold", result{out: []byte(xtextFoldCaser.String(s))}},
			{"nfkc", result{out: []byte(norm.NFKC.String(s))}},
			{"nfkd", result{out: []byte(norm.NFKD.String(s))}},
			{"fixpoint", result{out: []byte(fix)}},
		}
	})
}
