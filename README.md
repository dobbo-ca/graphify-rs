# graphify-rs

A pure-Rust port of [graphify-go](https://github.com/dobbo-ca/graphify-go) v0.12.0 (35fcd9e): a code knowledge graph builder. graphify-go is frozen at that commit and is the byte-parity oracle; after parity (tag `go-parity-v0.12.0`) this repo takes the upstream [graphify](https://github.com/Graphify-Labs/graphify) 0.9.107 fixes as recorded divergences (`parity/DIVERGENCES.md`).

Status: P0 (CI skeleton). Design: `docs/superpowers/specs/2026-11-12-graphify-rs-design.md`.

## Crates

| Crate | Contents |
|---|---|
| `gostd` | Go-exact pieces of the Go standard library, x/text and x/exp |
| `bedrock` | Bedrock (mantle) Anthropic Messages client |
| `graphify` | The pipeline and the `graphify` binary |

## No C

No `cc`, `bindgen`, `-sys` crate or FFI to C. `deny.toml` bans `cc`, `cmake`, `bindgen`, `ring`, `aws-lc-*`, `openssl-sys`, `security-framework-sys` and `core-foundation-sys`; the `test` CI job runs `cargo deny check`. `unsafe` is allowed only in SIMD and hot kernels, each with a scalar oracle and a `<CRATE>_SIMD=scalar` override.

## Third-party crates

| Crate | Reason |
|---|---|
| [gositter-rs](https://github.com/dobbo-ca/gositter-rs) | Own port; git dependency pinned by rev (from P2) |
| `rustls`, `rustls-graviola` | TLS for the Bedrock client with no C (from P1') |
| `webpki-roots` | TLS root store for the Bedrock client (from P1'); root certificates under CDLA-Permissive-2.0 (deny.toml exception) |

## Differences from the dobbo-ca/rust-ports conventions

| Rule | Here | Reason |
|---|---|---|
| Repos private | Public | Owner decision, 2026-11-12. |
| Tracked in the rust-ports checklist | Tracked in this repo's beads | Owner decision: rust-ports is for image libraries only. |
| CI on ARC only | ARC, plus GitHub-hosted macOS for darwin test and release jobs (from P2) | ARC is Linux only and darwin binaries ship. |
| Limits / cancel / max_alloc / max_threads | Library crates: yes. CLI: no allocation cap | The CLI reproduces graphify-go, which has neither. |

## Building

CI builds everything: see `ci/Dockerfile` and `.github/workflows/ci.yml` (BuildKit on the ARC runners). A PR needs the `test` job; `arm64` runs on main.

## Issue tracking

This repo uses [beads](https://github.com/steveyegge/beads): `bd ready`, `bd update <id> --claim`, `bd close <id>`, then `bd dolt push`. A fresh clone gets the issues with `bd dolt pull`.

## Licence

MIT. See `LICENSE` (graphify-go's licence, kept).
