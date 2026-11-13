//! The golang.org/x/text v0.27.0 (UCD 15.0.0) tables behind `cases.Fold`, `norm.NFKC` and
//! `norm.NFKD`: the per-rune data `unicode/norm/maketables.go` computes for its NFKC trie and
//! recomposition map, and the per-rune fold map of `cases` (see gostd's xtext module). The trie
//! itself is replaced by sorted per-rune tables.

use crate::ucd;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// The UCD files read.
pub struct Files {
    pub unicode_data: String,
    pub case_folding: String,
    pub composition_exclusions: String,
}

const HANGUL_BASE: u32 = 0xAC00;
const HANGUL_END: u32 = 0xD7A4;

/// The data of maketables.go `loadUnicodeData` and `loadCompositionExclusions`.
struct Norm {
    ccc: BTreeMap<u32, u8>,
    /// Canonical decomposition (maketables `forms[FCanonical].decomp`).
    cd: BTreeMap<u32, Vec<u32>>,
    /// Compatibility decomposition (`forms[FCompatibility].decomp`): the canonical one too.
    kd: BTreeMap<u32, Vec<u32>>,
    /// Runes whose decomposition has a `<tag>` (`compatDecomp`).
    compat: BTreeSet<u32>,
    excl: BTreeSet<u32>,
    /// `forms[FCompatibility].combinesBackward`.
    cb: BTreeSet<u32>,
}

/// One entry of the generated `NFKC_PROPS`.
#[derive(Debug, PartialEq, Eq)]
struct Props {
    ccc: u8,
    n_lead: u8,
    flags: u8,
    decomp: Vec<u32>,
}

fn hex_runes(s: &str) -> Vec<u32> {
    s.split_whitespace()
        .map(|x| u32::from_str_radix(x, 16).unwrap_or_else(|_| panic!("bad rune {x:?}")))
        .collect()
}

/// maketables.go `insertOrdered`: a stable insertion by CCC.
fn insert_ordered(d: &mut Vec<u32>, r: u32, ccc: u8, ccc_of: impl Fn(u32) -> u8) {
    let mut n = d.len();
    d.push(r);
    if ccc > 0 {
        while n > 0 && ccc_of(d[n - 1]) > ccc {
            d[n] = d[n - 1];
            n -= 1;
        }
    }
    d[n] = r;
}

impl Norm {
    fn load(f: &Files) -> Norm {
        let mut n = Norm {
            ccc: BTreeMap::new(),
            cd: BTreeMap::new(),
            kd: BTreeMap::new(),
            compat: BTreeSet::new(),
            excl: BTreeSet::new(),
            cb: BTreeSet::new(),
        };
        ucd::parse(&f.unicode_data, |p| {
            let r = p.rune;
            let c: u8 = p.string(3).parse().expect("ccc");
            if c != 0 {
                n.ccc.insert(r, c);
            }
            let d = p.string(5);
            if d.is_empty() {
                return;
            }
            if d.starts_with('<') {
                n.compat.insert(r);
                n.kd.insert(r, hex_runes(d.split_once('>').expect("tag").1));
            } else {
                n.cd.insert(r, hex_runes(d));
                n.kd.insert(r, hex_runes(d));
            }
        });
        ucd::parse(&f.composition_exclusions, |p| {
            n.excl.insert(p.rune);
        });
        // Phase 2 (combinesBackward), then the jamo V and T of phase 3.
        let mut cb = BTreeSet::new();
        for (&c, d) in &n.kd {
            if d.len() == 2 && !n.one_way(c, true) && !n.one_way(d[1], true) {
                cb.insert(d[1]);
            }
        }
        cb.extend(0x1161..0x1176);
        cb.extend(0x11A8..0x11C3);
        n.cb = cb;
        n
    }

    fn ccc(&self, r: u32) -> u8 {
        self.ccc.get(&r).copied().unwrap_or(0)
    }

    /// maketables.go `hasCompatDecomp`.
    fn has_compat(&self, r: u32) -> bool {
        self.compat.contains(&r)
            || self
                .kd
                .get(&r)
                .is_some_and(|d| d.iter().any(|&x| self.has_compat(x)))
    }

