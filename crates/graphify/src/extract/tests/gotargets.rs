//! graphify-go `internal/extract/gotargets_test.go`. It tests `importTargets`, `goModDir`,
//! `keep` and `contains` (`resolve.go`), so it is ported here and not with the Go extractor.

use std::collections::{BTreeMap, HashMap};

use crate::extract::Imp;
use crate::extract::resolve::{Corpus, contains, go_mod_dir, import_targets, keep};
use crate::testutil::Rng;

// The shared Go target lists must give what filtering the package per import
// gave: a file importing its own package, two checkouts of one module, and
// repeated files included.
#[test]
#[ignore = "graphify-rs-ihc.18"]
fn test_go_import_targets_match_per_import_filter() {
    let mut rng = Rng::new(3);
    for round in 0..300 {
        let mut go_mods: BTreeMap<String, String> = BTreeMap::new();
        let n = 1 + rng.intn(4);
        for dir in &[".", "a", "b", "a/vendor/x"][..n] {
            go_mods.insert(dir.to_string(), ["m", "m", "n"][rng.intn(3)].to_string());
        }
        let mut files: Vec<String> = Vec::new();
        let mut go_pkgs: HashMap<String, Vec<String>> = HashMap::new();
        let mut i = 0;
        while i < 1 + rng.intn(30) {
            let dir = [".", "a", "b", "a/vendor/x", "c"][rng.intn(5)];
            let sub = ["", "p", "q"][rng.intn(3)];
            let f = gostd::path::join(&[dir, sub, &format!("f{}.go", rng.intn(12))]);
            files.push(f.clone());
            let ip = ["m/p", "m/q", "n/p", "m"][rng.intn(4)];
            let pkg = go_pkgs.entry(ip.to_string()).or_default();
            pkg.push(f.clone());
            if rng.intn(6) == 0 {
                pkg.push(f);
                pkg.push(String::new());
            }
            i += 1;
        }
        let mut shared: HashMap<[String; 2], Vec<String>> = HashMap::new();
        for _ in 0..60 {
            let im = Imp {
                file: files[rng.intn(files.len())].clone(),
                spec: ["m/p", "m/q", "n/p", "m", "fmt"][rng.intn(5)].to_string(),
                ..Imp::default()
            };
            let mut want: Vec<String> = Vec::new();
            let mut pkg = go_pkgs.get(&im.spec).cloned().unwrap_or_default();
            let own = go_mod_dir(&go_mods, &im.file);
            let mine = keep(&pkg, |f| go_mod_dir(&go_mods, f) == own);
            if !mine.is_empty() {
                pkg = mine;
            }
            for f in &pkg {
                if !f.is_empty() && *f != im.file && !contains(&want, f) {
                    want.push(f.clone());
                }
            }
            let got = import_targets(&im, &Corpus::new(), &go_pkgs, &go_mods, &mut shared);
            assert_eq!(
                got, want,
                "round {round}: {} imports {}: got {got:?}, want {want:?}",
                im.file, im.spec
            );
        }
    }
}
