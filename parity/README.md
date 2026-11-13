# Parity

## gostd against Go (P1')

`crates/gostd` must give the same results as the Go code graphify-go v0.12.0 calls: Go std 1.27.4,
x/text v0.27.0, x/exp v0.0.0-20260603202125-055de637280b and BurntSushi/toml v1.4.0. Two checks
prove it: the Go tests ported to `crates/gostd/tests` (the count table below), and the oracle diff.

### The oracle

```
oracle -gen /inputs                Go writes the inputs: /inputs/<pkg>/<case>
oracle /inputs /go-out             Go runs every op
examples/oracle /inputs /rust-out  gostd runs every op
diff -r /go-out /rust-out          must be empty
```

- Go side: `parity/go/oracle` (`main.go` and one `<pkg>.go` per package, each registered in `init`).
  Its `go.sum` is a copy of graphify-go's, so the module versions are the ones graphify-go uses;
  the `go-mods` stage checks them with `go mod verify`.
- Rust side: `crates/gostd/examples/oracle/main.rs` and one `<pkg>.rs` per package, listed in `PKGS`.
- Output: `<out>/<pkg>/<case>.<op>.out` holds the output bytes; `<out>/<pkg>/<case>.<op>.err` holds
  the Go error text and a newline. Where graphify-go discards an error's text, the op writes a fixed
  word (`error`) instead, so only "error or not" is compared.
- Inputs are written only by Go, from its own xorshift64 seeded by the FNV-1a hash of the case
  name, so they do not change with Go's `math/rand`.
- A difference is a Rust bug. Never change the Go side to match Rust.
- CI: the `parity` and `parity-arm64` jobs (main and `workflow_dispatch`) build the stages of the
  same names in `ci/Dockerfile`. Each also reruns with `GOSTD_SIMD=scalar`. Each oracle package
  declares the fewest outputs it may write, beside its registration; both oracles fail below it
  (see the count table).

### Large Go test inputs

The `testinputs` stage builds `/gotest`, which the `test` and `arm64` stages bind-mount with
`GOSTD_GOTEST=/gotest`. A test that needs it panics if the variable is not set; it never skips.

| Path | Source |
|---|---|
| `/gotest/regexp/re2-exhaustive.txt` | `go1.27.4` `src/regexp/testdata/re2-exhaustive.txt.bz2`, decompressed |
| `/gotest/toml-test/` | `github.com/BurntSushi/toml@v1.4.0` `internal/toml-test/tests` (checked by `go.sum`) |

### Fuzz and bench

- Fuzz: `fuzz/` is its own cargo workspace (own `Cargo.lock`, `libfuzzer-sys`; the root workspace has `exclude = ["fuzz"]`, so `cargo deny` never sees it). Targets `json_syntax` (valid, compact, indent), `regexp_compile_match` and `toml_decode`; seeds in `fuzz/seeds/<target>/`. The `fuzz` workflow (`workflow_dispatch` only, input `max_total_time`, default 600 s) runs each target in the `fuzz` stage of `ci/Dockerfile` and uploads crash inputs as an artifact.
- Bench: the `bench` workflow (`workflow_dispatch`) builds the `bench` stage, which holds the Go oracle and `crates/gostd/examples/bench.rs`, and runs `sh parity/bench.sh`. Both sides time the same workloads (regexp: the 43 constant patterns over 20,000 lines of Go source; json: `marshal_indent`, `compact` and `valid` of a generated ~50 MB graph, decode is skipped until graphify-rs-aa6.10; norm: NFKC of the xtext inputs; sha256: 1 GiB; toml: the toml-test valid files x100). The table is in the job log and summary: Rust/Go ratio against the 1.10x target. It is not a gate.

### Count table

Source tests are Go `func Test*` (and, where noted, toml-test cases). "Not ported" names what is not
reached by graphify-go. "Passing" counts tests that run (not ignored) and pass in the `test` job.

One block per module, sorted by module name. A PR edits only its own module's block: it replaces
the placeholder row with its rows, keeps them up to date, and names its oracle package in the
block's first line. Each oracle package declares the fewest outputs it may write in its own
registration (`register(name, min, gen, run)` in `parity/go/oracle/<pkg>.go`, the `PKGS` entry in
`crates/gostd/examples/oracle/main.rs`); both oracles fail below it. No PR edits a shared number.

#### csv