    /// maketables.go phase 1 `isOneWay`, for the compatibility (`k`) or canonical form.
    fn one_way(&self, r: u32, k: bool) -> bool {
        let d = if k { self.kd.get(&r) } else { self.cd.get(&r) };
        let len = d.map_or(0, Vec::len);
        self.excl.contains(&r)
            || len == 1
            || (len > 1 && {
                let d = d.expect("len > 1");
                self.ccc(r) != 0 || self.ccc(d[0]) != 0
            })
            || len > 2
            || (k && self.has_compat(r))
    }

    /// maketables.go `decomposeRecursive` over the compatibility form.
    fn expand_rec(&self, r: u32, d: &mut Vec<u32>) {
        match self.kd.get(&r) {
            None => insert_ordered(d, r, self.ccc(r), |x| self.ccc(x)),
            Some(v) => v.iter().for_each(|&c| self.expand_rec(c, d)),
        }
    }

    /// `forms[FCompatibility].expandedDecomp`: empty if `r` has no decomposition.
    fn expand(&self, r: u32) -> Vec<u32> {
        let mut d = Vec::new();
        for &c in self.kd.get(&r).map_or(&[][..], Vec::as_slice) {
            self.expand_rec(c, &mut d);
        }
        d
    }

    /// The runtime `Properties` of `r` in the NFKC trie (see gostd tables::NormProps).
    fn props(&self, r: u32) -> Props {
        let exp = self.expand(r);
        let runes = if exp.is_empty() { vec![r] } else { exp.clone() };
        let non_starter = |x: &u32| self.ccc(*x) != 0 || self.cb.contains(x);
        let n_lead = runes.iter().take_while(|x| non_starter(x)).count();
        let n_trail = runes.iter().rev().take_while(|x| non_starter(x)).count();
        assert!(n_trail <= 3, "{r:04X}: {n_trail} trailing non-starters");
        assert!(
            n_lead == 0 || n_lead == n_trail,
            "{r:04X}: leading {n_lead} != trailing {n_trail} non-starters"
        );
        let ccc = self.ccc(exp.first().copied().unwrap_or(r));
        let mut flags = n_trail as u8;
        if self.kd.contains_key(&r) {
            flags |= 0x04;
        }
        // QC_C (maketables.go phase 3).
        if self.one_way(r, true) {
            flags |= 0x10;
        } else if self.cb.contains(&r) {
            // Also the jamo block rule: 1100..11FF is Yes except V/T, which are in `cb`.
            flags |= 0x18;
        }
        Props {
            ccc,
            n_lead: n_lead as u8,
            flags,
            decomp: exp,
        }
    }

    /// The recomposition map (key = low 16 bits of each rune), a later rune winning a collision.
    fn recomp(&self) -> BTreeMap<u32, u32> {
        let mut m = BTreeMap::new();
        for (&r, d) in &self.cd {
            if !self.one_way(r, false) {
                assert_eq!(
                    d.len(),
                    2,
                    "{r:04X}: two-way decomposition must have 2 runes"
                );
                m.insert(((d[0] & 0xFFFF) << 16) | (d[1] & 0xFFFF), r);
            }
        }
        m
    }
}

/// A Rust string literal of `\u{X}` escapes.
fn lit(runes: &[u32]) -> String {
    let mut s = String::from("\"");
    for &r in runes {
        write!(s, "\\u{{{r:X}}}").unwrap();
    }
    s.push('"');
    s
}

/// `norm15.rs`.
pub fn norm(f: &Files) -> String {
    let n = Norm::load(f);
    let mut out = String::new();
    out.push_str("// Code generated by gostd-gen from UCD 15.0.0. DO NOT EDIT.\n");
    out.push_str(
        "// The NFKC/NFKD per-rune data and recomposition map of golang.org/x/text v0.27.0\n",
    );
    out.push_str("// unicode/norm (maketables.go).\n\n");
    out.push_str("use super::{np, NormProps};\n\n");
    out.push_str("pub const VERSION: &str = \"15.0.0\";\n\n");
    out.push_str("pub static NFKC_PROPS: &[(u32, NormProps)] = &[\n");
    for r in (0..=0x10FFFFu32).filter(|r| !(HANGUL_BASE..HANGUL_END).contains(r)) {
        if char::from_u32(r).is_none() {
            continue;
        }
        let p = n.props(r);
        if p.ccc == 0 && p.flags == 0 {
            continue;
        }
        writeln!(
            out,
            "    (0x{r:04X}, np({}, {}, 0x{:02X}, {})),",
            p.ccc,
            p.n_lead,
            p.flags,
            lit(&p.decomp)
        )
        .unwrap();
    }
    out.push_str("];\n\n");
    out.push_str("pub static RECOMP: &[(u32, u32)] = &[\n");
    for (k, r) in n.recomp() {
        writeln!(out, "    (0x{k:08X}, 0x{r:04X}),").unwrap();
    }
    out.push_str("];\n");
    out
}

