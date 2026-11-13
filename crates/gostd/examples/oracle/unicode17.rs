//! `unicode17`: gostd::unicode against Go std unicode (see parity/go/oracle/unicode17.go).

use super::Op;
use gostd::unicode as u;
use std::fmt::Write;

fn scalars() -> impl Iterator<Item = char> {
    (0..=0x10FFFF).filter_map(char::from_u32)
}

fn bit(b: bool) -> char {
    if b { '1' } else { '0' }
}

fn props() -> String {
    let mut b = String::new();
    for r in scalars() {
        let flags: String = [
            u::is_letter(r),
            u::is_number(r),
            u::is_upper(r),
            u::is_space(r),
            u::is_print(r),
            u::is(&u::MN, r),
        ]
        .into_iter()
        .map(bit)
        .collect();
        writeln!(
            b,
            "{:04X} {flags} {:04X} {:04X} {:04X} {:04X}",
            r as u32,
            u::to_lower(r) as u32,
            u::to_upper(r) as u32,
            u::to_title(r) as u32,
            u::simple_fold(r) as u32
        )
        .unwrap();
    }
    b
}

fn tables() -> String {
    let mut b = String::new();
    let maps = [
        ("categories", u::CATEGORIES),
        ("scripts", u::SCRIPTS),
        ("properties", u::PROPERTIES),
        ("fold_category", u::FOLD_CATEGORY),
        ("fold_script", u::FOLD_SCRIPT),
    ];
    for (map, entries) in maps {
        for (name, t) in entries {
            let mut run: Option<(u32, u32)> = None;
            for r in scalars().filter(|&r| u::is(t, r)) {
                let r = r as u32;
                run = match run {
                    Some((lo, hi)) if hi + 1 == r => Some((lo, r)),
                    Some((lo, hi)) => {
                        writeln!(b, "{map} {name} {lo:04X}-{hi:04X}").unwrap();
                        Some((r, r))
                    }
                    None => Some((r, r)),
                };
            }
            if let Some((lo, hi)) = run {
                writeln!(b, "{map} {name} {lo:04X}-{hi:04X}").unwrap();
            }
        }
    }
    b
}

fn aliases() -> String {
    u::CATEGORY_ALIASES
        .iter()
        .map(|(k, v)| format!("{k} {v}\n"))
        .collect()
}

pub fn run(_name: &str, _input: &[u8]) -> Vec<Op> {
    vec![
        ("props", Ok(props().into_bytes())),
        ("tables", Ok(tables().into_bytes())),
        ("aliases", Ok(aliases().into_bytes())),
    ]
}
