//! The Rust side of the gostd parity check (see parity/README.md).
//!
//! `oracle <inputs> <out>` runs every op on every input in `<inputs>/<pkg>/<case>` and writes
//! `<out>/<pkg>/<case>.<op>.out` (the output bytes) or `<out>/<pkg>/<case>.<op>.err` (the error
//! text and a newline), the same tree as the Go oracle in parity/go/oracle. The inputs are
//! written only by the Go oracle (`oracle -gen`).

mod echo;
mod pcg;
mod strconv;
mod strings;
mod unicode17;
mod xtext;

use std::fs;
use std::path::Path;
use std::process::exit;

/// One op's outcome: the output bytes, or the Go error text.
type Op = (&'static str, Result<Vec<u8>, String>);

/// A package's runner: case name and input bytes to its ops.
type Run = fn(&str, &[u8]) -> Vec<Op>;

/// The packages, by the directory name the Go oracle uses.
const PKGS: &[(&str, Run)] = &[
    ("echo", echo::run),
    ("pcg", pcg::run),
    ("strconv", strconv::run),
    ("strings", strings::run),
    ("unicode17", unicode17::run),
    ("xtext", xtext::run),
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
    for (pkg, run) in PKGS {
        let dir = inputs.join(pkg);
        let od = out.join(pkg);
        fs::create_dir_all(&od)?;
        let mut cases: Vec<_> = fs::read_dir(&dir)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<Result<_, _>>()?;
        cases.sort();
        for case in cases {
            let name = case.to_string_lossy();
            let data = fs::read(dir.join(&case))?;
            for (op, res) in run(&name, &data) {
                match res {
                    Ok(b) => fs::write(od.join(format!("{name}.{op}.out")), b)?,
                    Err(e) => fs::write(od.join(format!("{name}.{op}.err")), e + "\n")?,
                }
            }
        }
    }
    Ok(())
}
