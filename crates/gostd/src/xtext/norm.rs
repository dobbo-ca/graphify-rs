//! x/text `unicode/norm` `NFKC.String` and `NFKD.String`, UCD 15.0.0, with x/text's
//! stream-safe rule: after 30 non-starters it inserts U+034F (`composition.go:10`).
//!
//! A literal port of the code `Form.String` reaches, keeping byte offsets into the `&str`.
//! The generated trie is replaced by `crate::tables::norm15::NFKC_PROPS`; both forms read it
//! (`forminfo.go:73-84`).

use crate::tables::norm15::{NFKC_PROPS, RECOMP};
use crate::tables::{NormProps, np};

const HANGUL_BASE: u32 = 0xAC00;
const HANGUL_END: u32 = 0xD7A4;
const JAMO_L_BASE: u32 = 0x1100;
const JAMO_V_BASE: u32 = 0x1161;
const JAMO_T_BASE: u32 = 0x11A7;
const JAMO_L_END: u32 = 0x1113;
const JAMO_V_END: u32 = 0x1176;
const JAMO_T_END: u32 = 0x11C3;
const JAMO_T_COUNT: u32 = 28;
const JAMO_VT_COUNT: u32 = 21 * 28;
const MAX_NON_STARTERS: u8 = 30;

/// `norm.NFKC.String(s)`.
///
/// Ports `Form.String` (`normalize.go:59`) with `formTable[NFKC]` (`forminfo.go:78`).
pub fn nfkc(s: &str) -> String {
    string(s, true)
}

/// `norm.NFKD.String(s)`.
///
/// Ports `Form.String` (`normalize.go:59`) with `formTable[NFKD]` (`forminfo.go:81`).
pub fn nfkd(s: &str) -> String {
    string(s, false)
}

/// Ports `compInfo` over the trie lookup (`forminfo.go:243`, `input.go:85`): the properties
/// of one rune. Hangul syllables are computed (`maketables.go:492`).
fn props(c: char) -> NormProps {
    let r = c as u32;
    if (HANGUL_BASE..HANGUL_END).contains(&r) {
        let lv = (r - HANGUL_BASE).is_multiple_of(JAMO_T_COUNT);
        return np(0, 0, 0x04 | if lv { 1 } else { 2 }, "");
    }
    match NFKC_PROPS.binary_search_by_key(&r, |&(k, _)| k) {
        Ok(i) => NFKC_PROPS[i].1,
        Err(_) => np(0, 0, 0, ""),
    }
}

fn is_yes_c(p: &NormProps) -> bool {
    p.flags & 0x10 == 0
}

fn is_yes_d(p: &NormProps) -> bool {
    p.flags & 0x04 == 0
}

fn combines_backward(p: &NormProps) -> bool {
    p.flags & 0x08 != 0
}

fn has_decomposition(p: &NormProps) -> bool {
    p.flags & 0x04 != 0
}

/// Ports `Properties.BoundaryBefore` (`forminfo.go:95`).
fn boundary_before(p: &NormProps) -> bool {
    p.ccc == 0 && !combines_backward(p)
}

#[derive(PartialEq, Eq)]
enum SsState {
    Success,
    Starter,
    Overflow,
}

/// Ports `streamSafe.next` (`composition.go:44`).
fn ss_next(ss: &mut u8, p: &NormProps) -> SsState {
    let n = p.n_lead;
    *ss += n;
    if *ss > MAX_NON_STARTERS {
        *ss = 0;
        return SsState::Overflow;
    }
    if n == 0 {
        *ss = p.flags & 3;
        return SsState::Starter;
    }
    SsState::Success
}

/// The rune at byte offset `i` and its properties and UTF-8 length.
fn info(src: &str, i: usize) -> (char, NormProps, usize) {
    let c = src[i..].chars().next().expect("offset inside src");
    (c, props(c), c.len_utf8())
}

/// Ports `input.skipASCII` (`input.go:39`).
fn skip_ascii(src: &str, mut p: usize, max: usize) -> usize {
    let b = src.as_bytes();
    while p < max && b[p] < 0x80 {
        p += 1;
    }
    p
}

