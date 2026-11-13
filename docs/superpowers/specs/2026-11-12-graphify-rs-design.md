# graphify-rs and gositter-rs: design

Date: 2026-11-12. Owner: Chris. Status: approved 2026-11-12, revised after
builder review.

## 1. Goal

Replace graphify-go with a Rust port that is faster than graphify-go and keeps
performance under our own control.

- **gositter-rs** (`dobbo-ca/gositter-rs`, crate `gositter`): a pure-Rust port
  of gositter v0.2.0 (7a27b30), itself a pure-Go port of tree-sitter v0.25.0.
  All 21 grammars.
- **graphify-rs** (`dobbo-ca/graphify-rs`): a port of graphify-go v0.12.0
  (35fcd9e), then the upstream graphify 0.9.107 backlog on top.

graphify-go is paused and frozen at 35fcd9e. It is the oracle for phase 2 and is
decommissioned in phase 5. gositter (Go) is read only and is the oracle for
gositter-rs.

### Non-goals

- Publishing to crates.io.
- Tree-sitter queries or incremental parsing (gositter has neither).
- Byte parity of the incremental cache files (`.graphify_cache.json`,
  `.graphify_stat.json`) and `.graphify_root`. They hold timestamps, mtimes and
  absolute paths, and are compared by behaviour (round-trip and invalidation
  tests), not bytes.

## 2. Rules that apply (from dobbo-ca/rust-ports conventions)

- Edition 2024, toolchain 1.99.1 pinned in `rust-toolchain.toml`, `Cargo.lock`
  committed, `publish = false`.
- std first. Never `cc`, `bindgen`, `-sys` crates or FFI to C. The `test` CI stage
  runs cargo-deny with a ban list: `cc`, `cmake`, `bindgen`, `ring`, `aws-lc-*`,
  `openssl-sys`, `security-framework-sys`, `core-foundation-sys`.
- Every third-party crate is in the README with its reason.
- `unsafe` only in SIMD and hot kernels, each with a scalar oracle, an x86_64
  tier and an aarch64 tier, a `<CRATE>_SIMD=scalar` override and cross-tier
  parity tests. (For SHA-256 the tiers are SHA-NI and ARMv8 SHA2, chosen at
  run time by CPU feature detection.)
- No build, test, lint, fuzz or run on Chris's Mac. Edit, `cargo fmt`,
  `cargo generate-lockfile` and git only. CI builds on ARC runners with
  in-cluster BuildKit (copied from deflate-rs `ci/` and workflows: `ci.yml`,
  `bench.yml`, `fuzz.yml`).
- Branch `port-<area>-<4 hex>`, PR into main, squash-merge on green.
- Tests first: port and land the tests before the code. Sources are the Go
  tests, the gositter tests and oracle, and upstream Python tests for behaviour
  Go also has. A count table lives in `parity/README.md`.
- graphify-go and gositter are read only.

**Differences from the conventions, with reasons (also in each README):**

| Rule | Here | Reason |
|---|---|---|
| Repos private | Both public | Owner decision, 2026-11-12. |
| Tracked in rust-ports checklist | Tracked in each repo's beads | Owner decision: rust-ports is for image libraries only. |
| CI on ARC only | ARC for everything, plus GitHub-hosted macOS runners for darwin test and release jobs | ARC is Linux only, Mac builds are banned, and graphify ships darwin binaries. Hosted macOS is free for public repos. |
| Limits / cancel / max_alloc / max_threads | Library crates: yes. CLI: worker count from `available_parallelism`, no allocation cap | The CLI reproduces graphify-go behaviour, which has neither. |

## 3. Repos and layout

```
gositter-rs/                       graphify-rs/
  src/          runtime              crates/gostd/     Go-semantics std pieces
  gengrammar/   parser.c -> tables   crates/bedrock/   LLM client (HTTP, TLS, SigV4)
  grammars/<g>/ tables + scanner.rs  crates/graphify/  lib + `graphify` binary
  treedump/     dump format          parity/           oracles, graphdiff, ledger
  testdata/regress/ 1,423 goldens    testdata/         fixtures, goldens
```

graphify-rs depends on gositter-rs as a git dependency pinned by `rev`. Both
repos are public, so CI needs no token to fetch it. No file is in Git LFS:
cargo git dependencies do not fetch LFS objects.

### gositter-rs

