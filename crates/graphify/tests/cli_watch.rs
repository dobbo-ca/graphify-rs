//! D-003: `graphify watch` ends on SIGTERM with the default signal exit. graphify-go catches
//! SIGINT and SIGTERM and returns 0 (`maintenance.go:336`); a handler needs FFI to libc.
#![cfg(unix)]

use std::ffi::OsStr;
use std::io::{BufRead, BufReader};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// Under qemu-user (the arm64 CI stage) a child aarch64 binary cannot be exec'd directly (see
// `cli_version.rs`): GRAPHIFY_TEST_RUNNER names the emulator to run it with.
fn graphify(args: &[&OsStr]) -> Command {
    let bin = env!("CARGO_BIN_EXE_graphify");
    let mut cmd = match std::env::var("GRAPHIFY_TEST_RUNNER") {
        Ok(runner) if !runner.is_empty() => {
            // clippy.toml bans split_whitespace.
            let mut parts = runner.split(' ').filter(|p| !p.is_empty());
            let mut c = Command::new(parts.next().unwrap());
            c.args(parts).arg(bin);
            c
        }
        _ => Command::new(bin),
    };
    cmd.args(args);
    cmd
}

/// Kills the child and removes the tree on drop, so a failed assert leaves no `watch` running.
struct Guard {
    child: Child,
    dir: std::path::PathBuf,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
#[ignore = "graphify-rs-ihc.21"]
fn watch_sigterm_d003() {
    let dir = std::env::temp_dir().join(format!("graphify-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // `watch` runs one update first, and an update of a tree with no source file is an error.
    std::fs::write(dir.join("a.md"), "# Title\n\nbody\n").unwrap();

    let child = graphify(&[OsStr::new("watch"), dir.as_os_str()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut g = Guard { child, dir };

    // Ready when it prints its banner (`maintenance.go:333`): the first update is done and the
    // poll loop is next. The reader keeps draining stdout after that so the child never blocks.
    let stdout = g.child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) if line.starts_with("watching for changes") => break,
            Ok(_) => {}
            Err(e) => panic!("watch did not print its banner: {e}"),
        }
    }

    let pid = g.child.id().to_string();
    let sent = Command::new("kill").args(["-TERM", &pid]).status().unwrap();
    assert!(sent.success(), "kill -TERM {pid}: {sent}");

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(s) = g.child.try_wait().unwrap() {
            break s;
        }
        assert!(
            Instant::now() < deadline,
            "watch still running 10s after SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        status.signal(),
        Some(15),
        "exit status {status}, want death by SIGTERM (D-003)"
    );
}
