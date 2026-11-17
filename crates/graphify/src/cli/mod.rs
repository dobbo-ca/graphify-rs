//! Port of graphify-go `cmd/graphify`: the command line.
//!
//! Shared API (plan Task 1): `Error`, `Out`, `run` and the `cmd_*` signatures are fixed. The
//! `hook`, `watch`, `install` and `save-result` commands are graphify-rs-ihc.21; the rest is
//! graphify-rs-ihc.20 (plan Tasks 21 and 20).

/// Go `fmt.Fprintf(w, ...)` to stdout or stderr, as a `Result` for `?`: see [`put`].
macro_rules! w {
    ($w:expr, $($arg:tt)*) => {
        $crate::cli::put(&mut *$w, format_args!($($arg)*))
    };
}

mod githead;
// graphify-rs-ihc.21: stubs until its code PR
#[allow(dead_code, unused_variables)]
pub mod install;
pub mod main;
// graphify-rs-ihc.21: stubs until its code PR
#[allow(dead_code, unused_variables)]
pub mod maintenance;
mod outfp;
mod prof;
// graphify-rs-ihc.21: stubs until its code PR
#[allow(dead_code, unused_variables)]
pub mod save_result;
mod semantic;
mod sigpipe;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

/// Where a command writes: stdout or stderr, or a buffer in tests.
pub type Out<'a> = &'a mut dyn Write;

/// A command error. `Display` is Go's error text, printed as `graphify: <err>` (`main.go:216-222`).
#[derive(Debug)]
pub enum Error {
    /// Any Go error, as its text.
    Msg(String),
    /// A write hit a closed pipe (Go `isBrokenPipe`): the process exits 0.
    BrokenPipe,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Msg(s) => f.write_str(s),
            Error::BrokenPipe => f.write_str("write /dev/stdout: broken pipe"),
        }
    }
}

impl std::error::Error for Error {}

/// Any Go error value as a command error, its text kept.
pub(crate) fn msg(e: impl fmt::Display) -> Error {
    Error::Msg(e.to_string())
}

/// Go's `*os.PathError` text, `<op> <path>: <errno text>` (for example
/// `open x.json: no such file or directory`), for an I/O error; `BrokenPipe` for EPIPE.
pub fn path_error(op: &str, path: &Path, e: &io::Error) -> Error {
    if sigpipe::is_broken_pipe(e) {
        return Error::BrokenPipe;
    }
    msg(crate::goerr::path_error(op, path, e))
}

/// Go `fmt.Fprintf` to stdout or stderr. A closed pipe is [`Error::BrokenPipe`] (Go's SIGPIPE
/// handler exits 0, `sigpipe_unix.go:16-23`); Go ignores any other write error, and so does this.
pub(crate) fn put(w: &mut dyn Write, args: fmt::Arguments<'_>) -> Result<(), Error> {
    match w.write_fmt(args) {
        Err(e) if sigpipe::is_broken_pipe(&e) => Err(Error::BrokenPipe),
        _ => Ok(()),
    }
}

