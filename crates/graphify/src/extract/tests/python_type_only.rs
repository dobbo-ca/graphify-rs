//! Upstream graphify v0.9.107 `tests/test_type_only_import_cycles.py`, the two Python stamp
//! cases (#3159): imports in the body of `if TYPE_CHECKING:` are type-only (Go `pyNestedImports`,
//! `python.go:72-88`). The cycle cases of that file belong to analyze.

use std::collections::{BTreeMap, BTreeSet};

use crate::extract::{file_from_bytes, resolve};
use crate::model::{Extraction, Relation};

/// Upstream `_extract`: every file extracted and resolved together.
fn extract_py(srcs: &[(&str, &str)]) -> Extraction {
    crate::extract::kinds::register_kinds();
    let results: Vec<_> = srcs
        .iter()
        .map(|(f, s)| file_from_bytes(f, s.as_bytes()))
        .collect();
    let files: Vec<String> = srcs.iter().map(|(f, _)| (*f).to_string()).collect();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    resolve(&results, &files, workers)
}

/// Upstream `_stamped_by_line`: {line: some imports/imports_from edge of that statement is
/// type-only} for one importing file.
fn stamped_by_line(ext: &Extraction, importer: &str) -> BTreeMap<u32, bool> {
    let mut out = BTreeMap::new();
    for e in &ext.edges {
        if e.source_file.as_str().ends_with(importer)
            && (e.relation == Relation::IMPORTS || e.relation == Relation::IMPORTS_FROM)
        {
            let loc = e.source_location.to_string();
            let line: u32 = loc[1..].parse().expect("an L<n> location");
            *out.entry(line).or_insert(false) |= e.type_only();
        }
    }
    out
}

fn stamped_lines(stamped: &BTreeMap<u32, bool>) -> Vec<u32> {
    stamped
        .iter()
        .filter(|(_, f)| **f)
        .map(|(l, _)| *l)
        .collect()
}

#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_python_imports_under_type_checking_are_stamped() {
    let ext = extract_py(&[
        ("pkg/__init__.py", ""),
        ("pkg/models.py", "class Item: ...\n"),
        (
            "pkg/service.py",
            concat!(
                "import sys\n",                                    // 1
                "import typing\n",                                 // 2
                "from typing import TYPE_CHECKING\n",              // 3
                "\n",                                              // 4
                "if TYPE_CHECKING:\n",                             // 5
                "    from pkg.models import Item\n",               // 6
                "    import pkg.models\n",                         // 7
                "    from .models import Item as Rel\n",           // 8
                "    if sys.version_info >= (3, 11):\n",           // 9
                "        from pkg.models import Item as Nested\n", // 10
                "if typing.TYPE_CHECKING:\n",                      // 11
                "    from pkg.models import Item as Attr\n",       // 12
            ),
        ),
    ]);
    let stamped = stamped_by_line(&ext, "service.py");
    assert_eq!(stamped_lines(&stamped), [6, 7, 8, 10, 12], "{stamped:?}");
}

/// The `else:` of the guard and `if not TYPE_CHECKING:` run at import time.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_python_imports_that_run_are_not_stamped() {
    let ext = extract_py(&[
        ("models.py", "class Item: ...\n"),
        ("fallback.py", "class Item: ...\n"),
        (
            "service.py",
            concat!(
                "from typing import TYPE_CHECKING\n",    // 1
                "import models\n",                       // 2
                "if TYPE_CHECKING:\n",                   // 3
                "    from models import Item\n",         // 4
                "else:\n",                               // 5
                "    from fallback import Item\n",       // 6
                "if not TYPE_CHECKING:\n",               // 7
                "    from models import Item as Late\n", // 8
            ),
        ),
    ]);
    let stamped = stamped_by_line(&ext, "service.py");
    assert_eq!(stamped_lines(&stamped), [4], "{stamped:?}");
    let lines: BTreeSet<u32> = stamped.keys().copied().collect();
    for want in [1, 2, 6, 8] {
        assert!(
            lines.contains(&want),
            "no import edge at line {want}: {stamped:?}"
        );
    }
}
