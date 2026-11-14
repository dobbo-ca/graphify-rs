//! graphify-go `internal/extract/bash_scriptinvocation_test.go`.

use crate::extract::{file, kinds, resolve};
use crate::graph;
use crate::idutil::make_id;
use crate::model::{Edge, Extraction, Relation};
use crate::testutil::TempDir;

/// Go `resolveFiles`: `File` on each of `files` under `root`, then `Resolve` (GOMAXPROCS workers).
fn resolve_files(root: &TempDir, files: &[&str]) -> Extraction {
    kinds::register_kinds();
    let results: Vec<_> = files
        .iter()
        .map(|f| file(root.path(), f).unwrap_or_else(|e| panic!("File({f}): {e}")))
        .collect();
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

/// Go `hasScriptCall`.
fn has_script_call(edges: &[Edge], src_id: &str, tgt_id: &str) -> bool {
    edges
        .iter()
        .any(|e| e.relation == Relation::CALLS && e.source == src_id && e.target == tgt_id)
}

/// Go `writeScript`.
fn write_script(root: &TempDir, name: &str, body: &str) {
    root.write(name, body.as_bytes());
}

// `bash x.sh` and a bare `./x.sh` both emit a script_invocation calls edge from
// the invoking script's file node to the invoked script's file node (#1756).
const INVOCATION_CMDS: [&str; 2] = ["./helpers.sh", "bash ./helpers.sh"];

#[test]
#[ignore = "graphify-rs-ihc.18"] // bash test (ihc.35, bash_scriptinvocation_test.go): needs resolve
fn test_bash_script_invocation() {
    for cmd in INVOCATION_CMDS {
        let root = TempDir::new();
        write_script(&root, "helpers.sh", "#!/bin/bash\necho helper\n");
        write_script(&root, "deploy.sh", &format!("#!/bin/bash\n{cmd}\n"));

        let ext = resolve_files(&root, &["deploy.sh", "helpers.sh"]);
        let from = make_id(&["deploy.sh"]);
        let to = make_id(&["helpers.sh"]);
        assert!(
            has_script_call(&ext.edges, &from, &to),
            "{cmd}: missing script_invocation edge deploy.sh->helpers.sh; edges={:?}",
            ext.edges
        );
        // Survives graph build (shell->shell, not a cross-language phantom).
        let g = graph::build(ext);
        assert!(
            g.has_edge(&from, &to),
            "{cmd}: script_invocation edge did not survive graph build"
        );
    }
}

// The edge is attributed to the enclosing function when the invocation is inside
// one, not to the file node.
#[test]
#[ignore = "graphify-rs-ihc.18"] // bash test (ihc.35, bash_scriptinvocation_test.go): needs resolve
fn test_bash_script_invocation_from_function() {
    let root = TempDir::new();
    write_script(&root, "helpers.sh", "#!/bin/bash\necho helper\n");
    write_script(
        &root,
        "deploy.sh",
        "#!/bin/bash\ndeploy() { bash ./helpers.sh; }\n",
    );

    let ext = resolve_files(&root, &["deploy.sh", "helpers.sh"]);
    let func = make_id(&["deploy.sh", "deploy"]);
    let to = make_id(&["helpers.sh"]);
    assert!(
        has_script_call(&ext.edges, &func, &to),
        "expected edge from deploy() function node; edges={:?}",
        ext.edges
    );
    assert!(
        !has_script_call(&ext.edges, &make_id(&["deploy.sh"]), &to),
        "edge should attribute to the function, not the file node"
    );
}

// A runner name that shadows a function defined in the file is that function, not
// a script invocation: no edge even though the target exists.
#[test]
#[ignore = "graphify-rs-ihc.18"] // bash test (ihc.35, bash_scriptinvocation_test.go): needs resolve
fn test_bash_script_invocation_skips_shadowed() {
    let root = TempDir::new();
    write_script(&root, "helpers.sh", "#!/bin/bash\necho helper\n");
    write_script(
        &root,
        "deploy.sh",
        "#!/bin/bash\nbash() { echo custom; }\nbash ./helpers.sh\n",
    );

    let ext = resolve_files(&root, &["deploy.sh", "helpers.sh"]);
    assert!(
        !has_script_call(
            &ext.edges,
            &make_id(&["deploy.sh"]),
            &make_id(&["helpers.sh"])
        ),
        "shadowed `bash` must not emit a script_invocation edge; edges={:?}",
        ext.edges
    );
}

// A dynamic target (`bash "./$X.sh"`) is not a literal and emits no edge.
#[test]
fn test_bash_script_invocation_skips_dynamic() {
    kinds::register_kinds();
    let root = TempDir::new();
    write_script(&root, "deploy.sh", "#!/bin/bash\nbash \"./$SCRIPT.sh\"\n");

    let r = file(root.path(), "deploy.sh").unwrap_or_else(|e| panic!("File: {e}"));
    for e in &r.edges {
        assert!(
            e.relation != Relation::CALLS,
            "dynamic invocation must emit no calls edge, got {e:?}"
        );
    }
}

// A target that is not in the corpus leaves a dangling edge that graph build
// prunes, so the final graph has no invocation edge to a missing script.
#[test]
#[ignore = "graphify-rs-ihc.18"] // bash test (ihc.35, bash_scriptinvocation_test.go): needs resolve
fn test_bash_script_invocation_prunes_missing() {
    let root = TempDir::new();
    write_script(&root, "deploy.sh", "#!/bin/bash\n./missing.sh\n");

    let ext = resolve_files(&root, &["deploy.sh"]); // missing.sh not collected
    let g = graph::build(ext);
    assert!(
        !g.has_edge(&make_id(&["deploy.sh"]), &make_id(&["missing.sh"])),
        "edge to a missing script must be pruned from the built graph"
    );
}

// Non-shell interpreters link to a literal script argument; dynamic ones don't.
const RUNNER_CASES: [(&str, bool); 5] = [
    ("python3 build.py", true),
    ("node build.js", true),
    ("python3 -u build.py", true),
    (r#""$PY" build.py"#, false),
    (r#"python3 "$X""#, false),
];

#[test]
#[ignore = "graphify-rs-ihc.26"] // bash test (ihc.35, bash_scriptinvocation_test.go): also parses python and js files (js is the last of the two to land); also needs resolve (ihc.18)
fn test_bash_runner_non_shell_targets() {
    for (cmd, want_edge) in RUNNER_CASES {
        let root = TempDir::new();
        write_script(&root, "build.py", "print(1)\n");
        write_script(&root, "build.js", "1\n");
        write_script(&root, "run.sh", &format!("#!/bin/bash\n{cmd}\n"));
        let ext = resolve_files(&root, &["run.sh", "build.py", "build.js"]);
        let got = ["build.py", "build.js"]
            .iter()
            .any(|f| has_script_call(&ext.edges, &make_id(&["run.sh"]), &make_id(&[*f])));
        assert_eq!(got, want_edge, "{cmd:?}: edge={got} want {want_edge}");
    }
}

// `python3 -m app.main` names a module, not a file; it must not hit app_main.
#[test]
#[ignore = "graphify-rs-ihc.25"] // bash test (ihc.35, bash_scriptinvocation_test.go): also parses a python file; also needs resolve (ihc.18)
fn test_bash_runner_module_flag_no_edge() {
    let root = TempDir::new();
    write_script(&root, "app.py", "def main():\n    pass\n");
    write_script(&root, "b1.sh", "#!/bin/bash\npython3 -m app.main\n");
    let ext = resolve_files(&root, &["b1.sh", "app.py"]);
    for e in &ext.edges {
        assert!(
            !(e.source == make_id(&["b1.sh"]) && e.target == make_id(&["app.py", "main"])),
            "module arg resolved to a symbol node: {e:?}"
        );
    }
}

// `-e` is errexit for shells, so `bash -e x.sh` still runs x.sh.
#[test]
#[ignore = "graphify-rs-ihc.18"] // bash test (ihc.35, bash_scriptinvocation_test.go): needs resolve
fn test_bash_shell_errexit_keeps_edge() {
    let root = TempDir::new();
    write_script(&root, "deploy.sh", "#!/bin/bash\n");
    write_script(&root, "ci.sh", "#!/bin/bash\nsh -e ./deploy.sh\n");
    write_script(&root, "ci2.sh", "#!/bin/bash\nbash -e deploy.sh\n");
    let ext = resolve_files(&root, &["deploy.sh", "ci.sh", "ci2.sh"]);
    for f in ["ci.sh", "ci2.sh"] {
        assert!(
            has_script_call(&ext.edges, &make_id(&[f]), &make_id(&["deploy.sh"])),
            "{f}: lost edge to deploy.sh"
        );
    }
}

// php's code flag is -r; `php -e s.php` still runs s.php.
#[test]
#[ignore = "graphify-rs-ihc.34"] // bash test (ihc.35, bash_scriptinvocation_test.go): also parses a php file; also needs resolve (ihc.18)
fn test_bash_php_dash_e_keeps_edge() {
    let root = TempDir::new();
    write_script(&root, "s.php", "<?php\n");
    write_script(&root, "e1.sh", "#!/bin/bash\nphp -e s.php\n");
    let ext = resolve_files(&root, &["s.php", "e1.sh"]);
    assert!(
        has_script_call(&ext.edges, &make_id(&["e1.sh"]), &make_id(&["s.php"])),
        "e1.sh: lost edge to s.php"
    );
}
