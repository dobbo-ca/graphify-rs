//! Port of `bash.go`. Owner: the gostd stream; filled by graphify-rs-ihc.35.

use std::collections::HashSet;
use std::sync::OnceLock;

use gositter::Node;
use gositter_grammars::bash;

use super::builder::{Builder, line, node_text, parse_root, walk_kinds};
use super::extract::Result;
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Relation};

/// Go `bashFuncKinds` (`bash.go:43`): the mask of `function_definition`.
static BASH_FUNC_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `extractBash`: `None` when the parse budget stops the parse (see `builder`). Function
/// definitions, `source`/`.` includes and call edges out of a shell script.
pub(crate) fn extract_bash(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, bash::language(), |root| {
        let mut b = Builder::new(rel);
        // Collect all function names first so a command that invokes a script is not
        // mistaken for one, and a forward-defined function still shadows a runner.
        let funcs = collect_bash_funcs(root, src);
        bash_items(&mut b, root, src, &funcs);
        b.res
    })
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    BASH_FUNC_KINDS.get_or_init(|| bash::language().kind_mask(&["function_definition"]));
}

/// Go `fieldText`.
fn field_text(n: Node<'_>, field: &str, src: &[u8]) -> String {
    n.child_by_field_name(field)
        .map_or_else(String::new, |c| node_text(c, src).into_owned())
}

/// Go `unquote`: `strings.Trim(s, "`\"'")`.
fn unquote(s: &str) -> &str {
    s.trim_matches(['`', '"', '\''])
}

/// Go `collectBashFuncs`: the function names defined anywhere in the file.
fn collect_bash_funcs(n: Node<'_>, src: &[u8]) -> HashSet<String> {
    let mask = *BASH_FUNC_KINDS
        .get()
        .expect("kinds::register_kinds was not called");
    let mut funcs = HashSet::new();
    walk_kinds(Some(n), mask, |cv| {
        let name = field_text(cv, "name", src);
        if !name.is_empty() {
            funcs.insert(name);
        }
        true
    });
    funcs
}

