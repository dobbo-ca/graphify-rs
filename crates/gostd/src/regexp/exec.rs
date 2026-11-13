//! Port of Go 1.27.4 regexp/exec.go (the NFA "pike" machine, leftmost-first) and the byte-slice
//! `input` of regexp.go:365-465. Go's `-1` "end of text" rune is `None`; positions that Go keeps
//! in `[]int` with -1 for "unset" are `isize` here.
//!
//! The backtrack and onepass engines are not ported, so every program runs on this machine, and
//! `longest` is always false (`Compile`, not `CompilePOSIX`).

use super::Regexp;
use super::syntax::{EmptyOp, InstOp, Prog, is_word_char};
use crate::utf8::{decode_last_rune, decode_rune};

/// Go `inputBytes.step` (regexp.go:421): the rune at `pos` and its width; `(None, 0)` at the end.
/// An invalid byte is `(U+FFFD, 1)`.
pub(super) fn step(b: &[u8], pos: usize) -> (Option<char>, usize) {
    if pos < b.len() {
        let (r, w) = decode_rune(&b[pos..]);
        return (Some(r), w);
    }
    (None, 0)
}

/// Go `inputBytes.context` (regexp.go:440): the runes before and after `pos` as a lazy flag.
fn context(b: &[u8], pos: usize) -> LazyFlag {
    let mut r1 = None;
    let mut r2 = None;
    // 0 < pos && pos <= len
    if pos > 0 && pos <= b.len() {
        r1 = Some(decode_last_rune(&b[..pos]).0);
    }
    // 0 <= pos && pos < len
    if pos < b.len() {
        r2 = Some(decode_rune(&b[pos..]).0);
    }
    LazyFlag(r1, r2)
}

/// Go `inputBytes.index` (regexp.go:436): `bytes.Index(b[pos:], prefix)`.
fn index(b: &[u8], prefix: &[u8], pos: usize) -> Option<usize> {
    b[pos..].windows(prefix.len()).position(|w| w == prefix)
}

/// Go `lazyFlag` (exec.go:122): the runes around a position, from which the empty-width flags
/// (`^ $ \A \z \B \b`) are computed only if needed.
#[derive(Clone, Copy)]
struct LazyFlag(Option<char>, Option<char>);

impl LazyFlag {
    /// Go `lazyFlag.match` (exec.go:128).
    fn matches(self, op: EmptyOp) -> bool {
        let mut op = op.0;
        if op == 0 {
            return true;
        }
        let r1 = self.0;
        if op & EmptyOp::BEGIN_LINE.0 != 0 {
            if r1 != Some('\n') && r1.is_some() {
                return false;
            }
            op &= !EmptyOp::BEGIN_LINE.0;
        }
        if op & EmptyOp::BEGIN_TEXT.0 != 0 {
            if r1.is_some() {
                return false;
            }
            op &= !EmptyOp::BEGIN_TEXT.0;
        }
        if op == 0 {
            return true;
        }
        let r2 = self.1;
        if op & EmptyOp::END_LINE.0 != 0 {
            if r2 != Some('\n') && r2.is_some() {
                return false;
            }
            op &= !EmptyOp::END_LINE.0;
        }
        if op & EmptyOp::END_TEXT.0 != 0 {
            if r2.is_some() {
                return false;
            }
            op &= !EmptyOp::END_TEXT.0;
        }
        if op == 0 {
            return true;
        }
        if r1.is_some_and(is_word_char) != r2.is_some_and(is_word_char) {
            op &= !EmptyOp::WORD_BOUNDARY.0;
        } else {
            op &= !EmptyOp::NO_WORD_BOUNDARY.0;
        }
        op == 0
    }
}

/// Go `entry` and `thread` (exec.go:21,30): an instruction pc and, for a live thread, its capture
/// array. A placeholder entry (Go `t == nil`) has `cap == None`.
struct Entry {
    pc: u32,
    cap: Option<Vec<isize>>,
}

/// Go `queue` (exec.go:14): a sparse set of pending threads.
struct Queue {
    sparse: Vec<u32>,
    dense: Vec<Entry>,
}

