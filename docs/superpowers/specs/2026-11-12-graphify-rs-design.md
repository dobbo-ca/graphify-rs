# graphify-rs and gositter-rs: design

Date: 2026-11-12. Owner: Chris. Status: draft for owner review.

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
  `.graphify_stat.json`). They are compared by behaviour, not bytes.

## 2. Rules that apply (from dobbo-ca/rust-ports conventions)

- Edition 2024, toolchain 1.99.1 pinned in `rust-toolchain.toml`, `Cargo.lock`
  committed, `publish = false`.
- std first. Never `cc`, `bindgen`, `-sys` crates or FFI to C. A cargo-deny ban
  list enforces this: `cc`, `cmake`, `bindgen`, `ring`, `aws-lc-*`,
  `openssl-sys`, `security-framework-sys`, `core-foundation-sys`.
- Every third-party crate is in the README with its reason.
- `unsafe` only in SIMD and hot kernels, each with a scalar oracle, AVX2 and
  NEON tiers, a `<CRATE>_SIMD=scalar` override and cross-tier parity tests.
- No build, test, lint, fuzz or run on Chris's Mac. Edit, `cargo fmt`,
  `cargo generate-lockfile` and git only. CI builds on ARC runners with
  in-cluster BuildKit (copied from deflate-rs `ci/` and workflows).
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
| Limits / cancel / max_alloc / max_threads | Library crates: yes. CLI: worker count from `available_parallelism`, no allocation cap | The CLI reproduces graphify-go behaviour, which has neither. |
| Ctrl-C in `watch` | Default SIGINT exit, not Go's clean exit 0 | A signal handler needs FFI to libc. Documented as divergence D-003. |

## 3. Repos and layout

```
gositter-rs/                       graphify-rs/
  src/          runtime              crates/gostd/     Go-semantics std pieces
  gengrammar/   parser.c -> tables   crates/bedrock/   LLM client (HTTP, TLS, SigV4)
  grammars/<g>/ tables + scanner.rs  crates/graphify/  lib + `graphify` binary
  treedump/     dump format          parity/           oracle, graph diff, ledger
  testdata/regress/ 1,423 goldens    testdata/         fixtures, goldens
```

graphify-rs depends on gositter-rs as a git dependency pinned by `rev`. Both
repos are public, so CI needs no token to fetch it.

### gositter-rs

- **Runtime** (about 6–7.5k lines): parser, subtree, stack, node, language,
  lexer, tree cursor, slab pool, `parse_budget`. Same C-exact contract as
  gositter's CONTRACT.md: same trees, same LOG lines, same stall count.
- **gengrammar** (about 2–2.5k lines): reads each grammar's `parser.c` (C
  subset), checks its sha256 against `grammars.lock`, and writes the tables.
  - Parse tables are `include_bytes!` blobs with a small typed view. verilog's
    dense table is 16.6 MB, so source literals are not an option.
  - The lexer form (generated Rust `match` code or a data-driven state table)
    is chosen by a CI benchmark spike on bash, cpp and verilog before the other
    grammars are generated.
- **Scanners**: 15 hand ports of gositter's `scanner.go` (about 6.5k lines).
  Serialize bytes must stay byte-identical. C `char` signedness follows
  gositter's `cchar` (unsigned on linux arm64, signed elsewhere).
- **Oracle**: the shared treedump format. The 1,423 C-made `.dump.gz` goldens
  in `testdata/regress` are the primary gate. A CI stage also builds gositter
  (Go) and diffs its dumps against ours on the same inputs.

### graphify-rs crates

| Crate | Contents | Go source |
|---|---|---|
| `gostd` | Go-exact pieces: Unicode 15 + 17 tables (fold, NFKC/NFKD, categories), `regexp` (RE2 port), `encoding/json` encode and decode, TOML, XML, CSV writer, `strconv` floats, pdqsort, `filepath`, x/exp PCG, SHA-256 | Go std, x/text, x/exp, BurntSushi/toml |
| `bedrock` | HTTP/1.1 client, SigV4, credential chain, Messages call | anthropic-sdk-go bedrock path |
| `graphify` | detect, extract, resolve, build, cluster, analyze, export, report, query, cache, serve, CLI | `internal/*`, `cmd/graphify` |

`graphify` mirrors graphify-go's package layout one module per package, so a
reviewer can read the two side by side.

## 4. Dependency map

| Go dependency | Rust choice |
|---|---|
| gositter v0.2.0 | gositter-rs (own) |
| x/text cases, norm (Unicode 15) | `gostd::unicode` tables generated from UCD 15.0.0 |
| Go std `unicode` (Unicode 17) | `gostd::unicode` tables generated from UCD 17.0.0 |
| `regexp` (46 sites + user pattern in `query`) | `gostd::regexp`, a port of Go `regexp` and `regexp/syntax` (about 5–6k lines) |
| `encoding/json` | `gostd::json` (Go escaping, float format, `MarshalIndent`) |
| BurntSushi/toml | `gostd::toml` (dynamic value tree, TOML 1.0) |
| `encoding/xml` | `gostd::xml` (pom.xml decode, GraphML escaping) |
| `encoding/csv` | `gostd::csv` (writer only) |
| `sort.Slice` | `gostd::sort`, a port of Go pdqsort (5 tie-prone sites) |
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

