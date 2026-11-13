use std::process::Command;

// Under qemu-user (the arm64 CI stage) a child aarch64 binary cannot be exec'd directly: there is
// no binfmt_misc in BuildKit. GRAPHIFY_TEST_RUNNER names the emulator to run it with.
fn graphify(args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_graphify");
    let mut cmd = match std::env::var("GRAPHIFY_TEST_RUNNER") {
        Ok(runner) if !runner.is_empty() => {
            // Space-separated (`qemu-aarch64 -L /usr/aarch64-linux-gnu`); clippy.toml bans split_whitespace.
            let mut parts = runner.split(' ').filter(|p| !p.is_empty());
            let mut c = Command::new(parts.next().unwrap());
            c.args(parts).arg(bin);
            c
        }
        _ => Command::new(bin),
    };
    cmd.args(args).output().unwrap()
}

#[test]
fn version_matches_go_format() {
    for arg in ["version", "--version", "-v"] {
        let out = graphify(&[arg]);
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8(out.stdout).unwrap(),
            "graphify dev (commit none, built unknown)\n"
        );
    }
}

#[test]
fn unknown_command_exits_1() {
    let out = graphify(&["nope"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "graphify: unknown command \"nope\"\n"
    );
}

#[test]
fn no_args_exits_2() {
    assert_eq!(graphify(&[]).status.code(), Some(2));
}