impl Queue {
    fn new(n: usize) -> Queue {
        Queue {
            sparse: vec![0; n],
            dense: Vec::new(),
        }
    }
}

/// Go `machine` (exec.go:41) for one search. `pool` holds spare capture arrays (Go `m.pool`);
/// `jobs` is the work stack of [`add`], kept to reuse its allocation.
struct Machine<'a> {
    re: &'a Regexp,
    pool: Vec<Vec<isize>>,
    jobs: Vec<Job>,
    matched: bool,
    matchcap: Vec<isize>,
}

/// Go `Regexp.find`'s machine path (exec.go:521) and `machine.match` (exec.go:175): the leftmost-first
/// match of `re` in `b` starting the search at `pos`, as `ncap` capture positions (-1 = unset);
/// `None` if there is no match.
pub(super) fn find(re: &Regexp, b: &[u8], pos: usize, ncap: usize) -> Option<Vec<isize>> {
    let mut m = Machine {
        re,
        pool: Vec::new(),
        jobs: Vec::new(),
        matched: false,
        matchcap: vec![-1; ncap],
    };
    let matched = m.run(b, pos);
    matched.then_some(m.matchcap)
}

impl Machine<'_> {
    /// Go `machine.match` (exec.go:175): reports whether a match was found; if so `matchcap`
    /// holds the submatch information.
    fn run(&mut self, b: &[u8], mut pos: usize) -> bool {
        let re = self.re;
        let start_cond = re.cond;
        if start_cond == EmptyOp(!0) {
            // impossible
            return false;
        }
        let n = re.prog.inst.len();
        let (mut runq, mut nextq) = (Queue::new(n), Queue::new(n));
        let (mut r, mut width) = step(b, pos);
        let (mut r1, mut width1) = (None, 0);
        if r.is_some() {
            (r1, width1) = step(b, pos + width);
        }
        let mut flag = if pos == 0 {
            LazyFlag(None, r)
        } else {
            context(b, pos)
        };
        loop {
            if runq.dense.is_empty() {
                if start_cond.0 & EmptyOp::BEGIN_TEXT.0 != 0 && pos != 0 {
                    // Anchored match, past beginning of text.
                    break;
                }
                if self.matched {
                    // Have match; finished exploring alternatives.
                    break;
                }
                if !re.prefix.is_empty() && r1 != re.prefix_rune {
                    // Match requires literal prefix; fast search for it.
                    let Some(advance) = index(b, re.prefix.as_bytes(), pos) else {
                        break;
                    };
                    pos += advance;
                    (r, width) = step(b, pos);
                    (r1, width1) = step(b, pos + width);
                }
            }
            if !self.matched {
                if !self.matchcap.is_empty() {
                    self.matchcap[0] = pos as isize;
                }
                add(
                    &re.prog,
                    &mut self.pool,
                    &mut runq,
                    re.prog.start as u32,
                    pos,
                    &mut self.matchcap,
                    flag,
                    &mut self.jobs,
                );
            }
            flag = LazyFlag(r, r1);
            self.step(&mut runq, &mut nextq, pos, pos + width, r, flag);
            if width == 0 {
                break;
            }
            if self.matchcap.is_empty() && self.matched {
                // Found a match and not paying attention to where it is, so any match will do.
                break;
            }
            pos += width;
            (r, width) = (r1, width1);
            if r.is_some() {
                (r1, width1) = step(b, pos + width);
            }
            std::mem::swap(&mut runq, &mut nextq);
        }
        self.matched
    }

    /// Go `machine.step` (exec.go:260), first-match mode: runs each thread on `runq` over the
    /// rune `c` (`None` at the end of text), which starts at `pos` and ends at `next_pos`, and
    /// appends new threads to `nextq`. `next_cond` is the empty-width flag after `c`.
    fn step(
        &mut self,
        runq: &mut Queue,
        nextq: &mut Queue,
        pos: usize,
        next_pos: usize,
        c: Option<char>,
        next_cond: LazyFlag,
    ) {
        let re = self.re;
        let prog = &re.prog;
        let mut j = 0;
        while j < runq.dense.len() {
            let Some(mut cap) = runq.dense[j].cap.take() else {
                j += 1;
                continue;
            };
            let i = &prog.inst[runq.dense[j].pc as usize];
            let add_thread = match i.op {
                InstOp::Match => {
                    if !cap.is_empty() {
                        cap[1] = pos as isize;
                        self.matchcap.copy_from_slice(&cap);
                    }
                    // First-match mode: cut off all lower-priority threads.
                    for d in runq.dense.drain(j + 1..) {
                        self.pool.extend(d.cap);
                    }
                    runq.dense.clear();
                    self.matched = true;
                    false
                }
                InstOp::Rune => c.is_some_and(|c| i.match_rune(c)),
                InstOp::Rune1 => c.is_some_and(|c| c as u32 == i.rune[0]),
                InstOp::RuneAny => true,
                InstOp::RuneAnyNotNL => c != Some('\n'),
                _ => panic!("bad inst"),
            };
            if add_thread {
                add(
                    prog,
                    &mut self.pool,
                    nextq,
                    i.out,
                    next_pos,
                    &mut cap,
                    next_cond,
                    &mut self.jobs,
                );
            }
            self.pool.push(cap);
            j += 1;
        }
        runq.dense.clear();
    }
}

