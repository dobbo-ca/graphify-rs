package main

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
)

// filepath: path/filepath on unix. Two kinds of case file in <inputs>/filepath.
//
// lex-0000..lex-4999 (5,000 cases, 8 ops each = 40,000 outputs): the input is four fields
// separated by 0x1F: path a, path b, a Match pattern and a Match name. a and b are up to 8
// elements of {a, b, ., .., "", /, //}; the pattern is one of the Go match_test.go patterns; the
// name is half the time one of the Go table names, half the time up to 8 elements of a small
// alphabet (a b c d x / - ] * \ and two non-ASCII runes). Ops clean, dir, base, ext (of a),
// is_abs (of a, true or false), join (a, b), rel (a, b: the path or the error text) and match
// (pattern, name: true, false or the error text).
//
// tree-00..tree-19 (20 cases, 3 ops each = 60 outputs): the input describes a tree, one line per
// entry, fields separated by a tab: `root<TAB>R` first (the walk root is
// /tmp/oracle-tree/<case>/R, or the tree dir when R is empty; R is appended as written, so
// `top/` keeps its trailing slash), then `d<TAB>path`, `f<TAB>path` and `l<TAB>path<TAB>target`
// in creation order, paths relative to the tree dir. A target starting with `@` stands for the
// tree dir (`@/a/b` is `<tree dir>/a/b`). Both oracles build the tree under
// /tmp/oracle-tree/<case> from the description (the Rust run is a different Docker stage, so
// nothing built at gen time would exist), run the ops, and remove it. Every printed path is
// relative to the tree dir (`.` for the dir itself; a path outside it is printed as is).
// Ops: walk (WalkDir from the root, one `path<TAB>type` line per callback, type d, f or l, or
// `path<TAB>err`; SkipDir on every entry whose name starts with `s`), glob (10 patterns under
// the tree dir: a `# pattern` line, then the matches or `error: <text>`) and eval_symlinks (each
// entry, entry/.. and entry/nope: `path<TAB>result`, or `path<TAB>!class` with class notexist,
// notdir, toomany or other, because the OS error texts differ between Go and Rust).

const (
	fpLexCases  = 5000
	fpTreeCases = 20
	fpTreeDir   = "/tmp/oracle-tree"
)

// fpPatterns are the distinct patterns of match_test.go matchTests (non-ASCII as \u escapes).
var fpPatterns = []string{
	"abc", "*", "*c", "a*", "a*/b", "a*b*c*d*e*/f", "a*b?c*x", "ab[c]", "ab[b-d]", "ab[e-g]",
	"ab[^c]", "ab[^b-d]", "ab[^e-g]", "a\\*b", "a?b", "a[^a]b", "a???b", "a[^a][^a][^a]b",
	"[a-\xce\xb6]*", "*[a-\xce\xb6]", "a*b", "[\\]a]", "[\\-]", "[x\\-]", "[\\-x]", "[]a]", "[-]",
	"[x-]", "[-x]", "\\", "[a-b-c]", "[", "[^", "[^bc", "a[", "a/b[", "*x",
}

// fpNames are names from the same table.
var fpNames = []string{
	"abc", "ab/c", "a", "axbxcxdxe/f", "axbxcxdxexxx/f", "abxbbxdbxebxczzx", "a*b", "ab", "a\xe2\x98\xbab",
	"\xce\xb1", "A", "]", "-", "x", "z", "a/b", "xxx",
}

var fpNameAlphabet = []string{"a", "b", "c", "d", "x", "/", "-", "]", "*", "\\", "\xe2\x98\xba", "\xce\xb1"}

var fpPathAlphabet = []string{"a", "b", ".", "..", "", "/", "//"}

var fpNamePool = []string{
	"a", "b", "c", "A", "B", ".hid", ".git", "s1", "sx", "Sub", "x.txt", "y.TXT", "z.go", "dir",
	"d2", "long-name", "m.go",
}

var fpGlobs = []string{"*", ".*", "*/*", "*/*/*", "[A-Z]*", "s*/*", "?/*", "*/[a-c]*", "*.[gt]*", "["}

func fpJoinAlphabet(r *rng, alphabet []string) string {
	var b strings.Builder
	for n := r.intn(9); n > 0; n-- {
		b.WriteString(alphabet[r.intn(len(alphabet))])
	}
	return b.String()
}

