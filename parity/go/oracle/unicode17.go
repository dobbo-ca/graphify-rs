package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode"
)

// unicode17: Go std unicode (UCD 17.0.0) against gostd::unicode. One input, "sweep": every
// scalar value (no surrogates: a Rust char cannot hold one) through each predicate and mapping,
// the member runs of every table, and the category aliases.
func init() {
	register("unicode17", func(dir string) error {
		return os.WriteFile(filepath.Join(dir, "sweep"), nil, 0o644)
	}, func(_ string, _ []byte) []op {
		return []op{
			{"props", result{out: []byte(unicodeProps())}},
			{"tables", result{out: []byte(unicodeTables())}},
			{"aliases", result{out: []byte(unicodeAliases())}},
		}
	})
}

func scalars(f func(r rune)) {
	for r := rune(0); r <= unicode.MaxRune; r++ {
		if r >= 0xD800 && r <= 0xDFFF {
			continue
		}
		f(r)
	}
}

func bit(b bool) byte {
	if b {
		return '1'
	}
	return '0'
}

func unicodeProps() string {
	var b strings.Builder
	scalars(func(r rune) {
		flags := []byte{bit(unicode.IsLetter(r)), bit(unicode.IsNumber(r)), bit(unicode.IsUpper(r)),
			bit(unicode.IsSpace(r)), bit(unicode.IsPrint(r)), bit(unicode.Is(unicode.Mn, r))}
		fmt.Fprintf(&b, "%04X %s %04X %04X %04X %04X\n", r, flags,
			unicode.ToLower(r), unicode.ToUpper(r), unicode.ToTitle(r), unicode.SimpleFold(r))
	})
	return b.String()
}

func unicodeTables() string {
	var b strings.Builder
	for _, m := range []struct {
		name string
		m    map[string]*unicode.RangeTable
	}{{"categories", unicode.Categories}, {"scripts", unicode.Scripts}, {"properties", unicode.Properties},
		{"fold_category", unicode.FoldCategory}, {"fold_script", unicode.FoldScript}} {
		var names []string
		for k := range m.m {
			names = append(names, k)
		}
		sort.Strings(names)
		for _, k := range names {
			t := m.m[k]
			start := rune(-1)
			prev := rune(-2)
			flush := func() {
				if start >= 0 {
					fmt.Fprintf(&b, "%s %s %04X-%04X\n", m.name, k, start, prev)
				}
			}
			scalars(func(r rune) {
				if !unicode.Is(t, r) {
					return
				}
				if r != prev+1 {
					flush()
					start = r
				}
				prev = r
			})
			flush()
		}
	}
	return b.String()
}

func unicodeAliases() string {
	var names []string
	for k := range unicode.CategoryAliases {
		names = append(names, k)
	}
	sort.Strings(names)
	var b strings.Builder
	for _, k := range names {
		fmt.Fprintf(&b, "%s %s\n", k, unicode.CategoryAliases[k])
	}
	return b.String()
}