/// A pending step of [`add`]: follow `pc`, or put back a capture slot that a `Capture` changed.
enum Job {
    Pc(u32),
    Restore(usize, isize),
}

/// Go `machine.add` (exec.go:317): adds an entry to `q` for `pc`, unless `q` already has one, and
/// for every instruction reachable from `pc` by following empty-width conditions satisfied by
/// `cond`. `pos` is the current position in the input; a thread gets a copy of `cap` (Go reuses
/// the stepping thread for the first leaf, which is only an allocation saving).
///
/// Go recurses on `Alt` (into `out`, then `arg`) and on `Capture`, on a goroutine stack that grows
/// to 1 GB; a chain of empty-matching copies can be tens of thousands deep (graphify-rs-aa6.23).
/// Here the recursion is the explicit stack `jobs`: `Alt` pushes `arg` and goes on with `out`,
/// and `Capture` pushes the restore of its slot and goes on with `out`. Everything the `out` walk
/// pushes is popped before those jobs, so entries are added in Go's order and each slot is put
/// back exactly when Go's recursive call returns.
#[allow(clippy::too_many_arguments)]
fn add(
    prog: &Prog,
    pool: &mut Vec<Vec<isize>>,
    q: &mut Queue,
    pc: u32,
    pos: usize,
    cap: &mut [isize],
    cond: LazyFlag,
    jobs: &mut Vec<Job>,
) {
    jobs.clear();
    jobs.push(Job::Pc(pc));
    while let Some(job) = jobs.pop() {
        let mut pc = match job {
            Job::Pc(pc) => pc,
            Job::Restore(arg, opos) => {
                cap[arg] = opos;
                continue;
            }
        };
        loop {
            if pc == 0 {
                break;
            }
            let j = q.sparse[pc as usize] as usize;
            if j < q.dense.len() && q.dense[j].pc == pc {
                break;
            }

            let j = q.dense.len();
            q.dense.push(Entry { pc, cap: None });
            q.sparse[pc as usize] = j as u32;

            let i = &prog.inst[pc as usize];
            match i.op {
                InstOp::Fail => break,
                InstOp::Alt | InstOp::AltMatch => {
                    jobs.push(Job::Pc(i.arg));
                    pc = i.out;
                }
                InstOp::EmptyWidth => {
                    if !cond.matches(EmptyOp(i.arg as u8)) {
                        break;
                    }
                    pc = i.out;
                }
                InstOp::Nop => pc = i.out,
                InstOp::Capture => {
                    let arg = i.arg as usize;
                    if arg < cap.len() {
                        jobs.push(Job::Restore(arg, cap[arg]));
                        cap[arg] = pos as isize;
                    }
                    pc = i.out;
                }
                InstOp::Match
                | InstOp::Rune
                | InstOp::Rune1
                | InstOp::RuneAny
                | InstOp::RuneAnyNotNL => {
                    let mut t = pool.pop().unwrap_or_default();
                    t.clear();
                    t.extend_from_slice(cap);
                    q.dense[j].cap = Some(t);
                    break;
                }
            }
        }
    }
}
