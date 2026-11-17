//! Port of `prof.go`: nothing. graphify-go writes a pprof CPU profile to `$GFY_CPUPROFILE` and an
//! allocation profile to `$GFY_MEMPROFILE` during `build` and `update` (`prof.go:12-51`); Rust has
//! no pprof, so both variables are ignored: no file is created and no warning printed
//! (`parity/DIVERGENCES.md` D-005, plan R9). The Go tests become the D-005 checks in
//! `crates/graphify/tests/cli_prof.rs`.
