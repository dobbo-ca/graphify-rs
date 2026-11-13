//! `gostd-gen <ucd-dir> <out-dir>` writes the gostd Unicode tables from the UCD files in
//! `<ucd-dir>/<version>/`. It runs only in CI (the `gen-build` stage of ci/Dockerfile); its
//! output is committed in crates/gostd/src/tables and checked by the `ci` stage.

mod rangetable;
mod ucd;
mod unicode17;
mod xtext15;

use std::path::Path;
use std::{fs, process};

fn read(dir: &Path, version: &str, file: &str) -> String {
    let p = dir.join(version).join(file);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [ucd_dir, out_dir] = args.as_slice() else {
        eprintln!("usage: gostd-gen <ucd-dir> <out-dir>");
        process::exit(2);
    };
    let (ucd_dir, out_dir) = (Path::new(ucd_dir), Path::new(out_dir));
    let v17 = |f| read(ucd_dir, "17.0.0", f);
    let files = unicode17::Files {
        unicode_data: v17("UnicodeData.txt"),
        case_folding: v17("CaseFolding.txt"),
        property_value_aliases: v17("PropertyValueAliases.txt"),
        scripts: v17("Scripts.txt"),
        prop_list: v17("PropList.txt"),
    };
    fs::create_dir_all(out_dir).expect("create out dir");
    fs::write(out_dir.join("unicode17.rs"), unicode17::generate(&files))
        .expect("write unicode17.rs");
    let v15 = |f| read(ucd_dir, "15.0.0", f);
    let files = xtext15::Files {
        unicode_data: v15("UnicodeData.txt"),
        case_folding: v15("CaseFolding.txt"),
        composition_exclusions: v15("CompositionExclusions.txt"),
    };
    fs::write(out_dir.join("norm15.rs"), xtext15::norm(&files)).expect("write norm15.rs");
    fs::write(out_dir.join("cases15.rs"), xtext15::cases(&files)).expect("write cases15.rs");
}
