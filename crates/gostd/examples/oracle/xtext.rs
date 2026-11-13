//! `xtext`: `cases.Fold`, `norm.NFKC` and `norm.NFKD`, and the idutil fixpoint loop. The `sweep`
//! case runs every scalar value through all three.

use super::Op;
use gostd::xtext::{cases, norm};
use std::fmt::Write;

/// Each char as `%04X`, joined by `,`; `-` for an empty string.
fn hex_runes(s: &str) -> String {
    if s.is_empty() {
        return "-".into();
    }
    let mut b = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 {
            b.push(',');
        }
        write!(b, "{:04X}", c as u32).unwrap();
    }
    b
}

pub fn run(name: &str, input: &[u8]) -> Vec<Op> {
    if name == "sweep" {
        let mut b = String::new();
        for c in (0..=0x10FFFF).filter_map(char::from_u32) {
            let s = c.to_string();
            let (f, kc, kd) = (cases::fold(&s), norm::nfkc(&s), norm::nfkd(&s));
            writeln!(
                b,
                "{:04X} {} {} {}",
                c as u32,
                hex_runes(&f),
                hex_runes(&kc),
                hex_runes(&kd)
            )
            .unwrap();
        }
        return vec![("sweep", Ok(b.into_bytes()))];
    }
    let s = std::str::from_utf8(input).expect("xtext inputs are valid UTF-8");
    let mut fix = String::new();
    let mut cur = s.to_string();
    for _ in 0..6 {
        let next = norm::nfkc(&cases::fold(&cur));
        fix.push_str(&next);
        fix.push('\n');
        if next == cur {
            break;
        }
        cur = next;
    }
    vec![
        ("fold", Ok(cases::fold(s).into_bytes())),
        ("nfkc", Ok(norm::nfkc(s).into_bytes())),
        ("nfkd", Ok(norm::nfkd(s).into_bytes())),
        ("fixpoint", Ok(fix.into_bytes())),
    ]
}
