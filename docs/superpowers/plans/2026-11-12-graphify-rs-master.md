# graphify-rs master plan (P0 and stream launch)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish P0 (CI skeletons, rulesets, phase beads) for gositter-rs and graphify-rs, then start the four P1/P1' streams, each of which writes its own detailed TDD plan.

**Architecture:** The design has six subsystems. This plan does not plan their code; it gives each stream its scope, its first beads and its brief. Each stream session writes `docs/superpowers/plans/<date>-<stream>.md` in its repo as its first task and gets it reviewed by this coordinator thread before code.

**Tech Stack:** Rust 1.99.1, edition 2024; BuildKit on ARC (`github-arc-runner-set`); beads (`bd`) with Dolt remotes; Go 1.27 for oracles only.

**Spec:** `docs/superpowers/specs/2026-11-12-graphify-rs-design.md` (graphify-rs repo).

## Global Constraints

- Toolchain `1.99.1` in `rust-toolchain.toml` with components `rustfmt`, `clippy`; edition `2024`; `rust-version = "1.99"`; `publish = false`; `Cargo.lock` committed.
- Never `cc`, `bindgen`, `-sys` crates or FFI to C. cargo-deny bans: `cc`, `cmake`, `bindgen`, `ring`, `aws-lc-sys`, `aws-lc-rs`, `openssl-sys`, `security-framework-sys`, `core-foundation-sys`.
- Allowed third-party crates before P3: `rustls` (no default provider), `rustls-graviola` 0.4, `webpki-roots`. Each listed in the README with its reason.
- No build, test, lint, fuzz or run on Chris's Mac. Edit, `cargo fmt`, `cargo generate-lockfile` and git only; push a branch and read CI.
- Do not change graphify-go or gositter (Go). Read them only.
- Tests first: port and land the tests before the code (failing, or `#[ignore = "<bead id>"]`). A PR that adds behaviour without its tests is not done.
- Branch `port-<area>-<4 hex>`; PR into main; squash-merge when `test` is green; check `main` stages after merge.
- Session titles start with `graphify-rs: `.
- Oracles: graphify-go 35fcd9e, gositter 7a27b30 (tag v0.2.0), upstream graphify 0.9.107.

## Review Focus

- A CI job that passes with no tests (empty crate): each skeleton has one real test so `cargo test` proves the job runs code.
- cargo-deny passes because the ban list never loads: the `deny` step must fail on a PR that adds `cc` (checked once in Task 1, then reverted).
- A stream that starts code before its plan is reviewed: the brief makes "plan PR reviewed by coordinator" its first gate.
- Two sessions in one checkout: every session uses a worktree (`workspaceStrategy`), never the primary checkout.
- Bead drift: each stream claims beads with `bd update --claim` and runs `bd dolt push` after every change.

---

### Task 1: gositter-rs CI skeleton

**Files (repo gositter-rs, branch `port-ci-<4hex>` from main):**
- Create: `Cargo.toml` (package `gositter` 0.1.0, no dependencies, `[lints.rust] unsafe_code = "deny"` at crate level until a kernel needs it)
- Create: `rust-toolchain.toml`, `.gitignore` (`/target`), `LICENSE` and `LICENSE-tree-sitter` (copied from gositter), `README.md` (purpose, the differences table from spec section 2, third-party crates: none)
- Create: `src/lib.rs` (crate doc only, `pub const TREE_SITTER_LANGUAGE_VERSION: u32 = 15;` copied from gositter `api.go:27` `LanguageVersion`)
- Create: `tests/version.rs`
- Create: `deny.toml` (bans from Global Constraints; licences MIT, Apache-2.0, ISC, BSD-3-Clause, Unicode-3.0)
- Create: `ci/Dockerfile` from deflate-rs: stages `toolchain`, `src`, `ci`, `qemu-tools`, `qemu-arm64`, `arm64`. `toolchain` also runs `cargo install cargo-deny --locked --version 0.18.3`. `ci` adds `cargo deny check` after clippy. Drop deflate's `DEFLATE_SIMD`, edge, go-run, parity, fuzz and bench stages; the runtime stream adds them back.
- Create: `.github/workflows/ci.yml` from deflate-rs: jobs `test` and `arm64`, cache scopes `gositter-rs-*`.

- [ ] **Step 1: Write the test** — `tests/version.rs`: `assert_eq!(gositter::TREE_SITTER_LANGUAGE_VERSION, 15);`
- [ ] **Step 2: Write the files above; run `cargo generate-lockfile` and `cargo fmt`.**
- [ ] **Step 3: Commit, push, open PR.** Expected: `test` green.
- [ ] **Step 4: Prove the ban list.** Push a commit adding `cc = "1"` to `[build-dependencies]`. Expected: `test` red at `cargo deny check` with `cc` named. Revert it.
- [ ] **Step 5: Create ruleset** "main: required checks" (same JSON as deflate-rs ruleset 26907959, merge methods `squash` only, required check `test`). Squash-merge the PR. Expected: `arm64` green on main.

### Task 2: graphify-rs CI skeleton

