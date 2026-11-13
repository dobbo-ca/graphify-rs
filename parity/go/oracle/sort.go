package main

import (
	"cmp"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strconv"
	"strings"
)

// sort: sort.Slice (pdqsort_func), slices.SortFunc (pdqsortCmpFunc) and sort.SliceStable on 400
// generated inputs, one decimal key per line. Each element is (key, original index). The ops
// write the output permutation (original indices, space separated, one line) and, for the two
// unstable sorts, the less/cmp call trace ("i j" per call, original indices, in call order).
// sort.SliceStable is permutation only: the Rust side uses std sort_by.

type sortElem struct{ key, idx int }

var sortSizes = []int{0, 1, 2, 12, 13, 19, 20, 21, 50, 100, 1000, 10000}

const sortCases = 400

// sortAdversary is TestAdversary's pattern: it runs sort.Slice against a comparator that
// freezes values as late as possible and returns the values it assigned, by original index.
func sortAdversary(n int) []int {
	gas := n
	val := make([]int, n)
	x := make([]int, n)
	for i := range val {
		val[i] = gas
		x[i] = i
	}
	nsolid, candidate := 0, 0
	sort.Slice(x, func(i, j int) bool {
		a, b := x[i], x[j]
		if val[a] == gas && val[b] == gas {
			if i == candidate {
				val[a] = nsolid
			} else {
				val[b] = nsolid
			}
			nsolid++
		}
		if val[a] == gas {
			candidate = i
		} else if val[b] == gas {
			candidate = j
		}
		return val[a] < val[b]
	})
	return val
}

func sortParse(in []byte) []sortElem {
	var x []sortElem
	for i, f := range strings.Fields(string(in)) {
		k, err := strconv.Atoi(f)
		if err != nil {
			panic(err)
		}
		x = append(x, sortElem{k, i})
	}
	return x
}

func sortOut(x []sortElem, trace [][2]int) []byte {
	var b strings.Builder
	for i, e := range x {
		if i > 0 {
			b.WriteByte(' ')
		}
		b.WriteString(strconv.Itoa(e.idx))
	}
	b.WriteByte('\n')
	for _, t := range trace {
		fmt.Fprintf(&b, "%d %d\n", t[0], t[1])
	}
	return []byte(b.String())
}

func init() {
	register("sort", 1200, func(dir string) error {
		for i := 0; i < sortCases; i++ {
			name := fmt.Sprintf("c%03d", i)
			n, pat, round := sortSizes[i%12], (i/12)%8, i/96
			if n == 10000 && round > 0 {
				n = 5000
			}
			r := seed("sort/" + name)
			keys := make([]int, n)
			switch pat {
			case 0, 1, 2, 3: // random keys from an alphabet of 1, 2, 3 or 1000 values
				for k := range keys {
					keys[k] = r.intn([]int{1, 2, 3, 1000}[pat])
				}
			case 4: // presorted
				for k := range keys {
					keys[k] = k / (round + 1)
				}
			case 5: // reversed
				for k := range keys {
					keys[k] = (n - k) / (round + 1)
				}
			case 6: // sawtooth
				for k := range keys {
					keys[k] = k % (3 + 4*round)
				}
			case 7: // the TestAdversary pattern
				keys = sortAdversary(n)
			}
			var b strings.Builder
			for _, k := range keys {
				b.WriteString(strconv.Itoa(k))
				b.WriteByte('\n')
			}
			if err := os.WriteFile(filepath.Join(dir, name), []byte(b.String()), 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(_ string, in []byte) []op {
		a := sortParse(in)
		var ta [][2]int
		sort.Slice(a, func(i, j int) bool {
			ta = append(ta, [2]int{a[i].idx, a[j].idx})
			return a[i].key < a[j].key
		})
		f := sortParse(in)
		var tf [][2]int
		slices.SortFunc(f, func(p, q sortElem) int {
			tf = append(tf, [2]int{p.idx, q.idx})
			return cmp.Compare(p.key, q.key)
		})
		s := sortParse(in)
		sort.SliceStable(s, func(i, j int) bool { return s[i].key < s[j].key })
		return []op{
			{"slice", result{out: sortOut(a, ta)}},
			{"sort_func", result{out: sortOut(f, tf)}},
			{"stable", result{out: sortOut(s, nil)}},
		}
	})
}