/// Ports `formInfo.quickSpan` (`normalize.go:304`) with `atEOF` true.
fn quick_span(src: &str, mut i: usize, end: usize, composing: bool) -> (usize, bool) {
    let mut last_cc = 0u8;
    let mut ss = 0u8;
    let mut last_seg_start = i;
    while i < end {
        let j = skip_ascii(src, i, end);
        if i != j {
            i = j;
            last_seg_start = i - 1;
            last_cc = 0;
            ss = 0;
            continue;
        }
        let (_, p, size) = info(src, i);
        match ss_next(&mut ss, &p) {
            SsState::Starter => last_seg_start = i,
            SsState::Overflow => return (last_seg_start, false),
            SsState::Success => {
                if last_cc > p.ccc {
                    return (last_seg_start, false);
                }
            }
        }
        if composing {
            if !is_yes_c(&p) {
                break;
            }
        } else if !is_yes_d(&p) {
            break;
        }
        last_cc = p.ccc;
        i += size;
    }
    if i == end {
        return (end, true);
    }
    (last_seg_start, false)
}

/// One `reorderBuffer.rune` entry: the rune and the parts of its `Properties` that are read
/// afterwards. `assignRune`/`appendRune` zero `ccc` and the flags.
#[derive(Clone, Copy)]
struct Entry {
    ch: char,
    ccc: u8,
    cb: bool,
}

/// Ports `reorderBuffer` (`composition.go:20`); the flusher is always `appendFlush`.
struct ReorderBuffer<'a> {
    composing: bool,
    src: &'a str,
    out: String,
    ss: u8,
    buf: Vec<Entry>,
}

