package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/BurntSushi/toml"
)

// toml: BurntSushi/toml v1.4.0 Decode into map[string]any against gostd::toml. Inputs, 2,756
// files: the 556 toml-test .toml files (185 valid, 371 invalid; "t_" and the path under tests/
// with "/" as "__"), 200 generated manifests ("m000".."m199", Cargo.toml and pyproject.toml
// shapes) and 2,000 single-byte mutations of the manifests ("x0000".."x1999").
//
// The toml-test files are read from the module cache of the go-mods stage, where
// `go mod download` put BurntSushi/toml (the same directory the testinputs stage copies to
// /gotest/toml-test); the generator fails if it does not find exactly 556.
//
// Op dump: one line per leaf, sorted, each "<key path>\t<type>\t<value>". The key path is the
// segments joined by ".", a table key as strconv.Quote and an array index as "[i]"; the root
// has the empty path. Types: string (strconv.Quote), integer (decimal), float (FormatFloat 'g'
// -1), bool, datetime, datetime-local, date-local and time-local (the Go layouts of the plan).
// A []any is a line "array" with its length, a []map[string]any a line "aot" with its length
// (the elements follow as "[i]" tables), an empty table below the root a line "table 0". A
// decode error is its text; a Go panic (a BUG in the parser) is "panic".

func init() {
	register("toml", 2756, tmGen, tmRun)
}

// tmTestsDir is internal/toml-test/tests of the BurntSushi/toml v1.4.0 module in the module
// cache ($GOMODCACHE, else $GOPATH/pkg/mod, else ~/go/pkg/mod).
func tmTestsDir() string {
	mc := os.Getenv("GOMODCACHE")
	if mc == "" {
		gp := os.Getenv("GOPATH")
		if gp == "" {
			home, _ := os.UserHomeDir()
			gp = filepath.Join(home, "go")
		}
		mc = filepath.Join(gp, "pkg", "mod")
	}
	return filepath.Join(mc, "github.com", "!burnt!sushi", "toml@v1.4.0", "internal", "toml-test", "tests")
}

var tmHeaders = []string{
	"[package]", "[package]", "[dependencies]", "[dependencies]", "[dev-dependencies]",
	"[build-dependencies]", "[target.'cfg(unix)'.dependencies]",
	"[target.\"x86_64-pc-windows-gnu\".dependencies]", "[workspace]", "[workspace.dependencies]",
	"[workspace.package]", "[features]", "[lib]", "[[bin]]", "[[bin]]", "[[example]]",
	"[[workspace.members]]", "[project]", "[project.optional-dependencies]", "[build-system]",
	"[tool.poetry.dependencies]", "[tool.black]", "[[tool.mypy.overrides]]", "[a.b.c]", "[a]",
	"[ a . b ]", "[\"quoted header\"]", "[]", "[a", "[[a]",
}

var tmKeys = []string{
	"name", "version", "edition", "members", "dependencies", "features", "serde", "tokio",
	"my-crate", "rand_core", "\"quoted key\"", "'lit key'", "\"\"", "a.b", "a.b.c", "tool.x",
	"optional", "workspace.version", "x.\"y.z\"", "1234", "requires", "python", "a b",
}

var tmValues = []string{
	"\"1.0\"", "'^1.2'", "\">=3.8,<4\"", "\"\"", "''", "\"caf\\u00e9 \\\"q\\\" \\\\ \\t\"",
	"\"\"\"\nline1\nline2 \\\n   cont\"\"\"", "'''\nraw\\n'''", "1", "-5", "+3", "0", "0x1F", "0o17",
	"0b101", "1_000", "01", "1__0", "9223372036854775807", "9223372036854775808", "1.5", "-0.0",
	"1e3", "6.02e+23", "1e400", "inf", "-inf", "nan", "+nan", "1.", ".5", "true", "false", "True",
	"1979-05-27", "07:32:00", "1979-05-27T07:32:00Z", "1979-05-27 07:32:00.5-07:00",
	"1979-05-27T7:32:00", "2001-02-29", "[]", "[\"a\", \"b\"]", "[1, 2]", "[\"a\", 1]",
	"[[1, 2], [3]]", "[{ name = \"x\" }, { name = \"y\", v = 1 }]", "[\n  \"a\",\n  \"b\", # c\n]",
	"[1,,2]", "[1 2]", "{}", "{ version = \"1\", features = [\"a\"] }", "{ git = \"u\", branch = \"m\" }",
	"{ a.b = 1, a.c = 2 }", "{ a = 1, a = 2 }", "{ a = 1,\n b = 2 }", "{ workspace = true }",
	"\"unterminated", "'unterminated", "\"bad \\q escape\"", "\"\\ud800\"", "= 1", "foo",
}

var tmEq = []string{" = ", " = ", "=", " =  ", " ="}

var tmTrail = []string{"", "", "", "", " ", " # c", "\t"}

var tmComments = []string{"# Cargo.toml", "# pyproject.toml", "#", "# caf\u00e9", "#\ttab"}

func tmPick(r *rng, a []string) string { return a[r.intn(len(a))] }

