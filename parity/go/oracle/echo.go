package main

import (
	"os"
	"path/filepath"
)

// echo proves the harness: the output is the input. It also exercises the generator.
func init() {
	register("echo", func(dir string) error {
		all := make([]byte, 256)
		for i := range all {
			all[i] = byte(i)
		}
		r := seed("echo/random")
		rnd := make([]byte, 1+r.intn(4096))
		for i := range rnd {
			rnd[i] = byte(r.next())
		}
		for name, b := range map[string][]byte{"empty": nil, "a": []byte("a"), "bytes": all, "random": rnd} {
			if err := os.WriteFile(filepath.Join(dir, name), b, 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(_ string, input []byte) []op {
		return []op{{"echo", result{out: input}}}
	})
}