/// The binary's entry (`src/main.rs` calls it): [`run`] over `std::env::args_os`, stdout and
/// stderr, on a thread with [`crate::STACK_BYTES`] of stack (plan hazard "Stack depth").
pub fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let cmd = std::thread::Builder::new()
        .stack_size(crate::STACK_BYTES)
        .spawn(move || run(&args, &mut io::stdout(), &mut io::stderr()))
        .unwrap_or_else(|e| panic!("start the command thread: {e}"));
    let code = cmd.join().unwrap_or_else(|p| std::panic::resume_unwind(p));
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Go `main`: runs the command in `args` (`args[0]` is the program) and returns the exit code
/// (`main.go:159-223`: 2 for usage, 1 for an error, 0 otherwise and on a broken pipe).
pub fn run(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> i32 {
    let code = dispatch(args, stdout, stderr);
    match stdout.flush() {
        Err(e) if sigpipe::is_broken_pipe(&e) => 0,
        _ => code,
    }
}

/// The body of Go `main` (`main.go:160-222`). SIGPIPE needs no handler: Rust starts with it
/// ignored, so a closed pipe is an EPIPE write error, which exits 0 here.
fn dispatch(args: &[OsString], stdout: Out<'_>, stderr: Out<'_>) -> i32 {
    let Some(cmd) = args.get(1) else {
        return match usage(stdout) {
            Err(_) => 0,
            Ok(()) => 2,
        };
    };
    let rest = &args[2..];
    let result = match cmd.as_encoded_bytes() {
        b"version" | b"--version" | b"-v" => w!(
            stdout,
            "graphify {} (commit {}, built {})\n",
            main::VERSION,
            main::COMMIT,
            main::DATE
        ),
        // Go `startProf` around build and update: D-005 (`prof.rs`).
        b"build" => main::cmd_build(rest, stdout, stderr),
        b"update" => main::cmd_update(rest, stdout, stderr),
        b"watch" => maintenance::cmd_watch(rest, stdout, stderr),
        b"hook" => maintenance::cmd_hook(rest, stdout, stderr),
        b"install" => install::cmd_install(rest, stdout, stderr),
        b"query" if rest.is_empty() => return must_arg(stderr, "query <pattern>"),
        b"query" => main::cmd_query(rest, stdout, stderr),
        b"ask" => main::cmd_ask(rest, stdout, stderr),
        b"explain" => main::cmd_explain(rest, stdout, stderr),
        b"path" => main::cmd_path(rest, stdout, stderr),
        b"god-nodes" => main::cmd_god_nodes(rest, stdout, stderr),
        b"extract" if rest.is_empty() => return must_arg(stderr, "extract <file>"),
        b"extract" => main::cmd_extract(rest, stdout, stderr),
        b"export" if rest.is_empty() => {
            return must_arg(stderr, "export <graphml|dot|csv|okf> [path]");
        }
        b"export" => main::cmd_export(rest, stdout, stderr),
        b"affected" => main::cmd_affected(rest, stdout, stderr),
        b"diff" => main::cmd_diff(rest, stdout, stderr),
        b"merge-driver" => main::cmd_merge_driver(rest, stdout, stderr),
        b"validate" => main::cmd_validate(rest, stdout, stderr),
        // Go `cmdServe` (`serve.go`): plan Task 22.
        b"serve" => todo!("graphify-rs-ihc.22"),
        b"save-result" => save_result::cmd_save_result(rest, stdout, stderr),
        b"-h" | b"--help" | b"help" => usage(stdout),
        other => Err(Error::Msg(format!(
            "unknown command {}",
            gostd::strconv::quote(other)
        ))),
    };
    match result {
        Ok(()) | Err(Error::BrokenPipe) => 0,
        Err(e) => match w!(stderr, "graphify: {e}\n") {
            Err(_) => 0,
            Ok(()) => 1,
        },
    }
}

/// Go `mustArg` with the argument missing: the usage line on stderr, exit 2.
fn must_arg(stderr: Out<'_>, usage: &str) -> i32 {
    match w!(stderr, "usage: graphify {usage}\n") {
        Err(_) => 0,
        Ok(()) => 2,
    }
}

/// Go `usage`.
fn usage(stdout: Out<'_>) -> Result<(), Error> {
    w!(
        stdout,
        "{}\n",
        r#"graphify - code knowledge graph

usage:
  graphify build [path] [--cargo] [--force] [--no-cluster]   build graph.json + report under <path>/graphify-out (--cargo adds Rust crate-dependency edges; --force overwrites even if the rebuild has fewer nodes, also GRAPHIFY_FORCE=1; --no-cluster skips community detection)
  graphify update [path] [--cargo] [--force] [--no-cluster]  rebuild incrementally, re-parsing only changed files (--force ignores the cache and re-parses everything, also GRAPHIFY_FORCE=1)
  graphify watch [path]        rebuild incrementally as files change (Ctrl-C to stop)
  graphify hook <install|uninstall|status> [path]  manage git hooks that update the graph after commits
  graphify install [--uninstall]  copy the graphify skill into ~/.claude/skills (Claude Code)
  graphify query <pattern>     find nodes by name (regex, case-insensitive)
  graphify ask "<question>"    NL retrieval: relevant subgraph as text [--dfs --budget N --context REL --graph path]
  graphify explain <node>      show a node and its neighbours [--graph path]
  graphify path <from> <to>    shortest dependency path between two nodes [--undirected --graph path]
  graphify god-nodes           most-connected nodes [--top N --graph path --json]
  graphify affected [file...|-] nodes defined in changed files + their dependents [--depth N --relation R]
  graphify diff <old> <new>    node/edge delta between two graph.json snapshots
  graphify merge-driver <base> <current> <other>  git merge driver: union-merge two graph.json files
  graphify validate            check graph.json for structural problems
  graphify serve [graph.json]  MCP stdio server: load graph.json once, answer many queries
  graphify extract <file>      print one file's extracted nodes/edges (debug)
  graphify export <fmt> [path] convert graph.json to graphml, dot, csv, or okf
  graphify save-result --question Q --answer A  file a Q&A result into graphify-out/memory/ so the next update graphs it [--type T --nodes N... --outcome useful|dead_end|corrected --correction TEXT]
  graphify version             print version"#
    )
}
