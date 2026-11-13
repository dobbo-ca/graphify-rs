use std::process::Command;

fn graphify(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_graphify"))
        .args(args)
        .output()
        .unwrap()
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