type fpEntry struct {
	kind         byte
	path, target string
}

func fpLinkTarget(r *rng, parent string, paths []string) string {
	k := r.intn(10)
	if k == 0 || len(paths) == 0 {
		return "missing"
	}
	if k == 1 {
		return ".."
	}
	if k == 2 {
		return "."
	}
	t := paths[r.intn(len(paths))]
	if r.intn(2) == 0 {
		return "@/" + t
	}
	rel, err := filepath.Rel(filepath.Join("/x", parent), filepath.Join("/x", t))
	if err != nil {
		return "missing"
	}
	return rel
}

func fpGenTree(c int) string {
	r := seed(fmt.Sprintf("filepath/tree-%02d", c))
	base, root := "", ""
	var es []fpEntry
	var paths []string
	if c%5 == 0 {
		// a symlinked root: the tree lives in real/, the walk starts at the link top (or top/)
		base = "real"
		root = "top/"
		if c%10 == 0 {
			root = "top"
		}
		es = append(es, fpEntry{'d', "real", ""}, fpEntry{'l', "top", "real"})
		paths = append(paths, "real")
	}
	at := func(dir, n string) string {
		if dir == "" {
			return n
		}
		return dir + "/" + n
	}
	dirs := []string{base}
	used := map[string]bool{}
	for n := 14 + r.intn(18); n > 0; n-- {
		parent := dirs[r.intn(len(dirs))]
		p := at(parent, fpNamePool[r.intn(len(fpNamePool))])
		if used[p] {
			continue
		}
		used[p] = true
		switch k := r.intn(10); {
		case k < 3:
			es = append(es, fpEntry{'d', p, ""})
			if strings.Count(p, "/") < 3 {
				dirs = append(dirs, p)
			}
		case k < 6:
			es = append(es, fpEntry{'f', p, ""})
		default:
			es = append(es, fpEntry{'l', p, fpLinkTarget(r, parent, paths)})
		}
		paths = append(paths, p)
	}
	if c%4 == 1 {
		// symlink loops: a pair and a self link
		es = append(es, fpEntry{'l', at(base, "ca"), "cb"}, fpEntry{'l', at(base, "cb"), "ca"},
			fpEntry{'l', at(base, "self"), "self"})
	}
	if c%3 == 0 {
		// An absolute link out of the tree. Not "/": the Go and Rust containers have different roots.
		es = append(es, fpEntry{'l', at(base, "toroot"), fpTreeDir})
	}
	var b strings.Builder
	b.WriteString("root\t" + root + "\n")
	for _, e := range es {
		switch e.kind {
		case 'l':
			fmt.Fprintf(&b, "l\t%s\t%s\n", e.path, e.target)
		default:
			fmt.Fprintf(&b, "%c\t%s\n", e.kind, e.path)
		}
	}
	return b.String()
}

// fpBuild creates the tree of a description under dir and returns the walk root and the entry
// paths.
func fpBuild(dir, desc string) (root string, entries []string) {
	if err := os.RemoveAll(dir); err != nil {
		panic(err)
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		panic(err)
	}
	lines := strings.Split(strings.TrimSuffix(desc, "\n"), "\n")
	root = strings.TrimPrefix(lines[0], "root\t")
	for _, l := range lines[1:] {
		f := strings.Split(l, "\t")
		p := dir + "/" + f[1]
		var err error
		switch f[0] {
		case "d":
			err = os.Mkdir(p, 0o755)
		case "f":
			err = os.WriteFile(p, nil, 0o644)
		case "l":
			t := f[2]
			if strings.HasPrefix(t, "@") {
				t = dir + t[1:]
			}
			err = os.Symlink(t, p)
		}
		if err != nil {
			panic(err)
		}
		entries = append(entries, f[1])
	}
	return root, entries
}

// fpRel prints p relative to the tree dir.
func fpRel(dir, p string) string {
	if p == dir {
		return "."
	}
	if r, ok := strings.CutPrefix(p, dir+"/"); ok {
		return r
	}
	return p
}

func fpClass(err error) string {
	switch {
	case errors.Is(err, fs.ErrNotExist):
		return "notexist"
	case errors.Is(err, syscall.ENOTDIR):
		return "notdir"
	case err.Error() == "EvalSymlinks: too many links":
		return "toomany"
	}
	return "other"
}