The golden reference is graphify-go 35fcd9e built for linux/amd64. These Go
behaviours do not come for free in Rust, and each has a test in `gostd` or an
explicit decision:

| Hazard | Handling |
|---|---|
| `sort.Slice` ties at query.go:159, :215, affected.go:140, okf.go:64, :118, :219 | pdqsort port, same comparator call order |
| Two Unicode versions (17 in std, 15 in x/text) | both table sets, chosen per call site |
| Node ID: casefold + NFKC to a fixpoint (max 6 loops); `normLabel` NFKD, drop Mn, ToLower | ported as-is in phase 2 |
| JSON: `<` style escapes, U+2028/9, invalid UTF-8, floats, `MarshalIndent` | `gostd::json` with Go's test vectors |
| RE2 semantics: leftmost-first, `\p{L}` | regexp port with Go's regexp tests |
| `strconv.Quote` in hook scripts | `gostd::strconv` |
| FMA on arm64 in ask.go (6 sites) | goldens are amd64, where Go does not fuse; Rust never fuses. `ask` scores on arm64 can differ from Go arm64 in the last bit (ledger D-004) |
| Go map order: mcpconfig.go:113 env, manifest.go:279 Poetry deps | Rust sorts the keys. Ledger entries D-001, D-002; the oracle normalises these two outputs |

The parity gate in phase 2: run both binaries on a fixed corpus in BuildKit,
then `diff -r` of `graphify-out/` must be empty, except the D-001/D-002 files
compared after normalisation.

## 6. Phases

```
P0 setup -> P1 gositter-rs ----------------+
         -> P1' gostd + bedrock -----------+
                                           v
            P2 graphify extract/resolve/CLI, diff -r vs Go
               ==> tag go-parity-v0.12.0
                                           v
            P3 upstream backlog p0 -> p3, each a ledger divergence
                                           v
            P4 perf gate vs Go -> P5 replace graphify-go
```

- **P0**: both repos, beads, CI from deflate-rs, ruleset with required checks
  `test` and `parity`, backlog import.
- **P1**: gositter-rs to all 1,423 goldens green on amd64 and arm64.
- **P1'** (parallel with P1): `gostd` and `bedrock`. Nothing here needs parse
  trees.
- **P2**: graphify extractors, resolve, build and CLI. Extractors start only
  when their grammar's goldens pass. Ends with the `go-parity-v0.12.0` tag.
- **P3**: the upstream backlog. No module gets the mixed rule (port the fixed
  version first): the p0 fixes are local (B01 is one function, `MakeID`), and
  `resolve` is where the byte oracle is most valuable.
- **P4**: cold and no-change builds on the 7 benchmark corpora (curl, graphify,
  k8s, opencv, redis, typescript, vscode) on dobbolab. Gate: graphify-rs is
  faster than graphify-go v0.12.0 on wall time on every corpus, with peak RSS
  no worse.
- **P5**: Homebrew tap formula, git hooks and `graphify install` point at
  graphify-rs; graphify-go archived.

## 7. Oracles and CI

| Stage | Compares | Phase |
|---|---|---|
| `test` | cargo fmt, clippy `-D warnings`, cargo test on amd64 | all |
| `arm64` | tests under qemu (gositter-rs: cchar unsigned path) | all |
| `parity` (gositter-rs) | treedump vs 1,423 C goldens, and vs gositter Go | P1 |
| `parity` (graphify-rs) | `diff -r` of graphify-out vs graphify-go 35fcd9e | P2 |
| `graphdiff` | node and edge sets vs Go; every difference must match an allowlist rule with a D-id | P3 |
| `upstream` | graphify 0.9.107 (Python) on the `work/` fixtures, compared by label and file | P3 |

After P3 starts, `diff -r` cannot stay the gate, so `parity/graphdiff` (Rust)
takes over. The `go-parity-v0.12.0` tag keeps the byte proof reproducible.
Node IDs differ from upstream by design (graphify-go GOALS.md:83), so the
upstream oracle matches on label and source file.

## 8. Backlog import (P0)

- 74 items from `~/.cache/parity-0107-r4t9/filed.txt` (B01–B72 plus critic
  items x4u.43, x4u.44): 12 p0, 22 p1, 28 p2, 12 p3.
- The open graphify-go beads under epics vi5, byx and 4ln. 9j5 (remove cgo) is
  closed as not applicable. 4ln perf items become P4 bench ideas.
- Every bead keeps its source ids (`B<nn>`, `graphify-go-<id>`, upstream PR
  numbers) in its description.
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
must pass the cargo-deny C ban and goes into the README with its reason.

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
  vectors.

## 11. Streams

Long independent streams run as T3 Code sessions titled `graphify-rs: <subject>`;
fan-out inside a stream uses workflows. This thread coordinates: it owns the
plan, the order and the beads, and writes each session's brief.

| Session | Phase |
|---|---|
| graphify-rs: tree-sitter runtime port | P1 |
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
| gengrammar | 2,000–2,500 |
| scanners | 6,500 |
| treedump + regress runner | 500 |
| gostd | 12,000 |
| bedrock | 1,500 |
| graphify | 25,000–30,000 |

Tests to port: 119 gositter Go tests plus 1,423 goldens; 604 graphify-go test
functions and 3 fuzz targets (about 660 with subtests); upstream Python tests
at v0.9.107 where Go has the behaviour (953 direct, 4,918 partial).
