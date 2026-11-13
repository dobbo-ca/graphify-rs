package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"golang.org/x/exp/rand"
)

// pcg: golang.org/x/exp/rand's PCG source on five seeds. uint64 is the first 10,000 outputs,
// uint64n is 1,000 draws for each of ten bounds ("n v" per line), and louvain (seed 42 only) is
// the Fisher-Yates shuffle of louvain.go:131, one permutation per line for n in 2..=1000.

var pcgSeeds = []struct {
	name string
	seed uint64
}{
	{"s0", 0}, {"s1", 1}, {"s42", 42}, {"smax", ^uint64(0)}, {"s2p63", 1 << 63},
}

var pcgBounds = []uint64{1, 2, 3, 5, 7, 10, 1 << 32, 1<<32 + 1, 1<<63 + 1, ^uint64(0)}

func init() {
	register("pcg", 11, func(dir string) error {
		for _, c := range pcgSeeds {
			if err := os.WriteFile(filepath.Join(dir, c.name), []byte(strconv.FormatUint(c.seed, 10)), 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(name string, in []byte) []op {
		seed, err := strconv.ParseUint(string(in), 10, 64)
		if err != nil {
			panic(err)
		}
		var u strings.Builder
		r := rand.New(rand.NewSource(seed))
		for i := 0; i < 10000; i++ {
			fmt.Fprintf(&u, "%d\n", r.Uint64())
		}
		var un strings.Builder
		r = rand.New(rand.NewSource(seed))
		for _, n := range pcgBounds {
			for i := 0; i < 1000; i++ {
				fmt.Fprintf(&un, "%d %d\n", n, r.Uint64n(n))
			}
		}
		ops := []op{
			{"uint64", result{out: []byte(u.String())}},
			{"uint64n", result{out: []byte(un.String())}},
		}
		if name == "s42" {
			var l strings.Builder
			for n := 2; n <= 1000; n++ {
				rnd := rand.New(rand.NewSource(seed)).Intn
				order := make([]int, n)
				for i := range order {
					order[i] = i
				}
				for i := range order[:n-1] {
					j := i + rnd(n-i)
					order[i], order[j] = order[j], order[i]
				}
				for i, v := range order {
					if i > 0 {
						l.WriteByte(' ')
					}
					l.WriteString(strconv.Itoa(v))
				}
				l.WriteByte('\n')
			}
			ops = append(ops, op{"louvain", result{out: []byte(l.String())}})
		}
		return ops
	})
}