- **Runtime** (about 6–7.5k lines): parser, subtree, stack, node, language,
  lexer, tree cursor, slab pool, `parse_budget`. Same C-exact contract as
  gositter's CONTRACT.md: same trees, same LOG lines, same stall count.
- **gengrammar** (about 2.5k lines): reads each grammar's `parser.c` (C
  subset), checks its sha256 against `grammars.lock`, and writes the tables.
  - gositter's `gen.sh` reads `parser.c` from the Go module cache. gositter-rs
    has a `gen` Dockerfile target instead: it downloads the 21 grammar modules
    named in `grammars.lock`, checks the sha256 of each `parser.c`, runs
    gengrammar, and exports the output with `outputs: type=local`. A
    `workflow_dispatch` job uploads that output as an artifact; it is committed
    from the Mac with git only. The `test` stage runs `gen` again and fails on
    any diff.
  - Parse tables are `include_bytes!` blobs with a small typed view. verilog's
    dense table is 16.6 MB, so source literals are not an option.
  - The lexer form (generated Rust `match` code or a data-driven state table)
    is chosen by a CI benchmark spike on bash, cpp and verilog before the other
    grammars are generated.
- **Scanners**: 15 hand ports of gositter's `scanner.go` (about 6.5k lines).
  Serialize bytes must stay byte-identical. C `char` signedness follows
  gositter's `cchar` (unsigned on linux arm64, signed elsewhere), so the
  `arm64` stage is required.
- **Goldens**: the 1,423 C-made `.dump.gz` files in `testdata/regress`. The
  Dockerfile gunzips them before the tests run, so gositter-rs needs no gzip
  code.
- **Go oracle**: gositter's own `oracle/` package uses cgo and imports
  `internal/treedump`. The gositter-rs `go-oracle` stage builds a pure-Go dump
  tool inside a copy of the gositter v0.2.0 module (`CGO_ENABLED=0`, so the C
  half is excluded), with `go mod download` checked against `go.sum`. Its dumps
  must equal ours on the regress inputs and on extra inputs.

### graphify-rs crates

| Crate | Contents | Go source |
|---|---|---|
| `gostd` | Go-exact pieces: Unicode 15 + 17 tables (fold, NFKC/NFKD, categories), `regexp` (RE2 port), `encoding/json` encode and decode, TOML, XML, CSV writer, `strconv`, `net/url` escaping, pdqsort, `filepath`, x/exp PCG, SHA-256 | Go std, x/text, x/exp, BurntSushi/toml |
| `bedrock` | HTTP/1.1 client, SigV4, credential chain, Messages call | anthropic-sdk-go bedrock path |
| `graphify` | detect, extract, resolve, build, cluster, analyze, export, report, query, cache, serve, CLI | `internal/*`, `cmd/graphify` |

`graphify` mirrors graphify-go's package layout one module per package, so a
reviewer can read the two side by side. UCD tables come from a `gen` Dockerfile
target, the same way as the grammar tables.

## 4. Dependency map

| Go dependency | Rust choice |
|---|---|
| gositter v0.2.0 | gositter-rs (own) |
| x/text cases, norm (Unicode 15) | `gostd::unicode` tables generated from UCD 15.0.0 |
| Go std `unicode` (Unicode 17) | `gostd::unicode` tables generated from UCD 17.0.0 |
| `regexp` (46 sites + user pattern in `query`) | `gostd::regexp`, a port of Go `regexp` and `regexp/syntax` |
| `encoding/json` | `gostd::json` (Go escaping, float format, `MarshalIndent`, v1 decode rules) |
| BurntSushi/toml | `gostd::toml` (dynamic value tree, TOML 1.0) |
| `encoding/xml` | `gostd::xml` (pom.xml decode, GraphML escaping) |
| `encoding/csv` | `gostd::csv` (writer only) |
| `net/url` (`PathUnescape` resolve.go:1367, `Parse` security.go:66) | `gostd::url` |
| `sort.Slice` | `gostd::sort`, a port of Go pdqsort (6 tie-prone sites) |
| x/exp/rand | `gostd::pcg` (PCGSource, `Intn`, `Uint64n`) |
| `crypto/sha256` | `gostd::sha256`, scalar plus SHA-NI and ARMv8 kernels |
| `path/filepath` | `gostd::filepath` (`Ext`, `Clean`, `Rel`, `WalkDir` order) |
| `crypto/rand` | read `/dev/urandom` |
| `os/exec` (git) | `std::process::Command` |
| `net`, CIDR check | `std::net` plus a small CIDR matcher |
| anthropic-sdk-go + AWS SDK | `bedrock` crate (own) |
| TLS | **third-party**: `rustls` (no default provider), `rustls-graviola` 0.4, `webpki-roots` |
| YAML (phase 3, B27) | **third-party if a pure-Rust crate fits** (see section 9) |

