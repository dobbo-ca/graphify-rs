//! `bench <inputs> <corpus>`: times five workloads on gostd and prints `<workload> <total ns>` per
//! line (stdout; checks go to stderr). parity/go/oracle/bench.go times the same workloads on Go
//! (`oracle -bench`); parity/bench.sh joins the two. Keep the two files in step. Each workload
//! runs once. The workloads, the generated data and `<inputs>` are described in bench.go.
//!
//! `json_decode` is skipped: gostd json has no unmarshal until graphify-rs-aa6.10.

use gostd::json::{self, Value};
use gostd::regexp::Regexp;
use gostd::sha256::Digest;
use gostd::toml;
use gostd::xtext::norm;
use std::path::Path;
use std::time::Instant;
use std::{env, fs};

const SEED: u64 = 0x2545_F491_4F6C_DD1D;
const NODES: u64 = 150_000;
const LINKS: usize = 280_000;

/// The 43 constant patterns of graphify-go: `regexpConstants` in parity/go/oracle/regexp.go.
const PATTERNS: &[&str] = &[
    r##"[^0-9a-zA-Z_]+"##,
    r##"^[0-9a-f]{7,40}$"##,
    r##"[\x00-\x1f\x7f]"##,
    r##"(?i)(^|[\\/])\.(env|envrc)(\.|$)"##,
    r##"(?i)\.(pem|key|p12|pfx|cert|crt|der|p8)$"##,
    r##"(id_rsa|id_dsa|id_ecdsa|id_ed25519)(\.pub)?$"##,
    r##"(?i)(\.netrc|\.pgpass|\.htpasswd)$"##,
    r##"(?i)(?:credential|secret|passwd|password|private_key)s?"##,
    r##"[-_\s]+"##,
    r##"^module\s+(\S+)"##,
    r##"^require\s*\("##,
    r##"^(\S+)\s+v\S+"##,
    r##"^require\s+(\S+)\s+v\S+"##,
    r##"[\s<>=!~;\[\(]"##,
    r##"\$\{([^}]+)\}"##,
    r##"^[A-Za-z_][A-Za-z0-9_]*(?:(?:::|\.)[A-Za-z_][A-Za-z0-9_]*)*$"##,
    r##"::|\."##,
    r##"^(?:class|struct)\s+((?:[A-Z][A-Z0-9_]*\s+)+)([A-Za-z_]\w*(?:\s+final)?\s*)([:{])"##,
    r##"(?i)(password|passwd|secret|token|api[-_]?key|access[-_]?key|private[-_]?key|credential|client[-_]?secret|connection[-_]?string|sas[-_]?token|auth|bearer|passphrase)"##,
    r##"://[^/@\s]*:[^@\s]+@"##,
    r##"^[A-Za-z_][\w.\[\]*-]*$"##,
    r##"^[A-Z]\w*$"##,
    r##"(?m)^revision\s*[:=]"##,
    r##"\[[^\]]*\]\((?:<([^>]+)>|([^)\s]+))"##,
    r##"^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?"##,
    r##"\[\[([^\]|#\\]+)\\?(?:[#|][^\]]*)?\]\]"##,
    r##"^(#{1,6})\s+(.+)"##,
    "`([^`\n]+)`",
    r##"^@[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*(?:@[\w.\-+]+)?$"##,
    r##"^[a-z0-9][a-z0-9._-]*-mcp(?:-[a-z0-9._-]+)?$|^mcp-[a-z0-9][a-z0-9._-]*$"##,
    r##"^-{1,2}\w"##,
    r##"^\s*(//|/\*|\*)"##,
    r##"(?i)\b(ADR[- ]?\d{1,5}|RFC[- ]?\d{1,5})\b"##,
    r##"([A-Za-z]+)[- ]?(\d+)"##,
    r##"(?i)^test_|_test\..|\.test\..|\.spec\..|_spec\..|\.tests\.ps1$"##,
    r##"Tests?\.java$|Tests\.cs$"##,
    r##"(?i)(<script\b(?:"[^"]*"|'[^']*'|[^>"'])*>)([\s\S]*?)(</script\s*>)"##,
    r##"(?i)\blang\s*=\s*['"]?([A-Za-z]+)['"]?"##,
    r##"\A\s*---\s*\r?\n([\s\S]*?)\r?\n---\s*(?:\r?\n|\z)"##,
    r##"import\(\s*['"]([^'"]+)['"]\s*\)"##,
    r##"[\p{L}\p{N}_]+"##,
    r##"[^\p{L}\p{N}_]+"##,
    r##"_+"##,
];