/// Go `bashItems`, with `bashFunc` folded in and the recursion on tree depth made a stack:
/// walks the statements directly under `n`. A caller is the id of the enclosing function
/// (`""` at file scope). Source/`.` commands always record an import; other commands record a
/// call only inside a function. Control-flow wrappers are descended so nested commands show.
fn bash_items(b: &mut Builder, n: Node<'_>, src: &[u8], funcs: &HashSet<String>) {
    // callers[0] is file scope; each stack entry is a statement with its caller's index.
    let mut callers: Vec<String> = vec![String::new()];
    let mut stack: Vec<(Node<'_>, usize)> = Vec::new();
    let mut buf: Vec<Node<'_>> = Vec::new();
    n.children(&mut buf);
    stack.extend(buf.iter().rev().map(|&c| (c, 0)));
    while let Some((c, caller)) = stack.pop() {
        match c.kind() {
            "function_definition" => {
                let name = field_text(c, "name", src);
                if name.is_empty() {
                    continue;
                }
                let id = b.id(&[&name]);
                b.def(&id, &name, &format!("{name}()"), line(c));
                if let Some(body) = c.child_by_field_name("body") {
                    callers.push(id);
                    let idx = callers.len() - 1;
                    buf.clear();
                    body.children(&mut buf);
                    stack.extend(buf.iter().rev().map(|&k| (k, idx)));
                }
            }
            "command" => bash_command(b, c, src, &callers[caller], funcs),
            _ => {
                buf.clear();
                c.children(&mut buf);
                stack.extend(buf.iter().rev().map(|&k| (k, caller)));
            }
        }
    }
}

/// Go `bashCommand`: `source x` / `. x` records an import of x; a script runner (`bash x.sh`)
/// or a bare `./x.sh` records a script_invocation calls edge to the invoked script's file
/// node; any other command inside a function records a call by the command's leading word.
fn bash_command(
    b: &mut Builder,
    n: Node<'_>,
    src: &[u8],
    caller_id: &str,
    funcs: &HashSet<String>,
) {
    let name = field_text(n, "name", src);
    if name.is_empty() {
        return;
    }
    if name == "source" || name == "." {
        let spec = first_arg(n, src);
        if !spec.is_empty() {
            b.imp(&spec, line(n));
        }
        return;
    }
    // A command whose name shadows a defined function is that function, not a
    // program invoking another script: skip the script-invocation edge (#1756).
    if !funcs.contains(&name) {
        let tgt = script_invocation_target(b, n, &name, src);
        if !tgt.is_empty() {
            // At file scope, the script's own file node is the caller.
            let caller = if caller_id.is_empty() {
                b.file_id.clone()
            } else {
                caller_id.to_string()
            };
            b.res.edges.push(Edge {
                source: caller,
                target: tgt,
                relation: Relation::CALLS,
                confidence: Confidence::EXTRACTED,
                weight: 1.0,
                source_file: b.src,
                source_location: line(n),
                ..Edge::default()
            });
        }
    }
    if !caller_id.is_empty() {
        b.call(caller_id, &name, line(n));
    }
}

/// Go `scriptInvocationTarget`: the file-node id of a script this command runs, or `""` when
/// the command is not a static script invocation (a bare `./x.sh`, or a runner `bash x.sh`
/// with a literal first argument), resolved against the invoking script's directory.
fn script_invocation_target(b: &Builder, n: Node<'_>, cmd_name: &str, src: &[u8]) -> String {
    let raw = if cmd_name.ends_with(".sh") {
        // Bare `./x.sh`: the command name is the script and must be a literal
        // (no $VAR/command-substitution in the name).
        match n.child_by_field_name("name") {
            Some(name_node) if !bash_has_expansion(name_node) => cmd_name.to_string(),
            _ => return String::new(),
        }
    } else if is_script_runner(cmd_name) {
        match first_arg_literal(n, src, cmd_name) {
            Some(lit) => lit,
            None => return String::new(),
        }
    } else {
        return String::new();
    };
    // A bare command must be a .sh; a runner argument may be any file.
    if gostd::path::ext(&raw).is_empty() {
        return String::new();
    }
    let dir = gostd::path::dir(&b.file);
    make_id(&[&gostd::path::join(&[&dir, &raw])])
}

/// Go `bashScriptRunners`: the interpreters whose first non-flag argument names an invoked
/// script.
fn is_script_runner(name: &str) -> bool {
    matches!(
        name,
        "bash"
            | "sh"
            | "zsh"
            | "ksh"
            | "dash"
            | "python"
            | "python3"
            | "node"
            | "ruby"
            | "perl"
            | "php"
            | "deno"
            | "bun"
    )
}

/// Go `firstArg`: the text of a command's first argument, quotes stripped.
fn first_arg(n: Node<'_>, src: &[u8]) -> String {
    match n.child_by_field_name("argument") {
        Some(arg) => unquote(&node_text(arg, src)).to_string(),
        None => String::new(),
    }
}

/// Go `firstArgLiteral`: the first argument's text, only when that argument is a static
/// literal (no shell expansion) and not a code flag; leading `-` flags are skipped.
fn first_arg_literal(n: Node<'_>, src: &[u8], runner: &str) -> Option<String> {
    let mut cur = n.walk();
    for arg in n.children_by_field_name("argument", &mut cur) {
        if bash_has_expansion(arg) {
            return None;
        }
        let txt = node_text(arg, src);
        let txt = unquote(&txt);
        if bash_code_flag(runner, txt) {
            return None;
        }
        if !txt.starts_with('-') {
            return Some(txt.to_string());
        }
    }
    None
}

/// Go `bashCodeFlag`: whether `flag` makes `runner` take a module or code string instead of a
/// file. `-e` is errexit for shells, so it only counts elsewhere.
fn bash_code_flag(runner: &str, flag: &str) -> bool {
    match runner {
        "bash" | "sh" | "zsh" | "ksh" | "dash" => flag == "-c",
        "python" | "python3" => flag == "-m" || flag == "-c",
        "php" => flag == "-r", // php -e is a debugger flag
        _ => flag == "-e",
    }
}

/// Go `bashHasExpansion`: whether `n` contains any shell expansion or substitution.
fn bash_has_expansion(n: Node<'_>) -> bool {
    let mut found = false;
    n.preorder(|c| {
        if found {
            return false;
        }
        found = matches!(
            c.kind(),
            "expansion"
                | "simple_expansion"
                | "command_substitution"
                | "arithmetic_expansion"
                | "process_substitution"
        );
        !found
    });
    found
}

#[cfg(test)]
#[path = "tests/bash.rs"]
mod bash_tests;

#[cfg(test)]
#[path = "tests/bash_scriptinvocation.rs"]
mod bash_scriptinvocation_tests;