The three TLS crates are the only planned third-party dependencies before phase
3. A real handshake with graviola and webpki-roots against
`bedrock-mantle.us-east-1.api.aws` passed during design. graviola supports only
x86_64 (AVX2, ADX, BMI2) and aarch64, which covers the four release targets.

## 5. Byte parity with Go v0.12.0

The golden reference is graphify-go 35fcd9e built for linux/amd64 with
`GOAMD64=v1`, `CGO_ENABLED=0`, `GOTOOLCHAIN=local` and no ldflags. These Go
behaviours do not come for free in Rust, and each has a test in `gostd` or an
explicit decision:

| Hazard | Handling |
|---|---|
| `sort.Slice` ties at query.go:159, :215, affected.go:140, okf.go:64, :118, :219 | pdqsort port, same comparator call order |
| Two Unicode versions (17 in std, 15 in x/text) | both table sets, chosen per call site |
| Node ID: casefold + NFKC to a fixpoint (max 6 loops); `normLabel` NFKD, drop Mn, ToLower | ported as-is in phase 2 |
| JSON encode: `\u003c` style escapes, U+2028/9, invalid UTF-8 (Go 1.27 writes the raw bytes EF BF BD, not `\ufffd`), floats, `MarshalIndent` | `gostd::json`, ported from the jsonv2-backed v1 files (see below), with Go's encode test vectors |
| JSON decode (v1 API on the json/v2 engine): case-insensitive field match, last duplicate key wins, invalid UTF-8 to U+FFFD; syntax error texts and offsets from the jsontext walker; `UnmarshalTypeError` names the root type and a dotted JSON path (printed by graphify-go at query.go:114, validate.go:28, export.go:187, :248) | `gostd::json` with the active `v2_*_test.go` vectors |
| Go 1.27.4 enables `GOEXPERIMENT=jsonv2` by default: `encoding/json` is the v1 API backed by json/v2, and `scanner.go`, `decode.go`, `encode.go` are excluded by `//go:build !goexperiment.jsonv2` | port the active files only; check the `//go:build` lines of every ported Go file at Go 1.27.4 defaults (bead graphify-rs-aa6.20) |
| GODEBUG defaults come from the main module's `go` line: graphify-go says `go 1.25.0` (for example `urlstrictcolons=0`, the lenient URL colon parse) | every Go oracle sets `godebug default=go1.25` in its go.mod |
| RE2 semantics: leftmost-first, `\p{L}` | regexp port with Go's regexp tests |
| `strconv.Quote` in hook scripts | `gostd::strconv` |
| FMA on arm64 in ask.go (6 sites) | goldens are amd64, where Go does not fuse; Rust never fuses. `ask` scores on arm64 can differ from Go arm64 in the last bit (D-004) |
| Go map order: mcpconfig.go:113 env, manifest.go:279 Poetry deps | Rust sorts the keys (D-001, D-002). Go's order changes node order and so Louvain communities, so no output normalisation can hide it. These inputs stay out of the byte corpus and are covered by unit tests |
| Broken pipe: Go exits 0 (`sigpipe_unix.go:16`) | Rust ignores SIGPIPE, so every stdout write handles `EPIPE` and exits 0. Not a divergence |

**The parity gate in phase 2.** Both binaries run in BuildKit on the same
corpus, at the same absolute path, with the corpus `.git` removed (so
`built_at_commit` is empty on both sides). The gate compares:

1. `diff -r` of `graphify-out/`, excluding `.graphify_cache.json`,
   `.graphify_stat.json` and `.graphify_root`.
2. Command transcripts: stdout, stderr and exit code for `query`, `explain`,
   `path`, `god-nodes`, `affected`, `diff`, `validate`, `extract`, `ask`, the
   four `export` formats and `hook install` (the written hook files). These
   cover the sort ties, `strconv.Quote` and the ask scores, which do not reach
   `graphify-out/`.

