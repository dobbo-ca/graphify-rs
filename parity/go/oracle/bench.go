package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/BurntSushi/toml"
	"golang.org/x/text/unicode/norm"
)

// bench: `oracle -bench <inputs> <corpus>` times five workloads and prints "<workload> <total ns>"
// per line (stdout; checks go to stderr). crates/gostd/examples/bench.rs times the same workloads
// on gostd; parity/bench.sh joins the two. Keep the two files in step. Each workload runs once.
//
//	regexp_find_all     the 43 constant patterns of graphify-go (regexpConstants), FindAllStringIndex
//	                    over every line of <corpus> (compile is not timed). The plan counts 46
//	                    patterns: three more are built at run time (gitignore.go) and are not here.
//	json_marshal_indent MarshalIndent("", "  ") of a generated graph.json-shaped value (~50 MB compact)
//	json_compact        Compact of that indented text
//	json_valid          Valid of that indented text
//	json_decode         Unmarshal of that indented text into an any
//	norm_nfkc           NFKC of every <inputs>/xtext string, 20 times
//	sha256_1gib         1 GiB (1,024 writes of a 1 MiB pseudo-random chunk)
//	toml_decode_x100    Decode into map[string]any of the toml-test valid files
//	                    (<inputs>/toml/t_valid__*), 100 times
//
// <inputs> is the output of `oracle -gen`. The pseudo-random data comes from the xorshift64
// generator below, seeded with benchSeed; the Rust side has the same generator.

const (
	benchSeed  = 0x2545F4914F6CDD1D
	benchNodes = 150000
	benchLinks = 280000
)

func benchTime(name string, f func()) {
	t := time.Now()
	f()
	fmt.Printf("%s %d\n", name, time.Since(t).Nanoseconds())
}

func benchGraph() map[string]any {
	r := rng(benchSeed)
	nodes := make([]any, benchNodes)
	for i := range nodes {
		x := r.next()
		nodes[i] = map[string]any{
			"id":          fmt.Sprintf("n%d", i),
			"label":       fmt.Sprintf("sym_%x", x&0xffffff),
			"source_file": fmt.Sprintf("pkg/dir%d/file%d.go", (x>>24)%500, (x>>40)%50),
			"file_type":   "code",
			"community":   float64((x >> 48) % 1000),
		}
	}
	relations := []string{"calls", "imports", "contains", "references"}
	confidences := []string{"EXTRACTED", "INFERRED"}
	links := make([]any, benchLinks)
	for i := range links {
		x := r.next()
		links[i] = map[string]any{
			"source":     fmt.Sprintf("n%d", x%benchNodes),
			"target":     fmt.Sprintf("n%d", (x>>20)%benchNodes),
			"relation":   relations[(x>>40)%4],
			"confidence": confidences[(x>>44)%2],
			"weight":     float64((x>>48)%1000) / 1000,
		}
	}
	return map[string]any{"directed": false, "nodes": nodes, "links": links}
}

// benchFiles reads the files of <dir> (sorted by name) whose name starts with prefix.
func benchFiles(dir, prefix string) [][]byte {
	ents, err := os.ReadDir(dir)
	check(err)
	var out [][]byte
	for _, e := range ents {
		if strings.HasPrefix(e.Name(), prefix) {
			b, err := os.ReadFile(filepath.Join(dir, e.Name()))
			check(err)
			out = append(out, b)
		}
	}
	return out
}

func runBench(inputs, corpusPath string) error {
	corpus, err := os.ReadFile(corpusPath)
	if err != nil {
		return err
	}

	lines := strings.Split(string(corpus), "\n")
	res := make([]*regexp.Regexp, len(regexpConstants))
	for i, p := range regexpConstants {
		res[i] = regexp.MustCompile(p)
	}
	matches := 0
	benchTime("regexp_find_all", func() {
		for _, re := range res {
			for _, l := range lines {
				matches += len(re.FindAllStringIndex(l, -1))
			}
		}
	})
	fmt.Fprintln(os.Stderr, "regexp lines", len(lines), "matches", matches)

	graph := benchGraph()
	var indented []byte
	benchTime("json_marshal_indent", func() {
		var err error
		indented, err = json.MarshalIndent(graph, "", "  ")
		check(err)
	})
	var compact bytes.Buffer
	benchTime("json_compact", func() { check(json.Compact(&compact, indented)) })
	valid := false
	benchTime("json_valid", func() { valid = json.Valid(indented) })
	benchTime("json_decode", func() {
		var v any
		check(json.Unmarshal(indented, &v))
	})
	fmt.Fprintln(os.Stderr, "json indented", len(indented), "compact", compact.Len(), "valid", valid)

	var strs []string
	for _, b := range benchFiles(filepath.Join(inputs, "xtext"), "") {
		strs = append(strs, string(b))
	}
	nfkcLen := 0
	benchTime("norm_nfkc", func() {
		for k := 0; k < 20; k++ {
			for _, s := range strs {
				nfkcLen += len(norm.NFKC.String(s))
			}
		}
	})
	fmt.Fprintln(os.Stderr, "norm strings", len(strs), "bytes out", nfkcLen)

	chunk := make([]byte, 1<<20)
	r := rng(benchSeed)
	for i := range chunk {
		chunk[i] = byte(r.next())
	}
	var sum []byte
	benchTime("sha256_1gib", func() {
		h := sha256.New()
		for i := 0; i < 1024; i++ {
			h.Write(chunk)
		}
		sum = h.Sum(nil)
	})
	fmt.Fprintf(os.Stderr, "sha256 %x\n", sum)

	docs := benchFiles(filepath.Join(inputs, "toml"), "t_valid__")
	tomlErrs := 0
	benchTime("toml_decode_x100", func() {
		for k := 0; k < 100; k++ {
			for _, d := range docs {
				var m map[string]any
				if _, err := toml.Decode(string(d), &m); err != nil {
					tomlErrs++
				}
			}
		}
	})
	fmt.Fprintln(os.Stderr, "toml docs", len(docs), "errors", tomlErrs)
	return nil
}
