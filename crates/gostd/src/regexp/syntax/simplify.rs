//! Port of Go 1.27.4 regexp/syntax/simplify.go: Regexp.Simplify and simplify1.
//!
//! Go returns the original node when nothing changes and shares one `sub` between the copies it
//! builds. The Rust tree owns its children, so unchanged nodes are cloned and shared subtrees are
//! cloned per use; the resulting trees are structurally identical to Go's.

use super::{Flags, Op, Regexp};

/// A node with only `op` and `flags` set (Go `&Regexp{Op: op, Flags: flags}`).
fn node(op: Op, flags: Flags, sub: Vec<Regexp>) -> Regexp {
    Regexp {
        op,
        flags,
        sub,
        rune: Vec::new(),
        min: 0,
        max: 0,
        cap: 0,
        name: String::new(),
    }
}

/// Go `Regexp.Simplify` (simplify.go:14).
pub(super) fn simplify(re: &Regexp) -> Regexp {
    match re.op {
        Op::Capture | Op::Concat | Op::Alternate => {
            // Simplify children. Go copies lazily (with `Rune = nil`); always copying is equivalent.
            Regexp {
                sub: re.sub.iter().map(simplify).collect(),
                rune: Vec::new(),
                name: re.name.clone(),
                ..*re
            }
        }

        Op::Star | Op::Plus | Op::Quest => {
            let sub = simplify(&re.sub[0]);
            simplify1(re.op, re.flags, sub, Some(re))
        }

        Op::Repeat => {
            // Special special case: x{0} matches the empty string
            // and doesn't even need to consider x.
            if re.min == 0 && re.max == 0 {
                return node(Op::EmptyMatch, Flags(0), Vec::new());
            }

            // The fun begins.
            let sub = simplify(&re.sub[0]);

            // x{n,} means at least n matches of x.
            if re.max == -1 {
                // Special case: x{0,} is x*.
                if re.min == 0 {
                    return simplify1(Op::Star, re.flags, sub, None);
                }

                // Special case: x{1,} is x+.
                if re.min == 1 {
                    return simplify1(Op::Plus, re.flags, sub, None);
                }

                // General case: x{4,} is xxxx+.
                let mut subs = vec![sub.clone(); usize::try_from(re.min - 1).unwrap_or(0)];
                subs.push(simplify1(Op::Plus, re.flags, sub, None));
                return node(Op::Concat, Flags(0), subs);
            }

            // Special case x{0} handled above.

            // Special case: x{1} is just x.
            if re.min == 1 && re.max == 1 {
                return sub;
            }

            // General case: x{n,m} means n copies of x and m copies of x?
            // The machine will do less work if we nest the final m copies,
            // so that x{2,5} = xx(x(x(x)?)?)?

            // Build leading prefix: xx.
            let mut prefix = None;
            if re.min > 0 {
                let subs = vec![sub.clone(); re.min as usize];
                prefix = Some(node(Op::Concat, Flags(0), subs));
            }

            // Build and attach suffix: (x(x(x)?)?)?
            if re.max > re.min {
                let mut suffix = simplify1(Op::Quest, re.flags, sub.clone(), None);
                for _ in i64::from(re.min) + 1..i64::from(re.max) {
                    let nre2 = node(Op::Concat, Flags(0), vec![sub.clone(), suffix]);
                    suffix = simplify1(Op::Quest, re.flags, nre2, None);
                }
                match prefix.as_mut() {
                    None => return suffix,
                    Some(p) => p.sub.push(suffix),
                }
            }
            if let Some(p) = prefix {
                return p;
            }

            // Some degenerate case like min > max or min < max < 0.
            // Handle as impossible match.
            node(Op::NoMatch, Flags(0), Vec::new())
        }

        _ => re.clone(),
    }
}

/// Go `simplify1` (simplify.go:134): the simple regexp equivalent to
/// `Regexp{Op: op, Flags: flags, Sub: {sub}}`, assuming `sub` is already simple. If that turns
/// out to be equivalent to `re`, returns (a clone of) `re`. Deviation from the layout stub: `sub`
/// is taken by value (it is moved into the result) and `re` is optional (Go's nil), and Go's
/// `sub == re.Sub[0]` pointer test is a structural comparison here.
pub(super) fn simplify1(op: Op, flags: Flags, sub: Regexp, re: Option<&Regexp>) -> Regexp {
    // Special case: repeat the empty string as much as
    // you want, but it's still the empty string.
    if sub.op == Op::EmptyMatch {
        return sub;
    }
    // The operators are idempotent if the flags match.
    if op == sub.op && flags & Flags::NON_GREEDY == sub.flags & Flags::NON_GREEDY {
        return sub;
    }
    if let Some(re) = re
        && re.op == op
        && re.flags & Flags::NON_GREEDY == flags & Flags::NON_GREEDY
        && sub == re.sub[0]
    {
        return re.clone();
    }

    node(op, flags, vec![sub])
}