### Divergence ledger (`parity/DIVERGENCES.md`), first entries

| ID | Divergence | Reason |
|---|---|---|
| D-001 | MCP env keys in sorted order | Go map order is random |
| D-002 | Poetry deps in sorted order | Go map order is random |
| D-003 | `watch` on SIGINT or SIGTERM ends with the default signal exit, not Go's clean exit | A handler needs FFI to libc |
| D-004 | `ask` scores on arm64 can differ from Go arm64 in the last bit | Go fuses multiply-add on arm64 |
| D-005 | `GFY_CPUPROFILE` is ignored | It writes a Go pprof profile; Rust has no equivalent format |

## 6. Phases

```
P0 setup -> P1 gositter-rs ----------------+
         -> P1' gostd + bedrock -----------+
                                           v
            P2 graphify extract/resolve/CLI, parity gate vs Go
               ==> tag go-parity-v0.12.0
                                           v
            P3 upstream backlog p0 -> p3, each a ledger divergence
                                           v
            P4 perf gate vs Go -> P5 replace graphify-go
```

- **P0**: both repos, beads, CI from deflate-rs, rulesets, backlog import
  (done 2026-11-12).
- **P1**: gositter-rs to all 1,423 goldens green on amd64 and arm64.
- **P1'** (parallel with P1): `gostd` and `bedrock`. Nothing here needs parse
  trees. `gostd` gets its own Go oracle: a small Go program runs Go std
  (json, strconv, regexp, toml, unicode, url, sort, pcg) on generated inputs,
  and the outputs must equal `gostd`'s. This is the graphify-rs `parity` stage
  until P2 extends it.
- **P2**: graphify extractors, resolve, build and CLI. Extractors start only
  when their grammar's goldens pass. Ends with the `go-parity-v0.12.0` tag.
- **P3**: the upstream backlog. No module gets the mixed rule (port the fixed
  version first): the p0 fixes are local (B01 touches `MakeID` and the
  keep-first rule in addNode), and `resolve` is where the byte oracle is most
  valuable.
- **P4**: cold and no-change builds on the 7 benchmark corpora (curl, graphify,
  k8s, opencv, redis, typescript, vscode) on dobbolab. Gate: graphify-rs is
  faster than graphify-go v0.12.0 on wall time on every corpus, with peak RSS
  no worse.
- **P5**: release workflow for linux and darwin, amd64 and arm64 (darwin on
  GitHub-hosted macOS). Homebrew tap formula, git hooks and `graphify install`
  point at graphify-rs; graphify-go archived.

## 7. Oracles and CI

| Stage | Compares | Gate | Phase |
|---|---|---|---|
| `test` | fmt, clippy `-D warnings` (x86_64 and aarch64), tests, tests with `<CRATE>_SIMD=scalar`, cargo-deny | PR | all |
| `arm64` | tests under qemu (gositter-rs: the unsigned `char` path) | main | all |
| `parity` (gositter-rs) | treedump vs 1,423 C goldens, and vs the pure-Go gositter dump tool | main | P1 |
| `parity` (graphify-rs) | P1': `gostd` vs Go std. P2: the gate in section 5 vs graphify-go 35fcd9e | main | P1' on |
| `graphdiff` | node and edge sets vs Go; every difference must match an allowlist rule with a D-id | main | P3 |
| `upstream` | graphify 0.9.107 (Python) on the `work/` fixtures, compared by label and file | no | P3 |
| `macos` | tests on GitHub-hosted macOS | main | P2 on |
| `fuzz.yml` | nightly cargo-fuzz: gositter-rs parse, the 3 graphify-go fuzz targets, `gostd` json/regexp/toml | no | all |
| `bench.yml` | per crate vs the Go original; gate 1.10x of Go or better | no | all |

The graphify-rs `go-oracle` stage builds graphify-go 35fcd9e from source with
`go mod download` checked against `go.sum` and `-mod=readonly`. graphify-go and
its about 30 module dependencies, gositter v0.2.0 included, are public.

As in deflate-rs, the repository ruleset requires only `test` on a PR. The
"main" stages run on push to main and on `workflow_dispatch`; a bead is done
only when they are green on main.

After P3 starts, the byte diff cannot stay the gate, so `parity/graphdiff`
(Rust) takes over. The `go-parity-v0.12.0` tag keeps the byte proof
reproducible. Node IDs differ from upstream by design (graphify-go GOALS.md:83),
so the upstream oracle matches on label and source file.

