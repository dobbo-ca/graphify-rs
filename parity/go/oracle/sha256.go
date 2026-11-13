package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// sha256: Sum256 of 131 short inputs (lengths 0 to 130, across the padding boundaries), 100 random
// inputs up to 1 MiB, and 200 inputs fed to sha256.New in pieces. The plan says 1,000 random
// inputs; 100 keeps BuildKit disk use near 50 MB. A split case file is
// "<comma-separated offsets>\n<data>" (offsets ascending, at most 8, may be empty).
func init() {
	register("sha256", 631, func(dir string) error {
		write := func(name string, b []byte) error {
			return os.WriteFile(filepath.Join(dir, name), b, 0o644)
		}
		for n := 0; n <= 130; n++ {
			b := make([]byte, n)
			for i := range b {
				b[i] = byte(i*7 + n)
			}
			if err := write(fmt.Sprintf("len%04d", n), b); err != nil {
				return err
			}
		}
		for i := 0; i < 100; i++ {
			name := fmt.Sprintf("big%03d", i)
			r := seed("sha256/" + name)
			b := make([]byte, r.intn(1<<20+1))
			for j := range b {
				b[j] = byte(r.next())
			}
			if err := write(name, b); err != nil {
				return err
			}
		}
		for i := 0; i < 200; i++ {
			name := fmt.Sprintf("split%03d", i)
			r := seed("sha256/" + name)
			b := make([]byte, r.intn(64<<10+1))
			for j := range b {
				b[j] = byte(r.next())
			}
			offs := make([]string, 0, 8)
			if len(b) > 0 {
				last := 0
				for k := r.intn(9); k > 0; k-- {
					last += r.intn(len(b) - last + 1)
					offs = append(offs, strconv.Itoa(last))
				}
			}
			if err := write(name, append([]byte(strings.Join(offs, ",")+"\n"), b...)); err != nil {
				return err
			}
		}
		return nil
	}, func(name string, input []byte) []op {
		if !strings.HasPrefix(name, "split") {
			s := sha256.Sum256(input)
			return []op{{"sum", result{out: []byte(hex.EncodeToString(s[:]))}}}
		}
		line, data, _ := strings.Cut(string(input), "\n")
		s := sha256.Sum256([]byte(data))
		d := sha256.New()
		prev := 0
		if line != "" {
			for _, f := range strings.Split(line, ",") {
				off, _ := strconv.Atoi(f)
				d.Write([]byte(data[prev:off]))
				prev = off
			}
		}
		d.Write([]byte(data[prev:]))
		return []op{
			{"sum", result{out: []byte(hex.EncodeToString(s[:]))}},
			{"split", result{out: []byte(hex.EncodeToString(d.Sum(nil)))}},
		}
	})
}
