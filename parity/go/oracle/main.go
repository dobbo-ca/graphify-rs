// Command oracle is the Go side of the gostd parity check (see ../../README.md).
//
//	oracle -gen <inputs>     write the test inputs
//	oracle <inputs> <out>    run every op on every input
//
// Inputs live in <inputs>/<pkg>/<case>. For each input and op, the run writes
// <out>/<pkg>/<case>.<op>.out (the output bytes) or <out>/<pkg>/<case>.<op>.err (the Go
// error text and a newline). crates/gostd/examples/oracle/main.rs does the same with gostd, and
// `diff -r` of the two trees must be empty.
package main

import (
	"flag"
	"fmt"
	"hash/fnv"
	"os"
	"path/filepath"
	"sort"
)

// result is the outcome of one op: the output bytes, or the error text.
type result struct {
	out []byte
	err string
}

// op is one named operation of a package on an input.
type op struct {
	name string
	res  result
}

type pkg struct {
	gen func(dir string) error
	run func(name string, input []byte) []op
}

var pkgs = map[string]pkg{}

// register is called from the init of each <pkg>.go.
func register(name string, gen func(dir string) error, run func(name string, input []byte) []op) {
	if _, dup := pkgs[name]; dup {
		panic("duplicate package " + name)
	}
	pkgs[name] = pkg{gen, run}
}

func main() {
	gen := flag.Bool("gen", false, "write the inputs: -gen <inputs>")
	flag.Parse()
	a := flag.Args()
	switch {
	case *gen && len(a) == 1:
		check(generate(a[0]))
	case !*gen && len(a) == 2:
		check(runAll(a[0], a[1]))
	default:
		flag.Usage()
		os.Exit(2)
	}
}

func check(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func names() []string {
	var n []string
	for k := range pkgs {
		n = append(n, k)
	}
	sort.Strings(n)
	return n
}

func generate(dir string) error {
	for _, n := range names() {
		d := filepath.Join(dir, n)
		if err := os.MkdirAll(d, 0o755); err != nil {
			return err
		}
		if err := pkgs[n].gen(d); err != nil {
			return fmt.Errorf("%s: %w", n, err)
		}
	}
	return nil
}

func runAll(in, out string) error {
	for _, n := range names() {
		files, err := os.ReadDir(filepath.Join(in, n))
		if err != nil {
			return err
		}
		od := filepath.Join(out, n)
		if err := os.MkdirAll(od, 0o755); err != nil {
			return err
		}
		for _, f := range files {
			data, err := os.ReadFile(filepath.Join(in, n, f.Name()))
			if err != nil {
				return err
			}
			for _, o := range pkgs[n].run(f.Name(), data) {
				p := filepath.Join(od, f.Name()+"."+o.name)
				if o.res.err != "" {
					err = os.WriteFile(p+".err", []byte(o.res.err+"\n"), 0o644)
				} else {
					err = os.WriteFile(p+".out", o.res.out, 0o644)
				}
				if err != nil {
					return err
				}
			}
		}
	}
	return nil
}

// rng is a xorshift64 generator seeded from a case name, so the inputs do not depend on
// math/rand or the Go version.
type rng uint64

func seed(name string) *rng {
	h := fnv.New64a()
	h.Write([]byte(name))
	r := rng(h.Sum64() | 1)
	return &r
}

func (r *rng) next() uint64 {
	x := uint64(*r)
	x ^= x << 13
	x ^= x >> 7
	x ^= x << 17
	*r = rng(x)
	return x
}

// intn returns a value in [0, n).
func (r *rng) intn(n int) int { return int(r.next() % uint64(n)) }
