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
  same names in `ci/Dockerfile`. Each also reruns with `GOSTD_SIMD=scalar`. `PARITY_MIN_OUTPUTS`
  proves the ops ran; every module PR raises it.

### Large Go test inputs

The `testinputs` stage builds `/gotest`, which the `test` and `arm64` stages bind-mount with
`GOSTD_GOTEST=/gotest`. A test that needs it panics if the variable is not set; it never skips.

| Path | Source |
|---|---|
| `/gotest/regexp/re2-exhaustive.txt` | `go1.27.4` `src/regexp/testdata/re2-exhaustive.txt.bz2`, decompressed |
| `/gotest/toml-test/` | `github.com/BurntSushi/toml@v1.4.0` `internal/toml-test/tests` (checked by `go.sum`) |

### Count table

Source tests are Go `func Test*` (and, where noted, toml-test cases). "Not ported" names what is not
reached by graphify-go.

| Module | Source | Source tests | Ported | Passing | Not ported (reason) |
|---|---|---|---|---|---|
| oracle harness | `parity/go/oracle` (`echo` op) | - | - | - | - |
