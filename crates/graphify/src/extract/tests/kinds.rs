//! The plan's Task 15 added tests for hazard "KindMask registration order": a gositter tree
//! carries mask bits only for kinds named before it was parsed, and bits go in first-registration
//! order, so every kind is named once at startup, in Go's order, and nowhere else. Both tests read
//! the extract sources; while no extractor is ported they hold trivially (every
//! `register_kinds` is empty). A grammar PR that adds a file to `extract/mod.rs` adds it to
//! `SOURCES`; one that adds kind names keeps them inside its `register_kinds`.

use std::collections::BTreeSet;

use gostd::strings::trim_space;

/// Every non-test source file of `src/extract`, by module name.
const SOURCES: &[(&str, &str)] = &[
    ("bash", include_str!("../bash.rs")),
    ("builder", include_str!("../builder.rs")),
    ("c", include_str!("../c.rs")),
    ("component", include_str!("../component.rs")),
    ("cpp", include_str!("../cpp.rs")),
    ("csharp", include_str!("../csharp.rs")),
    ("csharp_resolve", include_str!("../csharp_resolve.rs")),
    ("dispatch", include_str!("../dispatch.rs")),
    ("extract", include_str!("../extract.rs")),
    ("golang", include_str!("../golang.rs")),
    ("java", include_str!("../java.rs")),
    ("javascript", include_str!("../javascript.rs")),
    ("jsmodules", include_str!("../jsmodules.rs")),
    ("json", include_str!("../json.rs")),
    ("julia", include_str!("../julia.rs")),
    ("kinds", include_str!("../kinds.rs")),
    ("kotlin", include_str!("../kotlin.rs")),
    ("lua", include_str!("../lua.rs")),
    ("markdown", include_str!("../markdown.rs")),
    ("mcpconfig", include_str!("../mcpconfig.rs")),
    ("mod", include_str!("../mod.rs")),
    ("nulllabel", include_str!("../nulllabel.rs")),
    ("nulllabel_resolve", include_str!("../nulllabel_resolve.rs")),
    ("php", include_str!("../php.rs")),
    ("python", include_str!("../python.rs")),
    ("resolve", include_str!("../resolve.rs")),
    ("resultjson", include_str!("../resultjson.rs")),
    ("ruby", include_str!("../ruby.rs")),
    ("rust", include_str!("../rust.rs")),
    ("scala", include_str!("../scala.rs")),
    ("terraform", include_str!("../terraform.rs")),
    ("verilog", include_str!("../verilog.rs")),
    ("zig", include_str!("../zig.rs")),
];

/// The languages that name kinds, in Go's init order: the files of the package by name
/// (`bash.go:43` ... `verilog.go:148`, plan hazard "KindMask registration order").
const GO_INIT_ORDER: [&str; 18] = [
    "bash",
    "c",
    "cpp",
    "csharp",
    "golang",
    "java",
    "javascript",
    "julia",
    "kotlin",
    "lua",
    "php",
    "python",
    "ruby",
    "rust",
    "scala",
    "terraform",
    "verilog",
    "zig",
];

/// The lines of `src` that are not `//` comments.
fn code_lines(src: &str) -> impl Iterator<Item = &str> {
    src.lines().filter(|l| !trim_space(l).starts_with("//"))
}

/// `kind_mask(` is called only inside a `fn register_kinds` (and in `kinds.rs`), and no kind name
/// is passed to `walk_kinds` or `preorder_mask`, which take a mask registered at startup. The
/// scan is by line: a call split over lines is not seen.
#[test]
fn kinds_registered_at_startup() {
    // SOURCES is every module `mod.rs` declares.
    let declared: BTreeSet<&str> = code_lines(include_str!("../mod.rs"))
        .filter_map(|l| {
            let l = l
                .strip_prefix("pub(crate) ")
                .or_else(|| l.strip_prefix("pub "))
                .unwrap_or(l);
            l.strip_prefix("mod ")?.strip_suffix(';')
        })
        .filter(|n| *n != "tests")
        .collect();
    let listed: BTreeSet<&str> = SOURCES
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| *n != "mod")
        .collect();
    assert_eq!(
        declared, listed,
        "`extract/mod.rs` declares modules that `SOURCES` does not list (or the reverse)"
    );

    for (name, text) in SOURCES {
        if *name == "kinds" {
            continue;
        }
        let mut in_register = false;
        for (i, l) in text.lines().enumerate() {
            if trim_space(l).starts_with("//") {
                continue;
            }
            if l.contains("fn register_kinds") {
                in_register = !l.ends_with("{}");
            } else if l == "}" {
                in_register = false;
            }
            assert!(
                in_register || !l.contains("kind_mask("),
                "{name}.rs:{}: `kind_mask(` outside `fn register_kinds`; a worker must never \
                 name a new kind during extraction",
                i + 1
            );
            for f in ["walk_kinds(", "preorder_mask("] {
                if let Some(at) = l.find(f) {
                    let args = &l[at..];
                    let args = args.split(';').next().unwrap_or(args);
                    assert!(
                        !args.contains('"'),
                        "{name}.rs:{}: a kind name is passed to `{f}`; pass a mask registered \
                         in `register_kinds`",
                        i + 1
                    );
                }
            }
        }
    }
}

/// The bit of a kind is its position in the order the kinds were named, so `kinds::register_kinds`
/// must call the languages in Go's init order, and each of them must have a `register_kinds` of
/// its own. (The order of the names inside one language is pinned by the grammar PR that adds
/// them, against the `kinds()` masks.)
#[test]
fn kind_bits_match_go_order() {
    let order: Vec<&str> = code_lines(include_str!("../kinds.rs"))
        .filter_map(|l| {
            trim_space(l)
                .strip_prefix("super::")?
                .strip_suffix("::register_kinds();")
        })
        .collect();
    assert_eq!(
        order, GO_INIT_ORDER,
        "`kinds::register_kinds` must name the languages in Go's init order"
    );
    for lang in GO_INIT_ORDER {
        let text = SOURCES
            .iter()
            .find(|(n, _)| *n == lang)
            .map(|(_, t)| *t)
            .unwrap_or_else(|| panic!("{lang}.rs is not in SOURCES"));
        assert!(
            text.contains("fn register_kinds"),
            "{lang}.rs has no `fn register_kinds`"
        );
    }
}
