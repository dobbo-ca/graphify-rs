# graphify parity gate

graphify-rs against graphify-go v0.12.0 (`35fcd9e79624716b9d9372070055bad85a8ea779`), built in
`ci/Dockerfile` with Go 1.27.4, `GOAMD64=v1`, `CGO_ENABLED=0`. Plan: `docs/superpowers/plans/2026-11-13-p2-graphify.md`
("Parity gate", Task 23). Everything here runs only in CI (the `graphify-parity` job of `ci.yml`, on
schedule and `workflow_dispatch`, 60 min), never on a developer machine.

## Corpora

One Dockerfile stage `corpus-<name>` each; its only step is one `RUN` with the URL and the full SHA,
so its cache key is the SHA.

| Name | Source | Commit | Why |
|---|---|---|---|
| `ggo` | dobbo-ca/graphify-go | `35fcd9e79624716b9d9372070055bad85a8ea779` | Go, markdown, json, shell; the oracle's own tree |
| `ggo-testdata` | the same, `internal/extract/testdata` only | same | 18 language projects, every extractor |
| `gositter` | dobbo-ca/gositter | `7a27b3093a71a93449cf633355edb0a9e0e1add7` (v0.2.0) | very large Go files: the budget and large-first paths |
| `upstream` | Graphify-Labs/graphify, `tests/fixtures` and `worked` | `a8f97f18f086c7fe37afb8109d20d88971eea683` (v0.9.107) | multi-language fixtures |
| `curl` | curl/curl | `75a2079d5c28debb2eaa848ca9430f1fe0d7844c` (curl-8_11_1) | C, a P4 benchmark corpus |
| `redis` | redis/redis | `a0a6f23d997b024689ba157916837f493a593a34` (7.4.2) | C, Tcl, Lua, a P4 benchmark corpus |
| `nonutf8` | made in its stage: 3 files named `b"\xff.py"`, `b"a\xe2\x82.md"`, `b"\xed\xa0\x80.json"` | - | D-008: `build` only; only `graphify-out/graph.json` is compared |

`graphify-corpus` copies them to `/corpus/<name>` and runs `filter.sh`: deletes every committed
`graphify-out`; the D-001/D-002 inputs (MCP configs with an `env` object of 2+ keys, `pyproject.toml`
with `tool.poetry`); every file whose extension belongs to a group not in `groups.txt`
(`groups.map` is the extension to group map from `extract.go`'s dispatch); and extensionless
`#!` files until bash, python, js, ruby, php, lua and julia are all listed. It prints
`/corpus/MANIFEST` (sha256 of every file) in the log.

## Files

- `commands.txt`: `<n> <corpus> <args...> [< <stdin file>]`, run in order. A `# TODO(P2 args)` comment marks a draft value.
- `run.sh <out> [allow-file]`: runs the lines (all, or only those whose first argument is in the allow-file) with `env -i`
  (`PATH`, `HOME=/home/gate`, `TZ=UTC`, `LC_ALL=C`), writes `<out>/t/<n>.{stdout,stderr,exit}`, then copies
  `graphify-out` of each corpus to `<out>/corpus/<name>/`, `/work` (damaged and cross-corpus inputs) to `<out>/work/`,
  and the hook files to `<out>/hook/`. Prints seconds per corpus (`<out>/times.txt`).
- `compare.sh <go-out> <rs-out>`: the diff, see below. `filter.sh`, `groups.txt`, `groups.map`, `stdin/`.

## The switch

The Rust side only has to match what it implements. `rs-commands.txt` lists those subcommands, one per line
(it is empty while `build` is not on main).

- `gate-go` runs every line; `gate-rs` runs `run.sh /out rs-commands.txt`.
- The job always runs `gate-go` and uploads it as the artifact `graphify-gate-go` (transcripts, per-corpus times, MANIFEST).
  It builds and diffs the Rust side only if `rs-commands.txt` has a non-comment line.
- `compare.sh` `cmp`s every transcript the Rust side wrote; compares `graphify-out` (`diff -r` minus `.graphify_cache.json`,
  `.graphify_stat.json`, `.graphify_root`) only when `build` is listed, with the `export` files left out until `export`
  is listed; `nonutf8` is `cmp` of `graph.json` only; `hook/` only when `hook` is listed; each `/work` dir the Rust side made.
- A line that reads `graph.json` needs `build` listed first (`/work/*` inputs come from the built `ggo` and `ggo-testdata`).

Any difference fails the stage. It is a Rust bug unless a D-id in `parity/DIVERGENCES.md` covers it; then change the corpus or
`commands.txt` to avoid it (no output normalisation).

## Enable a subcommand

Implement it so every line of that subcommand in `commands.txt` is byte-equal to Go (stdout, stderr, exit), add its name to
`rs-commands.txt`, run the job by `workflow_dispatch`.

## Enable a group

1. Add its name to `groups.txt` (names: `json nogrammar golang python js java csharp c cpp hcl rust ruby php bash scala kotlin julia lua zig verilog`).
2. Add one `extract <file>` line for it to `commands.txt` (a file of that group in `ggo` or `ggo-testdata`) and re-check the `# TODO(P2 args)`
   labels: the filtered `ggo` graph gains that language's nodes.
3. Run the job and fix every difference in Rust.
