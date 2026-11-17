package main

import (
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// math: math.Log against gostd::math::log. On amd64 (the go-run stage) math.Log is the assembly
// archLog of math/log_amd64.s, which gostd ports. Input "bits": 20,000 float64 bit patterns, one
// 16-digit hex number per line. Op log: the result's bits, one 16-digit hex number per line.
// One input, one op: 1 output file.

func init() {
	register("math", 1, mathGen, mathRun)
}

// mathBits: the special values, every power of two (subnormals included), sqrt(2)/2 times
// powers of two and their neighbours (archLog doubles f1 when f1 <= sqrt(2)/2), the neighbours
// of 1, 1,000 random subnormals, then random bit patterns, every other one with the sign bit
// cleared.
func mathBits() []uint64 {
	const total = 20000
	r := seed("math/bits")
	bits := math.Float64bits
	v := []uint64{
		0, 1 << 63, bits(1), bits(-1), bits(2), bits(10), bits(math.E), bits(0.5),
		bits(math.Inf(1)), bits(math.Inf(-1)), bits(math.NaN()),
		0x7FF0000000000001, 0x7FFFFFFFFFFFFFFF, 0xFFF8000000000000, 0xFFF0000000000001,
		bits(math.MaxFloat64), bits(math.SmallestNonzeroFloat64), 1<<52 - 1, 1 << 52,
		bits(-math.SmallestNonzeroFloat64),
	}
	for e := -1074; e <= 1023; e++ {
		v = append(v, bits(math.Ldexp(1, e)))
	}
	for e := -60; e <= 60; e++ {
		b := bits(math.Ldexp(math.Sqrt2/2, e))
		v = append(v, b-1, b, b+1)
	}
	for d := uint64(1); d <= 200; d++ {
		v = append(v, bits(1)+d, bits(1)-d)
	}
	for i := 0; i < 1000; i++ {
		v = append(v, r.next()&(1<<52-1))
	}
	for len(v) < total {
		x := r.next()
		if len(v)%2 == 0 {
			x &^= 1 << 63
		}
		v = append(v, x)
	}
	return v
}

func mathGen(dir string) error {
	var b strings.Builder
	for _, x := range mathBits() {
		fmt.Fprintf(&b, "%016x\n", x)
	}
	return os.WriteFile(filepath.Join(dir, "bits"), []byte(b.String()), 0o644)
}

func mathRun(name string, in []byte) []op {
	var b strings.Builder
	for _, l := range scLines(in) {
		x, err := strconv.ParseUint(l, 16, 64)
		if err != nil {
			panic(err)
		}
		fmt.Fprintf(&b, "%016x\n", math.Float64bits(math.Log(math.Float64frombits(x))))
	}
	return []op{{"log", result{out: []byte(b.String())}}}
}
