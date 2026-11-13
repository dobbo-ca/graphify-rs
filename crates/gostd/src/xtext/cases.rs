//! x/text `cases.Fold()` (`cases/fold.go`, `cases/context.go` `foldFull`).

use crate::tables::cases15::FOLD;

/// `cases.Fold().String(s)`: full case folding (CaseFolding.txt status C and F), UCD 15.0.0.
///
/// Ports `caseFolder.Transform` (`cases/fold.go:12`) over `foldFull` (`cases/context.go:334`).
/// There is no context, so it is one table lookup per rune; `FOLD` holds only runes that change.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match FOLD.binary_search_by_key(&(c as u32), |&(r, _)| r) {
            Ok(i) => out.push_str(FOLD[i].1),
            Err(_) => out.push(c),
        }
    }
    out
}