func fpRunTree(name string, in []byte) []op {
	dir := fpTreeDir + "/" + name
	root, entries := fpBuild(dir, string(in))
	defer os.RemoveAll(dir)
	start := dir
	if root != "" {
		start = dir + "/" + root
	}

	var walk strings.Builder
	filepath.WalkDir(start, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			fmt.Fprintf(&walk, "%s\terr\n", fpRel(dir, p))
			return nil
		}
		t := "f"
		switch {
		case d.IsDir():
			t = "d"
		case d.Type()&fs.ModeSymlink != 0:
			t = "l"
		}
		fmt.Fprintf(&walk, "%s\t%s\n", fpRel(dir, p), t)
		if strings.HasPrefix(d.Name(), "s") {
			return fs.SkipDir
		}
		return nil
	})

	var glob strings.Builder
	for _, pat := range fpGlobs {
		fmt.Fprintf(&glob, "# %s\n", pat)
		m, err := filepath.Glob(dir + "/" + pat)
		if err != nil {
			fmt.Fprintf(&glob, "error: %s\n", err)
			continue
		}
		for _, p := range m {
			fmt.Fprintf(&glob, "%s\n", fpRel(dir, p))
		}
	}

	var eval strings.Builder
	for _, e := range entries {
		for _, suffix := range []string{"", "/..", "/nope"} {
			res, err := filepath.EvalSymlinks(dir + "/" + e + suffix)
			if err != nil {
				fmt.Fprintf(&eval, "%s\t!%s\n", e+suffix, fpClass(err))
			} else {
				fmt.Fprintf(&eval, "%s\t%s\n", e+suffix, fpRel(dir, res))
			}
		}
	}
	return []op{
		{"walk", result{out: []byte(walk.String())}},
		{"glob", result{out: []byte(glob.String())}},
		{"eval_symlinks", result{out: []byte(eval.String())}},
	}
}

func fpRunLex(in []byte) []op {
	f := strings.Split(string(in), "\x1f")
	if len(f) != 4 {
		panic("filepath: bad lex input")
	}
	a, b, pat, name := f[0], f[1], f[2], f[3]
	ops := []op{
		{"clean", result{out: []byte(filepath.Clean(a))}},
		{"dir", result{out: []byte(filepath.Dir(a))}},
		{"base", result{out: []byte(filepath.Base(a))}},
		{"ext", result{out: []byte(filepath.Ext(a))}},
		{"is_abs", result{out: []byte(strconv.FormatBool(filepath.IsAbs(a)))}},
		{"join", result{out: []byte(filepath.Join(a, b))}},
	}
	if r, err := filepath.Rel(a, b); err != nil {
		ops = append(ops, op{"rel", result{err: err.Error()}})
	} else {
		ops = append(ops, op{"rel", result{out: []byte(r)}})
	}
	if m, err := filepath.Match(pat, name); err != nil {
		ops = append(ops, op{"match", result{err: err.Error()}})
	} else {
		ops = append(ops, op{"match", result{out: []byte(strconv.FormatBool(m))}})
	}
	return ops
}

func init() {
	// 5,000 lex cases x 8 ops + 20 tree cases x 3 ops
	register("filepath", fpLexCases*8+fpTreeCases*3, func(dir string) error {
		for i := 0; i < fpLexCases; i++ {
			name := fmt.Sprintf("lex-%04d", i)
			r := seed("filepath/" + name)
			a := fpJoinAlphabet(r, fpPathAlphabet)
			b := fpJoinAlphabet(r, fpPathAlphabet)
			pat := fpPatterns[r.intn(len(fpPatterns))]
			var nm string
			if r.intn(2) == 0 {
				nm = fpNames[r.intn(len(fpNames))]
			} else {
				nm = fpJoinAlphabet(r, fpNameAlphabet)
			}
			in := strings.Join([]string{a, b, pat, nm}, "\x1f")
			if err := os.WriteFile(dir+"/"+name, []byte(in), 0o644); err != nil {
				return err
			}
		}
		for i := 0; i < fpTreeCases; i++ {
			name := fmt.Sprintf("tree-%02d", i)
			if err := os.WriteFile(dir+"/"+name, []byte(fpGenTree(i)), 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(name string, in []byte) []op {
		if strings.HasPrefix(name, "tree-") {
			return fpRunTree(name, in)
		}
		return fpRunLex(in)
	})
}
