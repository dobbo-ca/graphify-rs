//! graphify-go `internal/extract/local_imports_test.go`.

use std::collections::BTreeMap;

use crate::extract::{Result, file_from_bytes, resolve};

/// Go `importEdges`: extracts each source by extension, resolves the corpus with the given go.mod
/// dir -> module path table, and returns the sorted import edges.
fn import_edges(srcs: &[(&str, &str)], go_mods: &[(&str, &str)]) -> Vec<String> {
    crate::extract::kinds::register_kinds();
    let mut results = vec![Result {
        go_mods: go_mods
            .iter()
            .map(|(d, m)| (d.to_string(), m.to_string()))
            .collect::<BTreeMap<_, _>>(),
        ..Result::default()
    }];
    let mut files: Vec<String> = Vec::new();
    for (f, s) in srcs {
        results.push(file_from_bytes(f, s.as_bytes()));
        files.push(f.to_string());
    }
    files.sort();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut edges: Vec<String> = resolve(&results, &files, workers)
        .edges
        .iter()
        .filter(|e| matches!(e.relation.as_str(), "imports" | "imports_from"))
        .map(|e| format!("{} {} {}", e.source, e.relation.as_str(), e.target))
        .collect();
    edges.sort();
    edges
}

/// (name, sources, go.mod dir -> module path, want)
type Case = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
);

const ROOT: &[(&str, &str)] = &[(".", "example.com/m")];

const CASES: &[Case] = &[
    (
        "go package links every non-test file",
        &[
            (
                "main.go",
                "package main\nimport (\n\t\"fmt\"\n\t\"example.com/m/util\"\n)\n",
            ),
            ("util/a.go", "package util\n"),
            ("util/b_test.go", "package util\n"),
        ],
        ROOT,
        &["main_go imports fmt", "main_go imports_from util_a_go"],
    ),
    // No edge into a_test.go, so no cycle with b.
    (
        "go external test importing its importer",
        &[
            ("a/a.go", "package a\n"),
            (
                "a/a_test.go",
                "package a_test\nimport \"example.com/m/b\"\n",
            ),
            ("b/b.go", "package b\nimport \"example.com/m/a\"\n"),
        ],
        ROOT,
        &[
            "a_a_test_go imports_from b_b_go",
            "b_b_go imports_from a_a_go",
        ],
    ),
    (
        "go nested module wins",
        &[
            (
                "main.go",
                "package main\nimport \"example.com/m/sub/util\"\nimport \"example.com/sub/util\"\n",
            ),
            ("sub/util/a.go", "package util\n"),
        ],
        &[(".", "example.com/m"), ("sub", "example.com/sub")],
        &[
            "main_go imports example_com_m_sub_util",
            "main_go imports_from sub_util_a_go",
        ],
    ),
    (
        "go external test package skips itself",
        &[
            ("util/a.go", "package util\n"),
            (
                "util/a_test.go",
                "package util_test\nimport \"example.com/m/util\"\n",
            ),
        ],
        ROOT,
        &["util_a_test_go imports_from util_a_go"],
    ),
    (
        "go module under a dot directory",
        &[
            (
                ".tools/gen/main.go",
                "package main\nimport \"example.com/dot/util\"\n",
            ),
            (".tools/gen/util/u.go", "package util\n"),
        ],
        &[(".tools/gen", "example.com/dot")],
        &["tools_gen_main_go imports_from tools_gen_util_u_go"],
    ),
    (
        "go same module path in two directories",
        &[
            (
                "a/main.go",
                "package main\nimport \"example.com/same/util\"\n",
            ),
            ("a/util/u.go", "package util\n"),
            (
                "b/main.go",
                "package main\nimport \"example.com/same/util\"\n",
            ),
            ("b/util/u.go", "package util\n"),
        ],
        &[("a", "example.com/same"), ("b", "example.com/same")],
        &[
            "a_main_go imports_from a_util_u_go",
            "b_main_go imports_from b_util_u_go",
        ],
    ),
    (
        "go without go.mod stays external",
        &[
            ("main.go", "package main\nimport \"example.com/m/util\"\n"),
            ("util/a.go", "package util\n"),
        ],
        &[],
        &["main_go imports example_com_m_util"],
    ),
    (
        "c include beside the file",
        &[
            (
                "src/main.c",
                "#include \"h.h\"\n#include <stdio.h>\n#include \"gone.h\"\n",
            ),
            ("src/h.h", ""),
            ("src/stdio.h", ""),
        ],
        &[],
        &[
            "src_main_c imports gone_h",
            "src_main_c imports stdio_h",
            "src_main_c imports_from src_h_h",
        ],
    ),
    (
        "cpp include beside the file",
        &[
            ("src/w.cpp", "#include \"w.hpp\"\n#include <vector>\n"),
            ("src/w.hpp", ""),
        ],
        &[],
        &[
            "src_w_cpp imports vector",
            "src_w_cpp imports_from src_w_hpp",
        ],
    ),
    (
        "rust mod and crate paths",
        &[
            (
                "src/main.rs",
                "mod helper;\nmod deep;\nmod gone;\nuse crate::helper::assist;\nuse crate::deep::leaf;\nuse std::fmt;\n",
            ),
            ("src/helper.rs", "pub fn assist() {}\n"),
            (
                "src/deep/mod.rs",
                "pub mod leaf;\nuse super::helper;\nuse self::leaf::Leaf;\n",
            ),
            ("src/deep/leaf.rs", "use crate::{helper, deep::Other};\n"),
        ],
        &[],
        &[
            "src_deep_leaf_rs imports_from src_deep_mod_rs",
            "src_deep_leaf_rs imports_from src_helper_rs",
            "src_deep_mod_rs imports_from src_deep_leaf_rs",
            "src_deep_mod_rs imports_from src_helper_rs",
            "src_main_rs imports self_gone",
            "src_main_rs imports std",
            "src_main_rs imports_from src_deep_leaf_rs",
            "src_main_rs imports_from src_deep_mod_rs",
            "src_main_rs imports_from src_helper_rs",
        ],
    ),
    (
        "rust mod under a non-mod.rs file",
        &[
            ("src/lib.rs", "mod a;\n"),
            ("src/a.rs", "mod b;\n"),
            ("src/a/b.rs", "use super::super::a;\n"),
        ],
        &[],
        &[
            "src_a_b_rs imports_from src_a_rs",
            "src_a_rs imports_from src_a_b_rs",
            "src_lib_rs imports_from src_a_rs",
        ],
    ),
    (
        "ruby require_relative",
        &[
            (
                "app/main.rb",
                "require_relative 'lib/h'\nrequire_relative './lib/h'\nrequire 'json'\nrequire_relative 'gone'\n",
            ),
            ("app/lib/h.rb", ""),
            ("app/json.rb", ""),
        ],
        &[],
        &[
            "app_main_rb imports gone",
            "app_main_rb imports json",
            "app_main_rb imports_from app_lib_h_rb",
        ],
    ),
];

#[test]
#[ignore = "graphify-rs-ihc.33"] // also needs resolve (ihc.18)
fn test_local_imports_resolve_to_files() {
    for (name, srcs, go_mods, want) in CASES {
        let got = import_edges(srcs, go_mods);
        assert_eq!(&got[..], *want, "{name}: got {got:?}\nwant {want:?}");
    }
}