## 8. Backlog import (P0, done)

- 74 items from `~/.cache/parity-0107-r4t9/filed.txt` (B01–B72 plus critic
  items x4u.43, x4u.44): 12 p0, 22 p1, 28 p2, 12 p3, under the epic "P3:
  upstream graphify 0.9.107 backlog".
- 115 open graphify-go beads that no B-item covers: vi5, byx, 8as and acy under
  "P3: older graphify-go open beads"; 4ln and 9j5.18 under "P4: performance
  ideas". 9j5.17 (parse time growth) went to gositter-rs. The rest of 9j5
  (cgo removal) and l3o (a Go session task) do not apply.
- Every bead keeps its source ids (`B<nn>`, the graphify-go id as external
  ref, upstream PR numbers) in its description. All of them wait on the
  milestone `go-parity-v0.12.0`.
- The input directory `~/.cache/parity-0107-r4t9/` is read only and kept.

Each P3 fix lands as: the upstream test, ported; the `work/` fixture with its
expected output; the code; a `parity/DIVERGENCES.md` entry; and the allowlist
rule. We match what upstream captures (node and relation kinds) but do not copy
an upstream edge that is provably wrong; such a case gets its own ledger entry.

## 9. YAML extractor (B27)

Owner decision: build it in Rust. A pure-Rust YAML crate is acceptable if one
fits; if profiling shows it is a bottleneck, replace it with our own parser
with SIMD kernels.

Needs: multi-document streams, anchors and aliases, merge keys, local tags,
line numbers per node. Candidates to check in P3 (source read only, CI build):
`saphyr` / `saphyr-parser` and `yaml-rust2`. Rejected: `serde_yaml` (archived)
and `unsafe-libyaml` (machine-translated C, heavy `unsafe`). The chosen crate
must pass the `deny` stage and goes into the README with its reason.

## 10. LLM client (`bedrock` crate)

Same behaviour as graphify-go's semantic path:

- `POST https://bedrock-mantle.us-east-1.api.aws/anthropic/v1/messages`, base URL
  overridable by `ANTHROPIC_BEDROCK_MANTLE_BASE_URL`.
- Model `anthropic.claude-opus-4-8`, non-streaming, one forced tool,
  `max_tokens` 1024.
- Auth: Bearer from `AWS_BEARER_TOKEN_BEDROCK` or `ANTHROPIC_AWS_API_KEY`, else
  SigV4 (service `bedrock-mantle`).
- Credentials: env, then static profile in `~/.aws/credentials` /
  `~/.aws/config`, then `credential_process`, then
  `aws configure export-credentials` as a last resort (covers SSO).
- Retries: 2, backoff 0.5–8 s, `Retry-After` honoured.
- Own HTTP/1.1 client over rustls. SigV4 tested against the 38 aws-c-auth test
  vectors. Tests use a local mock server; no test calls AWS.

## 11. Streams

Long independent streams run as T3 Code sessions titled `graphify-rs: <subject>`;
fan-out inside a stream uses workflows. This thread coordinates: it owns the
plan, the order and the beads, and writes each session's brief.

| Session | Phase |
|---|---|
| graphify-rs: gositter-rs CI and runtime | P1 |
| graphify-rs: grammar tables and scanners | P1 |
| graphify-rs: core libraries (gostd) | P1' |
| graphify-rs: LLM client | P1' |
| graphify-rs: extractors and CLI | P2 |
| graphify-rs: upstream backlog | P3 |

Every brief repeats, word for word: tests first; no build, test, lint, fuzz or
run on Chris's Mac; graphify-go and gositter are read only.

## 12. Size estimate

| Part | Lines (approx.) |
|---|---|
| gositter-rs runtime | 6,000–7,500 |
| gengrammar | 2,500 |
| scanners | 6,500 |
| treedump + regress runner | 500 |
| gostd | 18,000–22,000 (Go sources: regexp 6.6k, toml 4.4k, xml 4.4k, x/text 5.2k, strconv 1.9k) |
| bedrock | 1,500 |
| graphify | 25,000–30,000 |

Tests to port: 119 gositter Go tests plus 1,423 goldens; 604 graphify-go test
functions and 3 fuzz targets (about 660 with subtests); upstream Python tests
at v0.9.107 where Go has the behaviour (953 direct, 4,918 partial).
