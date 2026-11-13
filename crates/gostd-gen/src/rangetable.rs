//! `rangetable.New` of golang.org/x/text v0.27.0 (`unicode/rangetable`): sorted, deduplicated
//! runes packed into stride ranges. For a list of single runes, `Merge` (merge.go) reduces to a
//! greedy pass: a range of one rune takes the next rune at any stride; a longer range takes it
//! only at its own stride.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub lo: u32,
    pub hi: u32,
    pub stride: u32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub r16: Vec<Range>,
    pub r32: Vec<Range>,
    pub latin_offset: usize,
}

pub fn new(runes: &[u32]) -> Table {
    let mut r = runes.to_vec();
    r.sort_unstable();
    r.dedup();
    let (lo, hi): (Vec<u32>, Vec<u32>) = r.iter().partition(|&&c| c <= 0xFFFF);
    let mut t = Table {
        r16: pack(&lo),
        r32: pack(&hi),
        latin_offset: 0,
    };
    t.latin_offset = t.r16.iter().take_while(|r| r.hi <= 0xFF).count();
    t
}

fn pack(runes: &[u32]) -> Vec<Range> {
    let mut out = Vec::new();
    let Some((&first, rest)) = runes.split_first() else {
        return out;
    };
    let mut r0 = Range {
        lo: first,
        hi: first,
        stride: 1,
    };
    for &c in rest {
        let stride = c - r0.hi;
        if r0.lo == r0.hi || stride == r0.stride {
            r0.hi = c;
            r0.stride = stride;
        } else {
            out.push(r0);
            r0 = Range {
                lo: c,
                hi: c,
                stride: 1,
            };
        }
    }
    out.push(r0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(lo: u32, hi: u32, stride: u32) -> Range {
        Range { lo, hi, stride }
    }

    #[test]
    fn strides_as_go() {
        let t = new(&[0x46, 0x41, 0x43, 0x45, 0x41]);
        assert_eq!(t.r16, [r(0x41, 0x45, 2), r(0x46, 0x46, 1)]);
        assert_eq!(t.latin_offset, 2);
    }

    #[test]
    fn split_16_32_and_latin_offset() {
        let t = new(&[0x30, 0x31, 0x32, 0x100, 0x10000, 0x10002]);
        assert_eq!(t.r16, [r(0x30, 0x32, 1), r(0x100, 0x100, 1)]);
        assert_eq!(t.r32, [r(0x10000, 0x10002, 2)]);
        assert_eq!(t.latin_offset, 1);
    }

    #[test]
    fn empty() {
        assert_eq!(new(&[]), Table::default());
    }
}