/// The xorshift64 generator of parity/go/oracle/main.go (`rng.next`).
fn next(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn time<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    println!("{name} {}", t.elapsed().as_nanos());
    r
}

fn obj<const N: usize>(kv: [(&str, Value); N]) -> Value {
    Value::Object(kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn s(x: impl Into<String>) -> Value {
    Value::String(x.into())
}

/// `benchGraph` in bench.go.
fn graph() -> Value {
    let mut x = SEED;
    let nodes: Vec<Value> = (0..NODES)
        .map(|i| {
            let v = next(&mut x);
            obj([
                ("id", s(format!("n{i}"))),
                ("label", s(format!("sym_{:x}", v & 0xff_ffff))),
                (
                    "source_file",
                    s(format!(
                        "pkg/dir{}/file{}.go",
                        (v >> 24) % 500,
                        (v >> 40) % 50
                    )),
                ),
                ("file_type", s("code")),
                ("community", Value::Number(((v >> 48) % 1000) as f64)),
            ])
        })
        .collect();
    let relations = ["calls", "imports", "contains", "references"];
    let confidences = ["EXTRACTED", "INFERRED"];
    let links: Vec<Value> = (0..LINKS)
        .map(|_| {
            let v = next(&mut x);
            obj([
                ("source", s(format!("n{}", v % NODES))),
                ("target", s(format!("n{}", (v >> 20) % NODES))),
                ("relation", s(relations[((v >> 40) % 4) as usize])),
                ("confidence", s(confidences[((v >> 44) % 2) as usize])),
                ("weight", Value::Number(((v >> 48) % 1000) as f64 / 1000.0)),
            ])
        })
        .collect();
    obj([
        ("directed", Value::Bool(false)),
        ("nodes", Value::Array(nodes)),
        ("links", Value::Array(links)),
    ])
}

/// `benchFiles` in bench.go: the files of `dir` (sorted by name) whose name starts with `prefix`.
fn files(dir: &Path, prefix: &str) -> Vec<Vec<u8>> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("dir entry").file_name())
        .filter(|n| n.to_string_lossy().starts_with(prefix))
        .collect();
    names.sort();
    names
        .iter()
        .map(|n| fs::read(dir.join(n)).expect("read"))
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let [inputs, corpus_path] = args.as_slice() else {
        eprintln!("usage: bench <inputs> <corpus>");
        std::process::exit(2);
    };
    let inputs = Path::new(inputs);

    let corpus = fs::read_to_string(corpus_path).expect("read corpus");
    let lines: Vec<&str> = corpus.split('\n').collect();
    let res: Vec<Regexp> = PATTERNS
        .iter()
        .map(|p| Regexp::compile(p).expect("pattern compiles"))
        .collect();
    let matches = time("regexp_find_all", || {
        let mut n = 0;
        for re in &res {
            for l in &lines {
                n += re.find_all_string_index(l, -1).len();
            }
        }
        n
    });
    eprintln!("regexp lines {} matches {matches}", lines.len());

    let graph = graph();
    let indented = time("json_marshal_indent", || {
        json::marshal_indent(&graph, "", "  ").expect("marshal_indent")
    });
    let mut compact = Vec::new();
    time("json_compact", || {
        json::compact(&mut compact, &indented, false).expect("compact");
    });
    let valid = time("json_valid", || json::valid(&indented));
    eprintln!(
        "json indented {} compact {} valid {valid}",
        indented.len(),
        compact.len()
    );
    println!("json_decode skipped");

    let strs: Vec<String> = files(&inputs.join("xtext"), "")
        .into_iter()
        .map(|b| String::from_utf8(b).expect("xtext input is UTF-8"))
        .collect();
    let nfkc_len = time("norm_nfkc", || {
        let mut n = 0;
        for _ in 0..20 {
            for t in &strs {
                n += norm::nfkc(t).len();
            }
        }
        n
    });
    eprintln!("norm strings {} bytes out {nfkc_len}", strs.len());

    let mut chunk = vec![0u8; 1 << 20];
    let mut x = SEED;
    for b in &mut chunk {
        *b = next(&mut x) as u8;
    }
    let sum = time("sha256_1gib", || {
        let mut h = Digest::new();
        for _ in 0..1024 {
            h.write(&chunk);
        }
        h.sum()
    });
    eprintln!(
        "sha256 {}",
        sum.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );

    let docs = files(&inputs.join("toml"), "t_valid__");
    let toml_errs = time("toml_decode_x100", || {
        let mut n = 0;
        for _ in 0..100 {
            for d in &docs {
                if toml::decode(d).is_err() {
                    n += 1;
                }
            }
        }
        n
    });
    eprintln!("toml docs {} errors {toml_errs}", docs.len());
}
