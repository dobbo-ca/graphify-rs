package main

import (
	"bytes"
	"encoding/csv"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"
)

// csv: encoding/csv Writer (default comma, no UseCRLF) on 2,000 generated records of 0 to 8
// fields. The input is the fields separated by 0x1F, which is not in the alphabet; an empty
// input is a record with no fields. Every piece is valid UTF-8 (gostd takes &str).

// csvAlphabet is every valid UTF-8 piece of stringsAlphabet (all the kinds of space, the
// case-folding letters, a combining mark, 4-byte runes) plus the pieces that decide quoting.
var csvAlphabet = func() []string {
	a := []string{",", "\"", "\r", "\n", "\\.", "\\", "\r\n", "\"\""}
	for _, s := range stringsAlphabet {
		if utf8.ValidString(s) {
			a = append(a, s)
		}
	}
	return a
}()

func init() {
	register("csv", 2000, func(dir string) error {
		for i := 0; i < 2000; i++ {
			name := fmt.Sprintf("c%04d", i)
			r := seed("csv/" + name)
			fields := make([]string, r.intn(9))
			for j := range fields {
				var b strings.Builder
				for n := r.intn(5); n > 0; n-- {
					b.WriteString(csvAlphabet[r.intn(len(csvAlphabet))])
				}
				fields[j] = b.String()
			}
			if err := os.WriteFile(filepath.Join(dir, name), []byte(strings.Join(fields, "\x1f")), 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(_ string, in []byte) []op {
		var fields []string
		if len(in) > 0 {
			fields = strings.Split(string(in), "\x1f")
		}
		var buf bytes.Buffer
		w := csv.NewWriter(&buf)
		w.Write(fields)
		w.Flush()
		if err := w.Error(); err != nil {
			return []op{{"write", result{err: err.Error()}}}
		}
		return []op{{"write", result{out: buf.Bytes()}}}
	})
}