impl ReorderBuffer<'_> {
    /// Ports `reorderBuffer.insertOrdered` (`composition.go:184`).
    fn insert_ordered(&mut self, ch: char, p: &NormProps) {
        let mut n = self.buf.len();
        self.buf.push(Entry {
            ch,
            ccc: p.ccc,
            cb: combines_backward(p),
        });
        if p.ccc > 0 {
            while n > 0 {
                if self.buf[n - 1].ccc <= p.ccc {
                    break;
                }
                self.buf[n] = self.buf[n - 1];
                n -= 1;
            }
            self.buf[n] = Entry {
                ch,
                ccc: p.ccc,
                cb: combines_backward(p),
            };
        }
    }

    /// Ports `reorderBuffer.insertCGJ` (`composition.go:273`): `Properties{size: 2}`.
    fn insert_cgj(&mut self) {
        self.insert_ordered('\u{34F}', &np(0, 0, 0, ""));
    }

    /// Ports `reorderBuffer.appendRune` (`composition.go:278`): no CCC ordering.
    fn append_rune(&mut self, r: u32) {
        self.buf.push(Entry {
            ch: char::from_u32(r).expect("jamo"),
            ccc: 0,
            cb: false,
        });
    }

    /// Ports `reorderBuffer.assignRune` (`composition.go:287`).
    fn assign_rune(&mut self, pos: usize, r: u32) {
        self.buf[pos] = Entry {
            ch: char::from_u32(r).expect("composite"),
            ccc: 0,
            cb: false,
        };
    }

    /// Ports `reorderBuffer.decomposeHangul` (`composition.go:411`).
    fn decompose_hangul(&mut self, r: u32) {
        let r = r - HANGUL_BASE;
        let x = r % JAMO_T_COUNT;
        let r = r / JAMO_T_COUNT;
        self.append_rune(JAMO_L_BASE + r / 21);
        self.append_rune(JAMO_V_BASE + r % 21);
        if x != 0 {
            self.append_rune(JAMO_T_BASE + x);
        }
    }

    /// Ports `reorderBuffer.insertDecomposed` (`composition.go:249`).
    fn insert_decomposed(&mut self, dcomp: &str) {
        for c in dcomp.chars() {
            let p = props(c);
            if boundary_before(&p) && !self.buf.is_empty() {
                self.do_flush();
            }
            self.insert_ordered(c, &p);
        }
    }

    /// Ports `reorderBuffer.insertFlush` (`composition.go:218`).
    fn insert_flush(&mut self, ch: char, p: &NormProps) {
        let r = ch as u32;
        if (HANGUL_BASE..HANGUL_END).contains(&r) {
            self.decompose_hangul(r);
        } else if has_decomposition(p) {
            self.insert_decomposed(p.decomp);
        } else {
            self.insert_ordered(ch, p);
        }
    }

    /// Ports `reorderBuffer.doFlush` (`composition.go:139`) with `appendFlush` and `reset`.
    fn do_flush(&mut self) {
        if self.composing {
            self.compose();
        }
        for e in &self.buf {
            self.out.push(e.ch);
        }
        self.buf.clear();
    }

    /// Ports `reorderBuffer.combineHangul` (`composition.go:424`).
    fn combine_hangul(&mut self, mut s: usize, start: usize, mut k: usize) {
        for i in start..self.buf.len() {
            let ccc_b = self.buf[k - 1].ccc;
            let ccc_c = self.buf[i].ccc;
            if ccc_b == 0 {
                s = k - 1;
            }
            if s != k - 1 && ccc_b >= ccc_c {
                self.buf[k] = self.buf[i];
                k += 1;
            } else {
                let l = self.buf[s].ch as u32;
                let v = self.buf[i].ch as u32;
                if (JAMO_L_BASE..JAMO_L_END).contains(&l) && (JAMO_V_BASE..JAMO_V_END).contains(&v)
                {
                    self.assign_rune(
                        s,
                        HANGUL_BASE
                            + (l - JAMO_L_BASE) * JAMO_VT_COUNT
                            + (v - JAMO_V_BASE) * JAMO_T_COUNT,
                    );
                } else if (HANGUL_BASE..HANGUL_END).contains(&l)
                    && v > JAMO_T_BASE
                    && v < JAMO_T_END
                    && (l - HANGUL_BASE).is_multiple_of(JAMO_T_COUNT)
                {
                    self.assign_rune(s, l + v - JAMO_T_BASE);
                } else {
                    self.buf[k] = self.buf[i];
                    k += 1;
                }
            }
        }
        self.buf.truncate(k);
    }

    /// Ports `reorderBuffer.compose` (`composition.go:463`).
    fn compose(&mut self) {
        let bn = self.buf.len();
        if bn == 0 {
            return;
        }
        let mut k = 1;
        let mut s = 0;
        for i in 1..bn {
            // isJamoVT (`composition.go:381`): any rune in U+1100..U+11FF.
            if (0x1100..=0x11FF).contains(&(self.buf[i].ch as u32)) {
                self.combine_hangul(s, i, k);
                return;
            }
            let ii = self.buf[i];
            if ii.cb {
                let ccc_b = self.buf[k - 1].ccc;
                let mut blocked = false;
                if ccc_b == 0 {
                    s = k - 1;
                } else {
                    blocked = s != k - 1 && ccc_b >= ii.ccc;
                }
                if !blocked {
                    let combined = combine(self.buf[s].ch, ii.ch);
                    if combined != 0 {
                        self.assign_rune(s, combined);
                        continue;
                    }
                }
            }
            self.buf[k] = ii;
            k += 1;
        }
        self.buf.truncate(k);
    }

    /// Ports `decomposeSegment` (`normalize.go:504`) with `atEOF` true.
    fn decompose_segment(&mut self, mut sp: usize) -> usize {
        let nsrc = self.src.len();
        let (ch, p, size) = info(self.src, sp);
        let (mut ch, mut p, mut size) = (ch, p, size);
        match ss_next(&mut self.ss, &p) {
            SsState::Starter => {
                if !self.buf.is_empty() {
                    self.do_flush();
                    return sp;
                }
            }
            SsState::Overflow => {
                self.insert_cgj();
                self.do_flush();
                return sp;
            }
            SsState::Success => {}
        }
        self.insert_flush(ch, &p);
        loop {
            sp += size;
            if sp >= nsrc {
                break;
            }
            (ch, p, size) = info(self.src, sp);
            match ss_next(&mut self.ss, &p) {
                SsState::Starter => break,
                SsState::Overflow => {
                    self.insert_cgj();
                    break;
                }
                SsState::Success => {}
            }
            self.insert_flush(ch, &p);
        }
        self.do_flush();
        sp
    }

    /// Ports `appendQuick` (`normalize.go:182`).
    fn append_quick(&mut self, i: usize) -> usize {
        if i == self.src.len() {
            return i;
        }
        let (end, _) = quick_span(self.src, i, self.src.len(), self.composing);
        self.out.push_str(&self.src[i..end]);
        end
    }
}

/// Ports `combine` (`forminfo.go:206`): both runes are truncated to 16 bits.
fn combine(a: char, b: char) -> u32 {
    let key = ((a as u32 & 0xFFFF) << 16) | (b as u32 & 0xFFFF);
    match RECOMP.binary_search_by_key(&key, |&(k, _)| k) {
        Ok(i) => RECOMP[i].1,
        Err(_) => 0,
    }
}

/// Ports `Form.String` (`normalize.go:59`) and `doAppendInner` (`normalize.go:251`).
fn string(s: &str, composing: bool) -> String {
    let (n, ok) = quick_span(s, 0, s.len(), composing);
    if ok {
        return s.to_string();
    }
    let mut rb = ReorderBuffer {
        composing,
        src: s,
        out: s[..n].to_string(),
        ss: 0,
        buf: Vec::with_capacity(32),
    };
    let mut p = n;
    while p < s.len() {
        p = rb.decompose_segment(p);
        p = rb.append_quick(p);
    }
    rb.out
}