Oracle package `csv`: inputs 2,000 generated records of 0 to 8 fields (the valid UTF-8 pieces of the strings alphabet plus `,` `"` `\r` `\n` `\.`); op `write` (Writer.Write then Flush).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `encoding/csv/writer_test.go`, `fuzz_test.go` | 3 | 2 (+1 added) | 3 | FuzzRoundtrip (no seeds, needs the reader); 8 of 29 writeTests rows (3 UseCRLF, 3 custom Comma, 2 invalid UTF-8 in `&str`); Reader and WriteAll not reached by graphify-go (Error is folded into flush's io::Result) |

#### filepath

Oracle package `filepath` (also covers `path`, the same code on unix): inputs `lex-0000`..`lex-4999` (5,000 cases of two generated paths over `{a, b, ., .., "", /, //}` up to 8 elements, a Go `matchTests` pattern and a name), `tree-00`..`tree-19` (20 tree descriptions: dot files, mixed case, symlinks to files and dirs, loops, an absolute link, a symlinked root; both oracles build the tree under `/tmp/oracle-tree/<case>`, run the ops and remove it); ops `clean`, `dir`, `base`, `ext`, `is_abs`, `join`, `rel`, `match` (lex), `walk` (SkipDir on names starting `s`), `glob` (10 patterns), `eval_symlinks` (tree; errors as a class, `notexist`, `notdir`, `toomany`, `other`). 40,060 outputs (5,000 x 8 + 20 x 3).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `path/filepath/path_test.go` | 34 | 19 (+2 added) | 21 | TestIsLocal, TestLocalize (IsLocal, Localize not reached by graphify-go); TestFromAndToSlash (identity on unix); TestSplitList, TestSplit (not reached); TestWalk, TestWalkSymlink, TestBug3486 (`Walk` not reached); TestWalkSkipAllOnFile (SkipAll not reached); TestAbsEmptyString, TestVolumeName, TestDriveLetterInEvalSymlinks (Windows); TestIssue29372, TestIssue51617 (not in the plan's list); TestEscaping (IsLocal not reached). Adapted, no chdir: TestAbs, TestEvalSymlinks*, TestRelativeSymlinkToAbsolute (absolute paths); TestWalkFileError (mode 0 directory in place of the `Lstat` hook); TestWalkDir without its PermErr subtest; Windows tables and AllocsPerRun left out |
| `path/filepath/match_test.go` | 8 | 6 | 6 | TestGlobUNC, TestWindowsGlob (Windows); TestGlob and TestNonWindowsGlobEscape use a temp dir, not the working directory |
| `path/path_test.go` | 8 | 6 | 6 | TestCleanMallocs (Go runtime AllocsPerRun); TestSplit (not reached) |

#### gen

UCD table generators (`crates/gostd-gen`); no oracle package of their own (the `unicode17` and `xtext` packages sweep the tables).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| x/text v0.27.0 `internal/export/unicode/gen.go`, `internal/ucd`, `unicode/rangetable` (no Go tests for gen.go) | - | 9 generator unit tests + 4 table tests (`tables_unicode17.rs`) | 13 | Output checked against Go 1.27.4 `unicode/tables.go`: 260 tables, 336 CaseRanges, 94 caseOrbit pairs, 42 CategoryAliases equal |
| x/text v0.27.0 `unicode/norm/maketables.go`, `cases/gen.go` (no Go tests for them) | - | 6 generator unit tests + 5 table tests (`tables_xtext15.rs`) | 11 | Full content proved by the `xtext` oracle sweep |

#### harness

Oracle package `echo`: inputs 4; ops `echo` (harness check).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `parity/go/oracle` (`echo` op) | - | - | - | - |

#### json

Oracle package `json` (encode side; the Value-tree ops come with graphify-rs-aa6.10): inputs `strings` (all 256 single bytes + 5,000 generated byte strings incl. invalid UTF-8, HTML chars, U+2028/9), `floats` (50,000 bit patterns), `docs-000`..`019` (10,000 generated JSON documents, ~15% damaged), `docs-edge` (12 documents that close past the top level and then open a new value; both must discard the output with Go's error); ops `html` (json.Marshal of a string; Go 1.27.4 runs the GOEXPERIMENT=jsonv2 code, so invalid UTF-8 is raw U+FFFD and syntax errors use the `jsontext` texts), `nohtml` (Encoder with SetEscapeHTML(false)), `json` (json.Marshal of a float64 or the error), `valid`, `compact`, `compact_html` (Compact then HTMLEscape), `indent` (prefix `>`, indent tab; errors as text).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `encoding/json/v2_encode_test.go` (+ TestMarshal, TestMarshalInvalidUTF8 from v2_decode_test.go) | 34 + 2 | 15 + 2 (+2 added: invalid_utf8_per_byte, float_format) | 19 | omitzero, `,string`, embedded/anonymous fields, `[]byte` base64, non-string map keys, pointer cycles, Number, reflect-only and synctest tests; 18 TestMarshalRawMessageValue rows for an empty non-nil RawMessage (graphify-go gets RawMessage only from decoding) |
| `encoding/json/v2_scanner_test.go` (+ v2_decode_test.go:480-496 syntax rows, jsontext/encode_test.go:393) | 6 | 6 (+4 added: indent_close_after_top_level_value, compact_and_indent_errors, max_depth, indent_placeholder_patch; +2 unit tests in src/json/scanner.rs: check_valid_errors, check_valid_truncated_number) | 12 | - (TestCompactBig/TestIndentBig use a fixed-seed port of Go's generator) |
| `encoding/json/tagkey_test.go`, `tags_test.go` | 1 + 1 | 1 (11 of 16 rows) | 1 | 5 rows test Go's tag-parse fallback to the field name; TestTagParsing tests the internal parseTag |
| `encoding/json/v2_stream_test.go` | 14 | 6 (+1 added: marshal_indent_prefix) | 7 | Decoder, Token, Buffered, InputOffset, MaxBytesError, blocking and HTTP tests (Decoder not used by graphify-go); the decode halves of TestRawMessage/TestNullRawMessage are ported as test_raw_message_decode / test_null_raw_message_decode (ignored until graphify-rs-aa6.10) |
| `encoding/json/v2_decode_test.go` (decode side) | 33 | 17 (+1 added: v1_compat_rules) | 1 (TestUnmarshalTypeErrorMessage; the rest ignored until graphify-rs-aa6.10) | Number/UseNumber, `,string` (graphify-go has no `,string` tag), embedded and unexported fields, `[]byte` base64, non-string map keys, `**T` targets, method-set, `time.Parse` and InvalidUnmarshal tests; TestUnmarshal keeps 100 of 178 rows (78 left out by class, listed in the file header) |
| `encoding/json/v2/fold_test.go` | 3 | 1 (TestEqualFold, 37 of 38 rows, as a field-matching test under the v1 options) | 0 | TestFoldRune checks the internal foldRune (under the v1 options matching reduces to strings.EqualFold); TestBenchmarkUnmarshalUnknown uses the v2 `case:ignore` tag; 1 row has invalid UTF-8 in a key |

#### pcg

Oracle package `pcg`: seeds 0, 1, 42, u64::MAX, 2^63; ops `uint64` (10,000 values), `uint64n` (10 bounds × 1,000), `louvain` (louvain.go:131 shuffle, seed 42, n = 2..=1000).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| x/exp v0.0.0-20260603202125-055de637280b `rand/{regress,arith128,modulo}_test.go` | 6 | 6 (TestRegress: Intn, Uint64n, Uint64 rows only) | 6 | rand_test.go funcs (Normal/Exp values and tables, Float32, Read*, ShuffleSmall, PCGSourceRoundTrip: not reached by graphify-go); race_test.go TestConcurrent (LockedSource not used by graphify-go) |

#### regexp

Oracle package `regexp`: inputs `c00`..`c42` (the 43 constant graphify-go patterns), `i00`..`i42` (the same with `(?i)`), `g0000`..`g1999` (generated, 10% invalid), each with 22 subjects incl. invalid UTF-8 and a 4 KiB subject; `quote_meta`; ops `compile` (ok or Go's error text), `match`, `find_index`, `find_submatch`, `find_all` (n = -1, 0, 2), `find_all_index`, `find_all_submatch`, `bytes_submatch_index`, `bytes_all_submatch_index`, `replace` (`_`, ``, `$1x`, `${1}`, `$$`, `${name}`), `replace_func`, `split` (n = -1, 0, 3), `quote_meta`.

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `regexp/syntax/parse_test.go` | 10 | 9 (+1 added) | 10 | TestAppendRangeCollapse (internal appendRange); 4 invalidRegexps rows with invalid UTF-8 (`&str` input) |
| `regexp/syntax/prog_test.go` | 1 | 1 | 1 | - |
| `regexp/syntax/simplify_test.go` | 1 | 1 | 1 | - |
| `regexp/all_test.go` | 18 | 10 (+1 added: 43 graphify-go patterns compile) | 11 | TestCopyMatch, TestReplaceAllLiteral, TestLiteralPrefix, TestUnmarshalText (not reached); TestDeepEqual (reflect); TestMinInputLen (internal); TestOnePassCutoff, TestSwitchBacktrack (engines deferred to P4, aa6.8) |
| `regexp/find_test.go` | 18 | 9 | 9 | Find, FindIndex, FindAll, FindAllIndex, FindStringSubmatchIndex, FindAllSubmatch, FindAllStringSubmatchIndex and the Reader variants (not reached) |
| `regexp/exec_test.go` (re2-search.txt, Fowler .dat files) | 4 | 2 | 2 | TestLongest (POSIX leftmost-longest not reached); TestProgramTooLongForBacktrack (aa6.8) |
| `regexp/exec2_test.go` (re2-exhaustive.txt from `GOSTD_GOTEST`) | 1 | 1 | 1 (release-mode RUN of the ci stage) | the two Longest runs of each case |
| `regexp/onepass_test.go` | 3 | 0 | 0 | onepass engine deferred to P4 (aa6.8) |

#### sha256

Oracle package `sha256`: inputs 131 `lenNNNN` (0 to 130 bytes), 100 `bigNNN` (random, up to 1 MiB), 200 `splitNNN`; ops `sum` (lowercase hex of Sum256), `split` (split cases: New, Write at the offsets, Sum).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `crypto/sha256/sha256_test.go` (go1.27.4) | 13 | 6 (+2 added: FIPS vectors, `tiers_agree`) | 8 | TestGoldenMarshal, TestMarshalTypeMismatch (marshal/unmarshal not reached); TestAllocations, TestAllocatonsWithTypeAsserts (Go allocation counts); TestCgo (cgo); TestHash (`cryptotest.TestHash` hash.Hash helper); TestExtraMethods (`cryptotest.NoExtraMethods`: Go method-set check). TestLargeHashes ported by finalizing the saved state in the test, not via unmarshal; SHA-224 rows of TestGolden and TestSize skipped |

#### sort

Oracle package `sort`: inputs 400 generated key lists (sizes 0 to 10,000; alphabets of 1, 2, 3, 1,000 keys, presorted, reversed, sawtooth, the TestAdversary pattern); ops `slice` (sort.Slice), `sort_func` (slices.SortFunc): output permutation and the less/cmp call trace; `stable` (sort.SliceStable vs std `sort_by`): permutation only.

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `sort/sort_test.go` | 21 | 15 | 15 | TestSortFloat64Slice, TestSortFloat64sCompareSlicesSort, TestFloat64s (floats with NaN, graphify-go sorts no floats); TestReverseSortIntSlice (sort.Reverse, sort.Interface only); TestHeapsortBM (Heapsort called directly, reached only through pdqsort); TestStableBM (Swap bound of sort.Stable symMerge, std `sort_by` used); TestSortBM without its Swap budget; TestSortLarge_Random at the testing.Short() size |
| `slices/sort_test.go` | 13 | 7 | 7 | TestSortFloat64Slice (floats, graphify-go sorts no floats); TestBinarySearchFloats (BinarySearch on floats with NaN, not called by graphify-go); TestMinMax, TestMinMaxNaNs, TestMinMaxPanics (slices.Min/Max not called); TestBinarySearchInts (same path as TestBinarySearch, graphify-go calls only BinarySearchFunc); TestSortLarge_Random at the testing.Short() size; TestSortIntSlice and TestSortStringSlice run through `sort_func`, not slices.Sort |

#### strconv

Oracle package `strconv`: inputs `floats` (200,000 bit patterns), `parse-000`..`019` (20,000 strings), `ints` (3,000), `quote-000`..`004` (5,000 byte strings), `sweep`; ops `g`, `e`, `f`, `upper_e`, `upper_g` (FormatFloat prec -1), `parse_float` (bits or error), `atoi`, `parse_int_0_64`, `parse_int_10_32`, `parse_uint_10_32`, `parse_uint_0_64`, `quote`, `quote_runes` (every scalar value).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `strconv/number_test.go`, `strconv/strconv_test.go` | 26 + 3 | 18 + 1 | 19 | Bool, Complex (4 + 3 funcs); TestNumErrorUnwrap (Go errors.Is); malloc counts; rows with base -1 / bit size -1 and bases other than 10 |
| `strconv/quote_test.go` | 11 | 2 (+1 added) | 3 | IsGraphic, QuoteToASCII, QuoteToGraphic, QuoteRune*, CanBackquote, Unquote* (not reached) |
| `internal/strconv/ftoa_test.go` | 4 | 3 (+1 added: the shortest-digit tie) | 4 | TestFormatFloatInvalidBitSize; 111 of 148 ftoatests rows (fixed precision, 'b', 'x', invalid fmt) |
| `internal/strconv/atof_test.go` | 7 | 5 | 5 | TestRoundTrip32, TestParseFloatIncorrectBitSize (float32, bit size); ParseFloatPrefix reduced to whole-string parses |
| `internal/strconv/fp_test.go` (atof1k.txt) | 3 | 1 | 1 | TestFp, TestFormatFloatTestdata (every line is fixed precision) |
| `internal/strconv/{atoi,itoa}_test.go` | 16 | 0 | 0 | the same tables as number_test.go, ported there |
| `internal/strconv/{decimal,math}_test.go` | 5 | 0 | 0 | internals (decimal fallback, log helpers) |

#### strings

Oracle package `strings`: inputs 2,000 generated byte strings; ops `to_valid_utf8` (U+FFFD and empty replacement), `bytes_trim_space`, `decode` (DecodeRune and DecodeLastRune walks, RuneCount); on valid UTF-8 also `to_lower`, `to_upper`, `trim_space`, `fields`, `equal_fold` (13 partners).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `strings/strings_test.go` | 41 | 7 | 7 | the 34 funcs that are not Unicode-table functions graphify-go calls; rows with invalid UTF-8 in `&str` inputs |
| `bytes/bytes_test.go` | 1 (TestTrimSpace) | 1 | 1 | - |

#### toml

Oracle package `toml`: 2,756 inputs (the 556 toml-test `.toml` files, read from the Go module cache of the `go-mods` stage; 200 generated Cargo.toml and pyproject.toml manifests; 2,000 single-byte mutations of them); op `dump` (one sorted line per decoded leaf, `<key path>\t<type>\t<value>`, or the error text).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `BurntSushi/toml@v1.4.0/decode_test.go` | 27 | 8 (+1 added) | 9 | 19 not reached by graphify-go (it decodes into `map[string]any` only): DecodeReader, DecodeFS, struct destinations (Embedded, IgnoreFields, Pointers, ArrayWrongSize, SizedInts, Types, DoubleTags), Unmarshaler, CustomDecode, Primitive, TextUnmarshaler, Duration, JSONNumber, MetaDotConflict, MetaKeys (MetaData), CustomEncode (encoder), DecodeParallel; the 8 ported are adapted where Go asserts on a struct destination |
| `BurntSushi/toml@v1.4.0/toml_test.go` | 3 (toml-test 180 valid + 371 invalid) | 2 (TestToml, TestTomlNextFails; errorTests as `error_tests`) | 3 | TestTomlNext (TOML 1.1, never enabled by graphify-go); the encode half and `metaTests` (encoder and MetaData not reached); 16 invalid cases that Go's `SkipTests` skips (355 of 371 invalid asserted) |

#### unicode

Oracle package `unicode17`: inputs `sweep`; ops `props` (every scalar value: IsLetter, IsNumber, IsUpper, IsSpace, IsPrint, Is(Mn), ToLower, ToUpper, ToTitle, SimpleFold), `tables` (member runs of every Categories, Scripts, Properties, FoldCategory, FoldScript table), `aliases`.

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `unicode/letter_test.go` | 14 | 10 (+1 added) | 11 | TestTurkishCase, TestSpecialCaseNoMapping (SpecialCase); TestCalibrate (Go perf); TestNegativeRune (no negative `char`) |
| `unicode/graphic_test.go` | 10 | 5 | 5 | IsControl, IsLower, IsGraphic, IsPunct, IsSymbol |
| `unicode/digit_test.go` | 2 | 0 | 0 | IsDigit |
| `unicode/script_test.go` | 2 | 2 (+1 added) | 3 | row `{0xdb80, "Cs"}` (surrogate `char`) |

#### url

Oracle package `url`: inputs 113 URLs from the Go test tables (`urltests`, TestParseErrors, and the TestParseFailure, TestStarRequest, TestNilUser, TestInvalidUserPassword and TestRejectControlCharacters strings), `g0000`..`g2999` (3,000 generated: schemes, userinfo, IPv4, IPv6 with and without %25 zones, bad ports, CTL bytes, `?`, `#`, `//`, a colon in the first segment), `unescape-0000`..`unescape-1999` (2,000 %-strings); ops `parse` (`%q` of Scheme, Hostname() and Port(), or the error text), `path_unescape` (the bytes or the error text). 5,113 outputs.

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `net/url/url_test.go` | 29 | 10 | 10 | 19 not reached by graphify-go (it calls `Parse` and `PathUnescape` only): TestParseRequestURI (`ParseRequestURI`), TestURLString and TestURLRedacted (`String`, `Redacted`), TestQueryEscape, TestPathEscape and TestShouldEscape (escaping), TestEncodeQuery, TestQueryValues, TestParseQuery and TestParseQueryLimits (`Values`, `ParseQuery`), TestResolvePath, TestResolveReference and TestJoinPath (resolve, join), TestRequestURI, TestURLClone and TestValuesClone, TestURLErrorImplementsNetError (`net.Error`), TestJSON and TestGob (marshalling); 1 of 62 `urltests` rows (empty `in`, skipped by Go); `OmitHost` not a `Url` field; TestParseFailure, TestStarRequest, TestNilUser and TestRejectControlCharacters adapted (no `ParseQuery`, `RequestURI`, nil `*Userinfo` or non-UTF-8 `&str`) |

#### utf8

Covered by the `strings` oracle package (`decode`: DecodeRune and DecodeLastRune walks, RuneCount).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `unicode/utf8/utf8_test.go` | 15 | 6 (+1 added) | 7 | Constants, FullRune, EncodeRune, AppendRune, RuntimeConversion, NegativeRune, RuneCountNonASCIIAllocation, RuneLen, ValidRune (Rust std encodes runes) |

#### xml

Oracle packages `xml-dec` (3,300 inputs: `p000`..`p299` generated poms with namespaces, entities, CDATA, comments, DOCTYPE, encoding declarations, CRLF and deep nesting (`p298`, `p299` are 10,000 and 10,001 deep), `m0000`..`m2999` single-byte mutations of them; op `elem`: one line per element, or `error`) and `xml-enc` (500 inputs `g000`..`g499`: GraphML node and edge sets over a mixed alphabet incl. control bytes, U+2028, characters outside the XML range and 4-byte runes; op `graphml`: `HEADER` + indented tree + newline).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `encoding/xml/xml_test.go` (go1.27.4) | 42 | 21 | 21 | not reached by graphify-go (non-strict mode, AutoClose, HTMLEntity, CopyToken, EncodeToken, Token wrappers): DecodeEOF, DecodeNilToken, NonStrictRawToken, RawTokenAltEncoding, AllScalars, Issue569, UnquotedAttrs, ValuelessAttrs, CopyTokenCharData, CopyTokenStartElement, CopyTokenComment, EscapeTextIOErrors, Issue5880, Issue8535, EncodeXMLNS, Issue7113, NewTokenDecoderIdempotent, WrapDecoder, TokenUnmarshaler, RoundTrip, HTMLAutoClose |
| `encoding/xml/read_test.go` (go1.27.4) | 23 | 6 (+1 added: `pom_shapes`) | 7 | not reached by graphify-go (only the `xmlElem` shape is decoded): UnmarshalFeed, UnmarshalPaths, UnmarshalBadPaths, UnmarshalWithoutNameType, UnmarshalAttr, UnmarshalNS, MarshalNS, UnmarshalNSAttr, MarshalNSAttr, Unmarshaler, UnmarshalIntoInterface, InvalidInnerXMLType, UnmarshalEmptyValues, UnmarshalWhitespaceValues, UnmarshalWhitespaceAttrs, UnmarshalXMLRawToken; UnmarshalIntoNil (no Rust equivalent) |
| `encoding/xml/marshal_test.go` (go1.27.4) | 18 | 1 (TestMarshal: 24 rows incl. the escaping rows) (+1 added: `escape_rules`) | 2 | not reached by graphify-go or no Encoder/writer API: MarshalErrors, Unmarshal (decodes marshalTests into typed structs), MarshalIndent (its only row is `,innerxml`), MarshalWriteErrors, MarshalWriteIOErrors, MarshalFlush, StructPointerMarshal, EncodeToken, ProcInstEncodeToken, DecodeEncode, Race9796, IsValidDirective, SimpleUseOfEncodeToken, Issue16158 (`,attr` into a byte field), InvalidXMLName, MarshalZeroValue, Close |

#### xtext

Oracle package `xtext`: inputs `sweep`, `cgj-31`, 12 fixed `q*` cases, 5,000 generated strings; ops `fold`, `nfkc`, `nfkd` (sweep: every scalar value), `fixpoint` (graphify MakeID: NFKC(fold(s)) until unchanged, at most 6 times).

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| x/text v0.27.0 `cases/fold_test.go`, `cases/context_test.go` | 7 | 2 (+1 added) | 3 | TestCaseProperties, TestCCC, TestContext (internals); TestMapping, TestWordBreaks (Lower/Upper/Title casers not called) |
| x/text v0.27.0 `unicode/norm/normalize_test.go` | 13 | 4 (+1 added) | 5 | 9 internals (boundaries, Span, IsNormal, DecomposeSegment, Linking); 28 NFKC append rows with invalid UTF-8 parts; NFC/NFD tables |
| x/text v0.27.0 `unicode/norm/composition_test.go` | 4 | 3 | 3 | TestFlush (internals); 2 NFD rows not observable through NFKD |
| x/text v0.27.0 `unicode/norm/ucd_test.go` (NormalizationTest.txt 15.0.0) | 3 | 2 | 2 | TestPerformance (Go timing); NFC/NFD columns |

## bedrock (P1')

`crates/bedrock` makes graphify-go's Bedrock call with anthropic-sdk-go v1.51.0 behaviour. There is
no oracle stage of its own (plan decision 6); the evidence is the ported tests below, the aws-c-auth
SigV4 vectors, the request-body golden `crates/bedrock/testdata/request_body.json`, and the gostd
oracle package `bedrock-body` (graphify-rs-1kw, minimum 201: the golden note and 200 generated
notes): Go 1.27.4 with anthropic-sdk-go v1.51.0 marshals graphify-go's `MessageNewParams`, and the
bytes must equal `bedrock::request_body`. Plan: `docs/superpowers/plans/2026-11-13-bedrock.md`.

Passing counts a test that runs (not ignored) and is green on main. Tests land ignored with their
bead id (graphify-rs-u2o.3 … .8) and lose the ignore in that bead's PR.

| Source | Functions | Ported | Passing | Skipped (reason) |
|---|---|---|---|---|
| graphify-go `internal/semantic/bedrock_test.go`, `semantic_test.go` | 13 | 0 | 0 | 13: P2, bead graphify-rs-ihc.1 (need idutil, model, detect, cache) |
| anthropic-sdk-go `client_test.go` | 10 | 3 (`TestContextDeadline` adapted to the attempt timeout) | 3 | 7: User-Agent value, header-override options, context cancel API, streaming |
| anthropic-sdk-go `internal/requestconfig/requestconfig_test.go` | 3 | 3 (adapted to `Error::Api`) | 3 | 0 |
| anthropic-sdk-go `bedrock/bedrockmantle_test.go` | 20 | 13 (7 port, 6 adapt) | 13 | 7: SkipAuth, Beta API, user request options, middleware |
| anthropic-sdk-go `message_test.go` | 10 | 1 (`TestCalculateNonStreamingTimeout`: 600 s) | 1 | 9: count tokens, streaming, model limits, param decode |
| aws-sdk-go-v2 v1.30.3 `aws/signer/v4`, `internal/v4` tests | 9 in scope | 9 | 9 | presign, middleware, custom URI escape, Opaque-URL cases of `TestGetURIPath` |
| aws-c-auth v0.9.0 signing vectors (via aws-sigv4 1.6.5) | 38 + 2 migrated | 40 (header files only) | 40 | `query-*` (presign) files not copied |
| aws-sdk-go-v2 internal/ini v1.8.0 `TestValidDataFiles` | 1 (20 fixtures) | 1 | 1 | 0 |
| aws-sdk-go-v2 config v1.27.27 env, shared config, process creds, credential source | 16 in scope | 15 | 15 | `TestSetEnvValue`; role, SSO, IMDS and web-identity cases (D-006: those go to the AWS CLI) |
| aws-sdk-go-v2 credentials v1.17.27 `processcreds` | 11 | 11 (`TestProviderWithAccountID` does not check the account id: not used) | 11 | 0 |
| golang.org/x/net v0.55.0 `http/httpproxy/proxy_test.go` | 7 | 7 (`https://`/`socks5://` rows adapted to D-007) | 7 | 0 |
| New (no source): SHA-256, HMAC, time, JSON, HTTP/TLS, proxy tunnel, retry rules, CLI fallback, Review Focus | 64 | — | 64 | — |

## graphify against graphify-go (P2)

`crates/graphify` must write the same `graphify-out/` bytes and the same command transcripts as graphify-go v0.12.0 (35fcd9e) built with Go 1.27.4 (`GOAMD64=v1`, `CGO_ENABLED=0`). Two checks prove it: the Go tests (and the upstream graphify v0.9.107 tests where Go has the behaviour) ported to `crates/graphify/src/<module>/tests/`, and the `graphify-parity` CI stage (plan Task 23). Plan: `docs/superpowers/plans/2026-11-13-p2-graphify.md`; per-file test map: `docs/superpowers/plans/2026-11-13-p2-graphify-testmap.md`.

One block per module, in the plan's module order. A PR edits only its own module's block (the `extract` and `cli` blocks: only its own row). "Passing" counts tests that run (not ignored) and pass in the `test` job. Tests land ignored with the bead id of the PR that makes them pass.

#### fsutil

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `fsutil` (`graphify-rs-ihc.3`) | - | - | - | not started |

#### idutil

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `idutil` (`graphify-rs-ihc.4`) | - | - | - | not started |

#### langfamily

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `langfamily` (`graphify-rs-ihc.5`) | - | - | - | not started |

#### security

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `security` (`graphify-rs-ihc.6`) | - | - | - | not started |

#### model

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `model` (`graphify-rs-ihc.7`) | - | - | - | not started |

#### graph

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `graph` (`graphify-rs-ihc.8`) | - | - | - | not started |

#### cluster

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `cluster` (`graphify-rs-ihc.9`) | - | - | - | not started |

#### analyze

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `analyze` (`graphify-rs-ihc.10`) | - | - | - | not started |

#### report

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `report` (`graphify-rs-ihc.11`) | - | - | - | not started |

#### export

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `export` (`graphify-rs-ihc.12`) | - | - | - | not started |

#### query

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `query` (`graphify-rs-ihc.13`) | - | - | - | not started |

#### detect

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `detect` (`graphify-rs-ihc.14`) | - | - | - | not started |

#### ts

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `ts` (`graphify-rs-ihc.15`) | - | - | - | not started |

#### extract

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `extract-core` (`graphify-rs-ihc.16`) | - | - | - | not started |
| `extract-nogrammar` (`graphify-rs-ihc.17`) | - | - | - | not started |
| `extract-resolve` (`graphify-rs-ihc.18`) | - | - | - | not started |
| `extract-golang` (`graphify-rs-ihc.24`) | - | - | - | not started |
| `extract-python` (`graphify-rs-ihc.25`) | - | - | - | not started |
| `extract-js` (`graphify-rs-ihc.26`) | - | - | - | not started |
| `extract-java` (`graphify-rs-ihc.27`) | - | - | - | not started |
| `extract-csharp` (`graphify-rs-ihc.28`) | - | - | - | not started |
| `extract-c` (`graphify-rs-ihc.29`) | - | - | - | not started |
| `extract-cpp` (`graphify-rs-ihc.30`) | - | - | - | not started |
| `extract-hcl` (`graphify-rs-ihc.31`) | - | - | - | not started |
| `extract-rust` (`graphify-rs-ihc.32`) | - | - | - | not started |
| `extract-ruby` (`graphify-rs-ihc.33`) | - | - | - | not started |
| `extract-php` (`graphify-rs-ihc.34`) | - | - | - | not started |
| `extract-bash` (`graphify-rs-ihc.35`) | - | - | - | not started |
| `extract-scala` (`graphify-rs-ihc.36`) | - | - | - | not started |
| `extract-kotlin` (`graphify-rs-ihc.37`) | - | - | - | not started |
| `extract-julia` (`graphify-rs-ihc.38`) | - | - | - | not started |
| `extract-lua` (`graphify-rs-ihc.39`) | - | - | - | not started |
| `extract-zig` (`graphify-rs-ihc.40`) | - | - | - | not started |
| `extract-verilog` (`graphify-rs-ihc.41`) | - | - | - | not started |

#### cache

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `cache` (`graphify-rs-ihc.19`) | - | - | - | not started |

#### semantic

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `semantic` (`graphify-rs-ihc.1`) | - | - | - | not started |

#### cli

| Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|
| `cli-core` (`graphify-rs-ihc.20`) | - | - | - | not started |
| `cli-maint` (`graphify-rs-ihc.21`) | - | - | - | not started |
| `cli-serve` (`graphify-rs-ihc.22`) | - | - | - | not started |