**Files (repo graphify-rs, branch `port-ci-<4hex>` from main):**
- Create: `Cargo.toml` (workspace, members `crates/gostd`, `crates/bedrock`, `crates/graphify`; `[workspace.package]` edition, rust-version, license MIT, publish false; `[workspace.lints.rust] unsafe_code = "deny"`)
- Create: `crates/gostd/src/lib.rs`, `crates/bedrock/src/lib.rs`, `crates/graphify/src/lib.rs`, `crates/graphify/src/main.rs`
- Create: `crates/graphify/tests/cli_version.rs`
- Create: `rust-toolchain.toml`, `.gitignore`, `deny.toml` (as Task 1), `LICENSE` (graphify-go's MIT licence, its copyright line kept), `README.md` (purpose, differences table, third-party crates table, beads usage)
- Create: `ci/Dockerfile`, `.github/workflows/ci.yml` (as Task 1, scopes `graphify-rs-*`, `--workspace` on clippy and test)
- Create: `parity/DIVERGENCES.md` with D-001 to D-005 from spec section 5.

- [ ] **Step 1: Write the test** — `cli_version.rs` runs `env!("CARGO_BIN_EXE_graphify")` with `version`; asserts exit 0 and stdout starts with `graphify ` (Go format `graphify %s (commit %s, built %s)`, main.go version command).
- [ ] **Step 2: Implement `main` with only the `version` command** (Go defaults from main.go:154-156: version `dev`, commit `none`, date `unknown`). Every other command prints `graphify: unknown command "<x>"` (Go `%q`, main.go:214) to stderr and exits 1.
- [ ] **Step 3: lockfile, fmt, commit, push, PR.** Expected: `test` green.
- [ ] **Step 4: Ruleset as Task 1 Step 5.** Merge the design PR (`design-graphify-rs-7c41`) and both `beads-metadata` PRs the same way.

### Task 3: Phase beads

Create in graphify-rs (`bd create`, each epic's description names its spec sections):
- Epic `P1': gostd` (spec 3, 4, 5) and epic `P1': bedrock` (spec 10).
- Epic `P2: extractors, resolve, CLI to Go parity` (spec 5, 6); the milestone `graphify-rs-63i` depends on it; it depends on both P1' epics and on the gositter-rs P1 milestone (cross-repo: recorded as an external ref `gositter-rs:<id>` in the description).

Create in gositter-rs:
- Epic `P1: runtime` and epic `P1: grammars (gengrammar, tables, scanners)`; milestone `P1: all 1,423 goldens green on amd64 and arm64` depends on both.

- [ ] **Step 1: Create them; `bd dep add` as above; `bd dolt push` in both repos.**
- [ ] **Step 2: Verify** `bd ready` in graphify-rs shows the two P1' epics and nothing from P3/P4.

### Task 4: Start the streams

One T3 session each, worktree workspace, title as below. Each gets the brief template plus its scope.

| Title | Repo | Scope | First beads |
|---|---|---|---|
| `graphify-rs: gositter-rs CI and runtime` | gositter-rs | runtime, treedump, regress runner, `parity` and `go-oracle` stages, fuzz/bench workflows | P1: runtime |
| `graphify-rs: grammar tables and scanners` | gositter-rs | gengrammar, `gen` stage, lexer-form spike, 21 grammars, 15 scanners | P1: grammars |
| `graphify-rs: core libraries (gostd)` | graphify-rs | `crates/gostd`, its Go oracle `parity` stage | P1': gostd |
| `graphify-rs: LLM client` | graphify-rs | `crates/bedrock` | P1': bedrock |

The grammar session starts with gengrammar and the scanners; it waits on the runtime session's `Language` type before it wires tables in. The two gositter-rs sessions agree that interface in the runtime session's plan PR.

P2 (`graphify-rs: extractors and CLI`) and P3 (`graphify-rs: upstream backlog`) start when their epics become ready.

**Brief template (copied into every session prompt, filled in):**

```
You are the "<title>" stream of the graphify-rs port. The coordinator is the
thread that launched you; report to it.

Read first: <repo>/CLAUDE.md, the spec docs/superpowers/specs/2026-11-12-graphify-rs-design.md
(graphify-rs repo, main), this plan's Global Constraints, and your beads (bd ready).

Scope: <scope>. Out of scope: everything else; file a bead for what you find.

Rules (word for word, no exceptions):
- No build, test, lint, fuzz or run on Chris's Mac. Edit, cargo fmt, cargo
  generate-lockfile and git only. Push a branch and read CI.
- Do not change graphify-go or gositter (Go). Read them only.
- Tests first: port and land the tests before the code. Sources: graphify-go
  tests, gositter tests and oracle, upstream Python tests (~/.cache/parity-0107-r4t9/up,
  tag v0.9.107) for behaviour Go also has. Keep the count table in parity/README.md.
- Never cc, bindgen, -sys crates or FFI to C. unsafe only in SIMD/hot kernels
  with scalar oracle, x86_64 + aarch64 tiers, <CRATE>_SIMD=scalar, parity tests.
- Branch port-<area>-<4 hex>; PR into main; squash-merge on green test.
- Use workflows for fan-out; agent tiers scout/implement for volume,
  verify/research only where a wrong answer is expensive. Put these rules in
  every workflow prompt.

First task: write docs/superpowers/plans/<date>-<area>.md (writing-plans skill),
open it as a PR, and wait for the coordinator's review before any code.
```

- [ ] **Step 1: Launch the four sessions** (`t3_thread_launch`, worktree strategy, base `origin/main`).
- [ ] **Step 2: Review each plan PR** against the spec before the session writes code.