/// The fold map: the CaseFolding C or F mapping if there is one, else the UnicodeData simple
/// lowercase; only runes it changes.
fn fold_map(f: &Files) -> BTreeMap<u32, Vec<u32>> {
    let mut m = BTreeMap::new();
    ucd::parse(&f.case_folding, |p| {
        if matches!(p.string(1), "C" | "F") {
            m.insert(p.rune, hex_runes(p.string(2)));
        }
    });
    ucd::parse(&f.unicode_data, |p| {
        if !p.string(13).is_empty() {
            m.entry(p.rune).or_insert_with(|| vec![p.rune_at(13)]);
        }
    });
    m.retain(|&r, v| v.as_slice() != [r]);
    m
}

/// `cases15.rs`.
pub fn cases(f: &Files) -> String {
    let mut out = String::new();
    out.push_str("// Code generated by gostd-gen from UCD 15.0.0. DO NOT EDIT.\n");
    out.push_str("// The `cases.Fold` map of golang.org/x/text v0.27.0.\n\n");
    out.push_str("pub const VERSION: &str = \"15.0.0\";\n\n");
    out.push_str("pub static FOLD: &[(u32, &str)] = &[\n");
    for (r, v) in fold_map(f) {
        writeln!(out, "    (0x{r:04X}, {}),", lit(&v)).unwrap();
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A UnicodeData line (fields 0..=14) with the given CCC, decomposition and lowercase.
    fn ud(cp: u32, ccc: u8, decomp: &str, lower: &str) -> String {
        format!("{cp:04X};X;Lo;{ccc};L;{decomp};;;;N;;;;{lower};\n")
    }

    fn files(unicode_data: &[String], excl: &str, folding: &str) -> Files {
        Files {
            unicode_data: unicode_data.concat(),
            case_folding: folding.to_owned(),
            composition_exclusions: excl.to_owned(),
        }
    }

    fn sample() -> Norm {
        let d = [
            ud(0x41, 0, "", ""),
            ud(0x73, 0, "", ""),
            ud(0x915, 0, "", ""),
            ud(0x93C, 7, "", ""),
            ud(0x300, 230, "", ""),
            ud(0x301, 230, "", ""),
            ud(0x307, 230, "", ""),
            ud(0x308, 230, "", ""),
            ud(0x323, 220, "", ""),
            ud(0xA0, 0, "<noBreak> 0020", ""),
            ud(0xC0, 0, "0041 0300", ""),
            ud(0xC1, 0, "0041 0301", ""),
            ud(0x1FA, 0, "00C1 0307", ""),
            ud(0x340, 230, "0300", ""),
            ud(0x344, 230, "0308 0301", ""),
            ud(0x958, 0, "0915 093C", ""),
            ud(0x17F, 0, "<compat> 0073", ""),
            ud(0x1E9B, 0, "017F 0307", ""),
            ud(0xE000, 0, "0073 0307 0323", ""),
        ];
        Norm::load(&files(&d, "0958 # excluded\n", ""))
    }

    #[test]
    fn composition_exclusion_singleton_and_non_starter() {
        let n = sample();
        let m = n.recomp();
        assert_eq!(m.get(&0x0041_0300), Some(&0xC0));
        assert_eq!(m.get(&0x0041_0301), Some(&0xC1));
        assert!(!m.values().any(|&r| r == 0x958), "script exclusion");
        assert!(!m.values().any(|&r| r == 0x340), "singleton");
        assert!(
            !m.values().any(|&r| r == 0x344),
            "non-starter decomposition"
        );
        assert!(!m.values().any(|&r| r == 0xE000), "three runes");
        assert!(!m.values().any(|&r| r == 0xA0), "compatibility only");
    }

    #[test]
    fn compat_vs_canonical() {
        let n = sample();
        // U+1E9B is canonical <017F, 0307>, but 017F is compat: recomposes, but NoC in NFKC.
        assert_eq!(n.recomp().get(&0x017F_0307), Some(&0x1E9B));
        assert!(n.one_way(0x1E9B, true) && !n.one_way(0x1E9B, false));
        assert_eq!(
            n.props(0x1E9B),
            Props {
                ccc: 0,
                n_lead: 0,
                flags: 0x15,
                decomp: vec![0x73, 0x307]
            }
        );
        assert_eq!(
            n.props(0xA0),
            Props {
                ccc: 0,
                n_lead: 0,
                flags: 0x14,
                decomp: vec![0x20]
            }
        );
    }

    #[test]
    fn recursive_decomposition_and_ordering() {
        let n = sample();
        assert_eq!(n.expand(0x1FA), [0x41, 0x301, 0x307]);
        // 0073 0307 0323: the lower-CCC mark moves before the higher one.
        assert_eq!(n.expand(0xE000), [0x73, 0x323, 0x307]);
    }

    #[test]
    fn props_of_marks_and_decomposed() {
        let n = sample();
        let p = |r| n.props(r);
        assert_eq!(
            p(0x301),
            Props {
                ccc: 230,
                n_lead: 1,
                flags: 0x19,
                decomp: vec![]
            }
        );
        // 0316-like: a mark that combines with nothing is Yes with one trailing non-starter.
        assert_eq!(
            p(0x323),
            Props {
                ccc: 220,
                n_lead: 1,
                flags: 0x01,
                decomp: vec![]
            }
        );
        assert_eq!(
            p(0xC0),
            Props {
                ccc: 0,
                n_lead: 0,
                flags: 0x05,
                decomp: vec![0x41, 0x300]
            }
        );
        assert_eq!(
            p(0x344),
            Props {
                ccc: 230,
                n_lead: 2,
                flags: 0x16,
                decomp: vec![0x308, 0x301]
            }
        );
        // Jamo V is a starter with one leading non-starter, combining backward.
        assert_eq!(
            p(0x1161),
            Props {
                ccc: 0,
                n_lead: 1,
                flags: 0x19,
                decomp: vec![]
            }
        );
        assert_eq!(p(0x1100).flags, 0);
    }

    #[test]
    fn fold_cf_then_lowercase_fallback() {
        let d = [
            ud(0x41, 0, "", "0061"),
            ud(0x61, 0, "", ""),
            ud(0x42, 0, "", "0062"),
            ud(0xDF, 0, "", ""),
            ud(0x1E9E, 0, "", "00DF"),
            ud(0x130, 0, "", "0069"),
        ];
        let cf = "0041; C; 0061; # A\n\
                  00DF; F; 0073 0073; # sharp s\n\
                  1E9E; F; 0073 0073; # capital sharp s\n\
                  1E9E; S; 00DF; # simple\n\
                  0130; F; 0069 0307; # dotted I\n\
                  0130; T; 0069; # Turkic\n";
        let m = fold_map(&files(&d, "", cf));
        assert_eq!(m.get(&0x41), Some(&vec![0x61]));
        assert_eq!(
            m.get(&0x42),
            Some(&vec![0x62]),
            "fallback to simple lowercase"
        );
        assert_eq!(m.get(&0xDF), Some(&vec![0x73, 0x73]));
        assert_eq!(
            m.get(&0x1E9E),
            Some(&vec![0x73, 0x73]),
            "F beats S and lowercase"
        );
        assert_eq!(m.get(&0x130), Some(&vec![0x69, 0x307]), "T ignored");
        assert!(!m.contains_key(&0x61));
    }

    #[test]
    fn output_format() {
        let n = norm(&files(
            &[ud(0xC0, 0, "0041 0300", ""), ud(0x300, 230, "", "")],
            "",
            "",
        ));
        assert!(n.starts_with("// Code generated by gostd-gen from UCD 15.0.0. DO NOT EDIT.\n"));
        assert!(n.contains("    (0x00C0, np(0, 0, 0x05, \"\\u{41}\\u{300}\")),\n"));
        assert!(n.contains("    (0x00410300, 0x00C0),\n"));
        let c = cases(&files(&[], "", "0041; C; 0061; #\n"));
        assert!(c.contains("    (0x0041, \"\\u{61}\"),\n"));
    }
}