// tmManifest builds one manifest: a few top-level keys, then 3 to 8 blocks, each a header and
// 0 to 4 key/value lines picked from small sets, so duplicates and redefinitions are common.
func tmManifest(r *rng) string {
	var b strings.Builder
	if r.intn(3) == 0 {
		for n := r.intn(3); n > 0; n-- {
			b.WriteString(tmPick(r, tmKeys) + tmPick(r, tmEq) + tmPick(r, tmValues) + "\n")
		}
	}
	for n := 3 + r.intn(6); n > 0; n-- {
		if r.intn(8) == 0 {
			b.WriteString(tmPick(r, tmComments) + "\n")
		}
		b.WriteString(tmPick(r, tmHeaders) + tmPick(r, tmTrail) + "\n")
		for k := r.intn(5); k > 0; k-- {
			b.WriteString(tmPick(r, tmKeys) + tmPick(r, tmEq) + tmPick(r, tmValues) + tmPick(r, tmTrail) + "\n")
		}
	}
	return b.String()
}

func tmManifestN(i int) string {
	return tmManifest(seed("toml/" + fmt.Sprintf("m%03d", i)))
}

// tmMutBytes are the bytes a mutation most often writes: TOML punctuation, signs, base and
// exponent letters, whitespace.
const tmMutBytes = "\"'[]{}=.,#\\\n\r\t _-+:0aAeExXoObBtTzZ9"

func tmGen(dir string) error {
	root := tmTestsDir()
	seen := map[string]bool{}
	n := 0
	for _, sub := range []string{"valid", "invalid"} {
		err := filepath.WalkDir(filepath.Join(root, sub), func(p string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if d.IsDir() || !strings.HasSuffix(p, ".toml") {
				return nil
			}
			rel, err := filepath.Rel(root, p)
			if err != nil {
				return err
			}
			name := "t_" + strings.ReplaceAll(strings.TrimSuffix(filepath.ToSlash(rel), ".toml"), "/", "__")
			if seen[name] {
				return fmt.Errorf("duplicate case %s", name)
			}
			seen[name] = true
			data, err := os.ReadFile(p)
			if err != nil {
				return err
			}
			n++
			return os.WriteFile(filepath.Join(dir, name), data, 0o644)
		})
		if err != nil {
			return err
		}
	}
	if n != 556 {
		return fmt.Errorf("toml-test: %d .toml files in %s, want 556", n, root)
	}
	for i := 0; i < 200; i++ {
		if err := os.WriteFile(filepath.Join(dir, fmt.Sprintf("m%03d", i)), []byte(tmManifestN(i)), 0o644); err != nil {
			return err
		}
	}
	for i := 0; i < 2000; i++ {
		name := fmt.Sprintf("x%04d", i)
		r := seed("toml/" + name)
		b := []byte(tmManifestN(r.intn(200)))
		c := tmMutBytes[r.intn(len(tmMutBytes))]
		if r.intn(4) == 0 {
			c = byte(r.intn(256))
		}
		b[r.intn(len(b))] = c
		if err := os.WriteFile(filepath.Join(dir, name), b, 0o644); err != nil {
			return err
		}
	}
	return nil
}

func tmJoin(path, seg string) string {
	if path == "" {
		return seg
	}
	return path + "." + seg
}

// tmTimeKind is the toml-test type and the Go layout of a decoded time.Time: the local kinds
// are the fixed zones internal.LocalDatetime, LocalDate and LocalTime, named as below.
func tmTimeKind(t time.Time) (string, string) {
	switch t.Location().String() {
	case "datetime-local":
		return "datetime-local", "2006-01-02T15:04:05.999999999"
	case "date-local":
		return "date-local", "2006-01-02"
	case "time-local":
		return "time-local", "15:04:05.999999999"
	}
	return "datetime", "2006-01-02T15:04:05.999999999Z07:00"
}

func tmDump(m map[string]any) []byte {
	var lines []string
	var walk func(path string, v any)
	walk = func(path string, v any) {
		switch x := v.(type) {
		case string:
			lines = append(lines, path+"\tstring\t"+strconv.Quote(x))
		case int64:
			lines = append(lines, path+"\tinteger\t"+strconv.FormatInt(x, 10))
		case float64:
			lines = append(lines, path+"\tfloat\t"+strconv.FormatFloat(x, 'g', -1, 64))
		case bool:
			lines = append(lines, path+"\tbool\t"+strconv.FormatBool(x))
		case time.Time:
			typ, layout := tmTimeKind(x)
			lines = append(lines, path+"\t"+typ+"\t"+x.Format(layout))
		case []any:
			lines = append(lines, path+"\tarray\t"+strconv.Itoa(len(x)))
			for i, e := range x {
				walk(tmJoin(path, "["+strconv.Itoa(i)+"]"), e)
			}
		case []map[string]any:
			lines = append(lines, path+"\taot\t"+strconv.Itoa(len(x)))
			for i, e := range x {
				walk(tmJoin(path, "["+strconv.Itoa(i)+"]"), e)
			}
		case map[string]any:
			if len(x) == 0 && path != "" {
				lines = append(lines, path+"\ttable\t0")
			}
			for k, e := range x {
				walk(tmJoin(path, strconv.Quote(k)), e)
			}
		default:
			lines = append(lines, fmt.Sprintf("%s\tother\t%T", path, v))
		}
	}
	walk("", m)
	sort.Strings(lines)
	var b strings.Builder
	for _, l := range lines {
		b.WriteString(l)
		b.WriteByte('\n')
	}
	return []byte(b.String())
}

func tmRun(_ string, in []byte) (ops []op) {
	defer func() {
		if r := recover(); r != nil {
			ops = []op{{"dump", result{err: "panic"}}}
		}
	}()
	var m map[string]any
	if _, err := toml.Decode(string(in), &m); err != nil {
		return []op{{"dump", result{err: err.Error()}}}
	}
	return []op{{"dump", result{out: tmDump(m)}}}
}
