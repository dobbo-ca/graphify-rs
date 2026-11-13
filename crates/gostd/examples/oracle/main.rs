//! The Rust side of the gostd parity check (see parity/README.md).
//!
//! `oracle <inputs> <out>` runs every op on every input in `<inputs>/<pkg>/<case>` and writes
//! `<out>/<pkg>/<case>.<op>.out` (the output bytes) or `<out>/<pkg>/<case>.<op>.err` (the error
//! text and a newline), the same tree as the Go oracle in parity/go/oracle. The inputs are
//! written only by the Go oracle (`oracle -gen`).

mod bedrock_body;
mod csv;
mod echo;
mod filepath;
mod json;
mod pcg;
mod regexp;
mod sha256;
mod sort;
mod strconv;
mod strings;
mod toml;
mod unicode17;
mod url;
mod xml_dec;
mod xml_enc;
mod xtext;

use std::fs;
use std::path::Path;
use std::process::exit;

/// One op's outcome: the output bytes, or the Go error text.
type Op = (&'static str, Result<Vec<u8>, String>);

/// A package's runner: case name and input bytes to its ops.
type Run = fn(&str, &[u8]) -> Vec<Op>;

/// The packages, by the directory name the Go oracle uses, with the fewest output files each may
/// write (the same number as its `register` call in parity/go/oracle/<pkg>.go). `run_all` fails if
/// a package writes fewer, so a package whose ops stop running cannot pass.
const PKGS: &[(&str, usize, Run)] = &[
    ("bedrock-body", 201, bedrock_body::run),
    ("csv", 2000, csv::run),
    ("echo", 4, echo::run),
    ("filepath", 40060, filepath::run),
    ("json", 1227, json::run),
    ("pcg", 11, pcg::run),
    ("regexp", 22833, regexp::run),
    ("sha256", 631, sha256::run),
    ("sort", 1200, sort::run),
    ("strconv", 36, strconv::run),
    ("strings", 8000, strings::run),
    ("toml", 2756, toml::run),
    ("unicode17", 3, unicode17::run),
    ("url", 5113, url::run),
    ("xml-dec", 3300, xml_dec::run),
    ("xml-enc", 500, xml_enc::run),
    ("xtext", 20053, xtext::run),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [inputs, out] = args.as_slice() else {
        eprintln!("usage: oracle <inputs> <out>");
        exit(2);
    };
    if let Err(e) = run_all(Path::new(inputs), Path::new(out)) {
        eprintln!("{e}");
        exit(1);
    }
}

fn run_all(inputs: &Path, out: &Path) -> std::io::Result<()> {
    for &(pkg, min, run) in PKGS {
        let dir = inputs.join(pkg);
        let od = out.join(pkg);
        fs::create_dir_all(&od)?;
        let mut cases: Vec<_> = fs::read_dir(&dir)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<Result<_, _>>()?;
        cases.sort();
        let mut count = 0;
        for case in cases {
            let name = case.to_string_lossy();
            let data = fs::read(dir.join(&case))?;
            for (op, res) in run(&name, &data) {
                match res {
                    Ok(b) => fs::write(od.join(format!("{name}.{op}.out")), b)?,
                    Err(e) => fs::write(od.join(format!("{name}.{op}.err")), e + "\n")?,
                }
                count += 1;
            }
        }
        println!("{pkg}: {count} outputs (minimum {min})");
        if count < min {
            return Err(std::io::Error::other(format!(
                "{pkg}: {count} outputs, want at least {min}"
            )));
        }
    }
    Ok(())
}
