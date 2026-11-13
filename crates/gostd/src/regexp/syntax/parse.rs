//! Port of Go 1.27.4 regexp/syntax/parse.go: the parser, class helpers, Unicode class lookup.
//!
//! Go's parser works on `*Regexp` pointers: nodes are shared between the parse stack, `Sub`
//! slices and a free list (`p.free`), and the height and size limits cache their results in maps
//! keyed by pointer (`p.height`, `p.size`). Those caches and the free-list count (`numRegexp`)
//! decide when `ErrNestingDepth` and `ErrLarge` fire, so to stay byte-identical the parser here
//! keeps the same shape over an arena: a node pointer is an index into `Parser::nodes`, `p.free`
//! is `Parser::free`, `numRegexp` is `nodes.len()`, and the two maps are per-node fields. The
//! finished tree is converted to an owned `Regexp` at the end (`Parser::build_regexp`).
//!
//! The only items other files use are `parse`, `append_range`, `in_char_class` and `CharGroup`.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::perl_groups::{perl_group, posix_group};
use super::{Error, ErrorCode, Flags, MAX_FOLD, MIN_FOLD, Op, Regexp};
use crate::unicode::{
    CATEGORY_ALIASES, CN, Range16, Range32, RangeTable, SCRIPTS, category, fold_category,
    fold_script, script, simple_fold,
};

/// Go `unicode.MaxRune` (unicode/letter.go:13).
const MAX_RUNE: u32 = 0x10FFFF;

/// Go `maxHeight` (parse.go:94): maximum height of a regexp parse tree.
const MAX_HEIGHT: usize = 1000;

/// Go `instSize` (parse.go:104): `5 * 8` bytes.
const INST_SIZE: i64 = 5 * 8;

/// Go `maxSize` (parse.go:103): maximum size of a compiled regexp in Insts. Go's `<<` and `/`
/// share a precedence level, so this is `(128 << 20) / instSize`.
const MAX_SIZE: i64 = (128 << 20) / INST_SIZE;

/// Go `runeSize` (parse.go:123): a rune is an int32.
const RUNE_SIZE: usize = 4;

/// Go `maxRunes` (parse.go:122): maximum runes in a regexp tree, counting all nodes.
const MAX_RUNES: usize = (128 << 20) / RUNE_SIZE;

/// Go `charGroup` (parse.go:1575): `sign` is +1 or -1, `class` a range-pair list.
#[derive(Debug, Clone, Copy)]
pub(super) struct CharGroup {
    pub sign: i32,
    pub class: &'static [u32],
}

/// Go `anyTable` (parse.go:1638).
static ANY_TABLE: RangeTable = RangeTable {
    r16: &[Range16 {
        lo: 0,
        hi: 0xFFFF,
        stride: 1,
    }],
    r32: &[Range32 {
        lo: 0x10000,
        hi: MAX_RUNE,
        stride: 1,
    }],
    latin_offset: 0,
};

/// Go `asciiTable` (parse.go:1643).
static ASCII_TABLE: RangeTable = RangeTable {
    r16: &[Range16 {
        lo: 0,
        hi: 0x7F,
        stride: 1,
    }],
    r32: &[],
    latin_offset: 1,
};

/// Go `asciiFoldTable` (parse.go:1647): ASCII plus long s (U+017F) and Kelvin K (U+212A).
static ASCII_FOLD_TABLE: RangeTable = RangeTable {
    r16: &[
        Range16 {
            lo: 0,
            hi: 0x7F,
            stride: 1,
        },
        Range16 {
            lo: 0x017F,
            hi: 0x017F,
            stride: 1,
        },
        Range16 {
            lo: 0x212A,
            hi: 0x212A,
            stride: 1,
        },
    ],
    r32: &[],
    latin_offset: 1,
};

/// Whether `f` has any bit of `x` set (Go `f&x != 0`).
fn has(f: Flags, x: Flags) -> bool {
    (f & x) != Flags(0)
}

/// An `Error` for `code` on `expr`.
fn err(code: ErrorCode, expr: &str) -> Error {
    Error {
        code,
        expr: expr.to_string(),
    }
}

/// Go `unicode.SimpleFold` on a Go rune. A value that is not a `char` (a surrogate) has no case
/// orbit, so it folds to itself.
fn fold(r: u32) -> u32 {
    char::from_u32(r).map_or(r, |c| u32::from(simple_fold(c)))
}

/// A parse-tree node: Go `Regexp` plus the per-pointer caches the parser keeps in maps.
struct Node {
    op: Op,
    /// Go `opVerticalBar` when `op == Op::Pseudo`; otherwise Pseudo is Go `opLeftParen`.
    vbar: bool,
    flags: Flags,
    sub: Vec<usize>,
    rune: Vec<u32>,
    min: i32,
    max: i32,
    cap: i32,
    name: String,
    /// Go `p.height[re]`; 0 means absent.
    height: usize,
    /// Go `p.size[re]`. Go never deletes a size entry, so `new_node` keeps this across reuse.
    size: Option<i64>,
}

impl Node {
    fn new(op: Op) -> Node {
        Node {
            op,
            vbar: false,
            flags: Flags(0),
            sub: Vec::new(),
            rune: Vec::new(),
            min: 0,
            max: 0,
            cap: 0,
            name: String::new(),
            height: 0,
            size: None,
        }
    }
}

/// Go `parser` (parse.go:127).
struct Parser<'a> {
    /// Parse mode flags.
    flags: Flags,
    /// Stack of parsed expressions (node indexes).
    stack: Vec<usize>,
    /// Every node ever allocated; `nodes.len()` is Go `numRegexp`.
    nodes: Vec<Node>,
    /// Go `p.free`: nodes passed to `reuse`, most recent last.
    free: Vec<usize>,
    /// Number of capturing groups seen.
    num_cap: i32,
    whole_regexp: &'a str,
    /// Number of runes in char classes.
    num_runes: usize,
    /// Product of all repetitions seen.
    repeats: i64,
    /// Go `p.height != nil`.
    height_on: bool,
    /// Go `p.size != nil`.
    size_on: bool,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str, flags: Flags) -> Parser<'a> {
        Parser {
            flags,
            stack: Vec::new(),
            nodes: Vec::new(),
            free: Vec::new(),
            num_cap: 0,
            whole_regexp: s,
            num_runes: 0,
            repeats: 0,
            height_on: false,
            size_on: false,
        }
    }

    /// Go's `panic(code)` caught in `parse` (parse.go:892): an `Error` on the whole expression.
    fn limit_err(&self, code: ErrorCode) -> Error {
        err(code, self.whole_regexp)
    }

    /// Go `parser.newRegexp` (parse.go:141).
    fn new_node(&mut self, op: Op) -> usize {
        if let Some(id) = self.free.pop() {
            let size = self.nodes[id].size;
            self.nodes[id] = Node::new(op);
            self.nodes[id].size = size;
            id
        } else {
            self.nodes.push(Node::new(op));
            self.nodes.len() - 1
        }
    }

    /// Go `parser.reuse` (parse.go:154).
    fn reuse(&mut self, id: usize) {
        self.nodes[id].height = 0;
        self.free.push(id);
    }

    /// Go `parser.checkLimits` (parse.go:162).
    fn check_limits(&mut self, re: usize) -> Result<(), Error> {
        if self.num_runes > MAX_RUNES {
            return Err(self.limit_err(ErrorCode::Large));
        }
        self.check_size(re)?;
        self.check_height(re)
    }

    /// Go `parser.checkSize` (parse.go:170).
    fn check_size(&mut self, re: usize) -> Result<(), Error> {
        if !self.size_on {
            // We haven't started tracking size yet.
            // Do a relatively cheap check to see if we need to start.
            if self.repeats == 0 {
                self.repeats = 1;
            }
            if self.nodes[re].op == Op::Repeat {
                let mut n = self.nodes[re].max;
                if n == -1 {
                    n = self.nodes[re].min;
                }
                if n <= 0 {
                    n = 1;
                }
                if i64::from(n) > MAX_SIZE / self.repeats {
                    self.repeats = MAX_SIZE;
                } else {
                    self.repeats *= i64::from(n);
                }
            }
            if (self.nodes.len() as i64) < MAX_SIZE / self.repeats {
                return Ok(());
            }

            // We need to start tracking size.
            // Belatedly populate the cache with everything constructed so far.
            self.size_on = true;
            for stacked in self.stack.clone() {
                self.check_size(stacked)?;
            }
        }

        if self.calc_size(re, true) > MAX_SIZE {
            return Err(self.limit_err(ErrorCode::Large));
        }
        Ok(())
    }

    /// Go `parser.calcSize` (parse.go:212).
    fn calc_size(&mut self, re: usize, force: bool) -> i64 {
        if !force && let Some(size) = self.nodes[re].size {
            return size;
        }

        let (op, sub, nrunes, min, max) = {
            let n = &self.nodes[re];
            (n.op, n.sub.clone(), n.rune.len(), n.min, n.max)
        };
        let size: i64 = match op {
            Op::Literal => nrunes as i64,
            // star can be 1+ or 2+; assume 2 pessimistically
            Op::Capture | Op::Star => 2 + self.calc_size(sub[0], false),
            Op::Plus | Op::Quest => 1 + self.calc_size(sub[0], false),
            Op::Concat => sub.iter().map(|&s| self.calc_size(s, false)).sum(),
            Op::Alternate => {
                let mut size: i64 = sub.iter().map(|&s| self.calc_size(s, false)).sum();
                if sub.len() > 1 {
                    size += sub.len() as i64 - 1;
                }
                size
            }
            Op::Repeat => {
                let sub = self.calc_size(sub[0], false);
                if max == -1 {
                    if min == 0 {
                        2 + sub // x*
                    } else {
                        1 + i64::from(min) * sub // xxx+
                    }
                } else {
                    // x{2,5} = xx(x(x(x)?)?)?
                    i64::from(max) * sub + i64::from(max - min)
                }
            }
            _ => 0,
        };

        let size = size.max(1);
        self.nodes[re].size = Some(size);
        size
    }

    /// Go `parser.checkHeight` (parse.go:258).
    fn check_height(&mut self, re: usize) -> Result<(), Error> {
        if self.nodes.len() < MAX_HEIGHT {
            return Ok(());
        }
        if !self.height_on {
            self.height_on = true;
            for stacked in self.stack.clone() {
                self.check_height(stacked)?;
            }
        }
        if self.calc_height(re, true) > MAX_HEIGHT {
            return Err(self.limit_err(ErrorCode::NestingDepth));
        }
        Ok(())
    }

    /// Go `parser.calcHeight` (parse.go:273).
    fn calc_height(&mut self, re: usize, force: bool) -> usize {
        if !force && self.nodes[re].height != 0 {
            return self.nodes[re].height;
        }
        let mut h = 1;
        for sub in self.nodes[re].sub.clone() {
            let hsub = self.calc_height(sub, false);
            if h < 1 + hsub {
                h = 1 + hsub;
            }
        }
        self.nodes[re].height = h;
        h
    }

    /// Go `parser.push` (parse.go:293): pushes the regexp `id` onto the parse stack. Go returns
    /// the regexp, or nil when `maybeConcat` consumed it; every caller ignores that value.
    fn push(&mut self, id: usize) -> Result<(), Error> {
        self.num_runes += self.nodes[id].rune.len();
        let n = &self.nodes[id];
        let rune = &n.rune;
        let is_class = n.op == Op::CharClass;
        let single = is_class && rune.len() == 2 && rune[0] == rune[1];
        // Case-insensitive rune like [Aa] or [Δδ].
        let folded = is_class
            && (rune.len() == 4
                && rune[0] == rune[1]
                && rune[2] == rune[3]
                && fold(rune[0]) == rune[2]
                && fold(rune[2]) == rune[0]
                || rune.len() == 2
                    && rune[0] + 1 == rune[1]
                    && fold(rune[0]) == rune[1]
                    && fold(rune[1]) == rune[0]);
        let r0 = rune.first().copied().unwrap_or(0);

        if single {
            // Single rune.
            let flags = self.flags & !Flags::FOLD_CASE;
            if self.maybe_concat(Some(r0), flags) {
                return Ok(());
            }
            let n = &mut self.nodes[id];
            n.op = Op::Literal;
            n.rune.truncate(1);
            n.flags = flags;
        } else if folded {
            let flags = self.flags | Flags::FOLD_CASE;
            if self.maybe_concat(Some(r0), flags) {
                return Ok(());
            }
            // Rewrite as (case-insensitive) literal.
            let n = &mut self.nodes[id];
            n.op = Op::Literal;
            n.rune.truncate(1);
            n.flags = flags;
        } else {
            // Incremental concatenation.
            self.maybe_concat(None, Flags(0));
        }

        self.stack.push(id);
        self.check_limits(id)
    }

    /// Go `parser.maybeConcat` (parse.go:339): incremental concatenation of literal runes into
    /// string nodes. `Some(r)` is Go's `r >= 0` (reuse the left-over node to push `r` with
    /// `flags`); `None` is Go's `-1`. Reports whether `r` was pushed.
    fn maybe_concat(&mut self, r: Option<u32>, flags: Flags) -> bool {
        let n = self.stack.len();
        if n < 2 {
            return false;
        }

        let re1 = self.stack[n - 1];
        let re2 = self.stack[n - 2];
        {
            let (a, b) = (&self.nodes[re1], &self.nodes[re2]);
            if a.op != Op::Literal
                || b.op != Op::Literal
                || (a.flags & Flags::FOLD_CASE) != (b.flags & Flags::FOLD_CASE)
            {
                return false;
            }
        }

        // Push re1 into re2.
        let moved = std::mem::take(&mut self.nodes[re1].rune);
        self.nodes[re2].rune.extend(moved);

        // Reuse re1 if possible.
        if let Some(r) = r {
            self.nodes[re1].rune = vec![r];
            self.nodes[re1].flags = flags;
            return true;
        }

        self.stack.pop();
        self.reuse(re1);
        false // did not push r
    }

    /// Go `parser.literal` (parse.go:368): pushes a literal regexp for the rune `r`.
    fn literal(&mut self, r: u32) -> Result<(), Error> {
        let id = self.new_node(Op::Literal);
        self.nodes[id].flags = self.flags;
        let r = if has(self.flags, Flags::FOLD_CASE) {
            min_fold_rune(r)
        } else {
            r
        };
        self.nodes[id].rune = vec![r];
        self.push(id)
    }

    /// Go `parser.op` (parse.go:394): pushes a regexp with the given op onto the stack and
    /// returns it. (Go returns `push`'s result, which is never nil for these ops.)
    fn op(&mut self, op: Op) -> Result<usize, Error> {
        let id = self.new_node(op);
        self.nodes[id].flags = self.flags;
        self.push(id)?;
        Ok(id)
    }

    /// Go `parser.repeat` (parse.go:404): replaces the top stack element with itself repeated
    /// according to op, min, max. Returns the updated `after`.
    fn repeat(
        &mut self,
        op: Op,
        min: i32,
        max: i32,
        before: &'a str,
        after: &'a str,
        last_repeat: &str,
    ) -> Result<&'a str, Error> {
        let mut flags = self.flags;
        let mut after = after;
        if has(self.flags, Flags::PERL_X) {
            if let Some(rest) = after.strip_prefix('?') {
                after = rest;
                flags = Flags(flags.0 ^ Flags::NON_GREEDY.0);
            }
            if !last_repeat.is_empty() {
                // In Perl it is not allowed to stack repetition operators:
                // a** is a syntax error, not a doubled star, and a++ means
                // something else entirely, which we don't support!
                return Err(err(
                    ErrorCode::InvalidRepeatOp,
                    &last_repeat[..last_repeat.len() - after.len()],
                ));
            }
        }
        let n = self.stack.len();
        if n == 0 {
            return Err(err(
                ErrorCode::MissingRepeatArgument,
                &before[..before.len() - after.len()],
            ));
        }
        let sub = self.stack[n - 1];
        if self.nodes[sub].op == Op::Pseudo {
            return Err(err(
                ErrorCode::MissingRepeatArgument,
                &before[..before.len() - after.len()],
            ));
        }

        let re = self.new_node(op);
        let node = &mut self.nodes[re];
        node.min = min;
        node.max = max;
        node.flags = flags;
        node.sub = vec![sub];
        self.stack[n - 1] = re;
        self.check_limits(re)?;

        if op == Op::Repeat && (min >= 2 || max >= 2) && !self.repeat_is_valid(re, 1000) {
            return Err(err(
                ErrorCode::InvalidRepeatSize,
                &before[..before.len() - after.len()],
            ));
        }

        Ok(after)
    }

    /// Go `repeatIsValid` (parse.go:452): whether the repetition `re` is valid, meaning that the
    /// combination of the top-level repetition and any inner repetitions does not exceed `n`
    /// copies of the innermost thing.
    fn repeat_is_valid(&self, re: usize, n: i32) -> bool {
        let node = &self.nodes[re];
        let mut n = n;
        if node.op == Op::Repeat {
            let mut m = node.max;
            if m == 0 {
                return true;
            }
            if m < 0 {
                m = node.min;
            }
            if m > n {
                return false;
            }
            if m > 0 {
                n /= m;
            }
        }
        node.sub.iter().all(|&sub| self.repeat_is_valid(sub, n))
    }

    /// Go `parser.concat` (parse.go:477): replaces the top of the stack (above the topmost '|'
    /// or '(') with its concatenation.
    fn concat(&mut self) -> Result<(), Error> {
        self.maybe_concat(None, Flags(0));

        // Scan down to find pseudo-operator | or (.
        let mut i = self.stack.len();
        while i > 0 && self.nodes[self.stack[i - 1]].op != Op::Pseudo {
            i -= 1;
        }
        let subs = self.stack.split_off(i);

        // Empty concatenation is special case.
        if subs.is_empty() {
            let id = self.new_node(Op::EmptyMatch);
            return self.push(id);
        }

        let id = self.collapse(&subs, Op::Concat)?;
        self.push(id)
    }

    /// Go `parser.alternate` (parse.go:497): replaces the top of the stack (above the topmost
    /// '(') with its alternation.
    fn alternate(&mut self) -> Result<(), Error> {
        // Scan down to find pseudo-operator (.
        // There are no | above (.
        let mut i = self.stack.len();
        while i > 0 && self.nodes[self.stack[i - 1]].op != Op::Pseudo {
            i -= 1;
        }
        let subs = self.stack.split_off(i);

        // Make sure top class is clean.
        // All the others already are (see swapVerticalBar).
        if let Some(&last) = subs.last() {
            self.clean_alt(last);
        }

        // Empty alternate is special case
        // (shouldn't happen but easy to handle).
        if subs.is_empty() {
            let id = self.new_node(Op::NoMatch);
            return self.push(id);
        }

        let id = self.collapse(&subs, Op::Alternate)?;
        self.push(id)
    }

    /// Go `cleanAlt` (parse.go:523): cleans `re` for eventual inclusion in an alternation. Go's
    /// trailing capacity trim (`cap-len > 100`) has no effect on the value and is omitted.
    fn clean_alt(&mut self, re: usize) {
        let n = &mut self.nodes[re];
        if n.op == Op::CharClass {
            n.rune = clean_class(&n.rune);
            if n.rune == [0, MAX_RUNE] {
                n.rune = Vec::new();
                n.op = Op::AnyChar;
                return;
            }
            if n.rune == [0, '\n' as u32 - 1, '\n' as u32 + 1, MAX_RUNE] {
                n.rune = Vec::new();
                n.op = Op::AnyCharNotNL;
            }
        }
    }

    /// Go `parser.collapse` (parse.go:549): the result of applying `op` to `subs`. If a sub
    /// contains `op` nodes, they all get hoisted up so that there is never a concat of a concat
    /// or an alternate of an alternate.
    fn collapse(&mut self, subs: &[usize], op: Op) -> Result<usize, Error> {
        if subs.len() == 1 {
            return Ok(subs[0]);
        }
        let mut re = self.new_node(op);
        let mut list = Vec::new();
        for &sub in subs {
            if self.nodes[sub].op == op {
                list.extend_from_slice(&self.nodes[sub].sub);
                self.reuse(sub);
            } else {
                list.push(sub);
            }
        }
        if op == Op::Alternate {
            list = self.factor(list)?;
            if list.len() == 1 {
                let old = re;
                re = list[0];
                self.reuse(old);
                return Ok(re);
            }
        }
        self.nodes[re].sub = list;
        Ok(re)
    }

    /// Go `parser.factor` (parse.go:589): factors common prefixes from the alternation list
    /// `sub`. Returns a replacement list and frees (passes to `reuse`) any removed regexps.
    ///
    /// For example,
    ///
    ///   ABC|ABD|AEF|BCX|BCY
    ///
    /// simplifies by literal prefix extraction to
    ///
    ///   A(B(C|D)|EF)|BC(X|Y)
    ///
    /// which simplifies by character class introduction to
    ///
    ///   A(B[CD]|EF)|BC[XY]
    fn factor(&mut self, mut sub: Vec<usize>) -> Result<Vec<usize>, Error> {
        if sub.len() < 2 {
            return Ok(sub);
        }

        // Round 1: Factor out common literal prefixes.
        let mut str_: Vec<u32> = Vec::new();
        let mut strflags = Flags(0);
        let mut start = 0;
        let mut out = Vec::new();
        for i in 0..=sub.len() {
            // Invariant: the regexps that were in sub[0..start] have been used or marked for
            // reuse. Invariant: sub[start..i] consists of regexps that all begin with str as
            // modified by strflags.
            let mut istr: Vec<u32> = Vec::new();
            let mut iflags = Flags(0);
            if i < sub.len() {
                (istr, iflags) = self.leading_string(sub[i]);
                if iflags == strflags {
                    let mut same = 0;
                    while same < str_.len() && same < istr.len() && str_[same] == istr[same] {
                        same += 1;
                    }
                    if same > 0 {
                        // Matches at least one rune in current range.
                        // Keep going around.
                        str_.truncate(same);
                        continue;
                    }
                }
            }

            // Found end of a run with common leading literal string:
            // sub[start..i] all begin with str, but sub[i] does not even begin with str[0].
            //
            // Factor out common string and append factored expression to out.
            if i == start {
                // Nothing to do - run of length 0.
            } else if i == start + 1 {
                // Just one: don't bother factoring.
                out.push(sub[start]);
            } else {
                // Construct factored form: prefix(suffix1|suffix2|...)
                let prefix = self.new_node(Op::Literal);
                self.nodes[prefix].flags = strflags;
                self.nodes[prefix].rune = str_.clone();

                for s in &mut sub[start..i] {
                    *s = self.remove_leading_string(*s, str_.len());
                    self.check_limits(*s)?;
                }
                let suffix = self.collapse(&sub[start..i], Op::Alternate)?; // recurse

                let re = self.new_node(Op::Concat);
                self.nodes[re].sub = vec![prefix, suffix];
                out.push(re);
            }

            // Prepare for next iteration.
            start = i;
            str_ = istr;
            strflags = iflags;
        }
        sub = out;

        // Round 2: Factor out common simple prefixes, just the first piece of each
        // concatenation. This will be good enough a lot of the time.
        //
        // Complex subexpressions (e.g. involving quantifiers) are not safe to factor because
        // that collapses their distinct paths through the automaton, which affects correctness
        // in some cases.
        start = 0;
        out = Vec::new();
        let mut first: Option<usize> = None;
        for i in 0..=sub.len() {
            // Invariant: sub[start..i] consists of regexps that all begin with ifirst.
            let mut ifirst: Option<usize> = None;
            if i < sub.len() {
                ifirst = self.leading_regexp(sub[i]);
                if let (Some(f), Some(g)) = (first, ifirst)
                    && self.equal(f, g)
                    // first must be a character class OR a fixed repeat of a character class.
                    && (self.is_char_class(f)
                        || (self.nodes[f].op == Op::Repeat
                            && self.nodes[f].min == self.nodes[f].max
                            && self.is_char_class(self.nodes[f].sub[0])))
                {
                    continue;
                }
            }

            // Found end of a run with common leading regexp:
            // sub[start..i] all begin with first but sub[i] does not.
            //
            // Factor out common regexp and append factored expression to out.
            if i == start {
                // Nothing to do - run of length 0.
            } else if i == start + 1 {
                // Just one: don't bother factoring.
                out.push(sub[start]);
            } else if let Some(prefix) = first {
                // Construct factored form: prefix(suffix1|suffix2|...)
                for (j, s) in sub[start..i].iter_mut().enumerate() {
                    let reuse = j != 0; // prefix came from sub[start]
                    *s = self.remove_leading_regexp(*s, reuse);
                    self.check_limits(*s)?;
                }
                let suffix = self.collapse(&sub[start..i], Op::Alternate)?; // recurse

                let re = self.new_node(Op::Concat);
                self.nodes[re].sub = vec![prefix, suffix];
                out.push(re);
            }

            // Prepare for next iteration.
            start = i;
            first = ifirst;
        }
        sub = out;

        // Round 3: Collapse runs of single literals into character classes.
        start = 0;
        out = Vec::new();
        for i in 0..=sub.len() {
            // Invariant: sub[start..i] consists of regexps that are either literal runes or
            // character classes.
            if i < sub.len() && self.is_char_class(sub[i]) {
                continue;
            }

            // sub[i] is not a char or char class;
            // emit char class for sub[start..i]...
            if i == start {
                // Nothing to do - run of length 0.
            } else if i == start + 1 {
                out.push(sub[start]);
            } else {
                // Make new char class.
                // Start with most complex regexp in sub[start].
                let mut max = start;
                for (j, &s) in sub.iter().enumerate().take(i).skip(start + 1) {
                    let (a, b) = (&self.nodes[sub[max]], &self.nodes[s]);
                    if (a.op as u8) < (b.op as u8) || a.op == b.op && a.rune.len() < b.rune.len() {
                        max = j;
                    }
                }
                sub.swap(start, max);

                let dst = sub[start];
                for &s in &sub[start + 1..i] {
                    self.merge_char_class(dst, s);
                    self.reuse(s);
                }
                self.clean_alt(dst);
                out.push(dst);
            }

            // ... and then emit sub[i].
            if i < sub.len() {
                out.push(sub[i]);
            }
            start = i + 1;
        }
        sub = out;

        // Round 4: Collapse runs of empty matches into a single empty match.
        out = Vec::new();
        for (i, &s) in sub.iter().enumerate() {
            if self.nodes[s].op == Op::EmptyMatch
                && sub
                    .get(i + 1)
                    .is_some_and(|&n| self.nodes[n].op == Op::EmptyMatch)
            {
                continue;
            }
            out.push(s);
        }

        Ok(out)
    }

    /// Go `Regexp.Equal` (regexp.go:64) on two nodes. `parse.rs` compares arena nodes here
    /// because `regexp::equal` takes an owned `Regexp` tree.
    fn equal(&self, x: usize, y: usize) -> bool {
        let (a, b) = (&self.nodes[x], &self.nodes[y]);
        if a.op != b.op {
            return false;
        }
        match a.op {
            Op::EndText if (a.flags & Flags::WAS_DOLLAR) != (b.flags & Flags::WAS_DOLLAR) => {
                return false;
            }
            Op::Literal | Op::CharClass => {
                return (a.flags & Flags::FOLD_CASE) == (b.flags & Flags::FOLD_CASE)
                    && a.rune == b.rune;
            }
            Op::Alternate | Op::Concat => {
                return a.sub.len() == b.sub.len()
                    && a.sub.iter().zip(&b.sub).all(|(&p, &q)| self.equal(p, q));
            }
            Op::Star | Op::Plus | Op::Quest
                if (a.flags & Flags::NON_GREEDY) != (b.flags & Flags::NON_GREEDY)
                    || !self.equal(a.sub[0], b.sub[0]) =>
            {
                return false;
            }
            Op::Repeat
                if (a.flags & Flags::NON_GREEDY) != (b.flags & Flags::NON_GREEDY)
                    || a.min != b.min
                    || a.max != b.max
                    || !self.equal(a.sub[0], b.sub[0]) =>
            {
                return false;
            }
            Op::Capture
                if a.cap != b.cap || a.name != b.name || !self.equal(a.sub[0], b.sub[0]) =>
            {
                return false;
            }
            _ => {}
        }
        true
    }

    /// Go `parser.leadingString` (parse.go:778): the leading literal string that `re` begins
    /// with, and its fold flag. Copies the runes; Go aliases them.
    fn leading_string(&self, re: usize) -> (Vec<u32>, Flags) {
        let mut re = &self.nodes[re];
        if re.op == Op::Concat && !re.sub.is_empty() {
            re = &self.nodes[re.sub[0]];
        }
        if re.op != Op::Literal {
            return (Vec::new(), Flags(0));
        }
        (re.rune.clone(), re.flags & Flags::FOLD_CASE)
    }

    /// Go `parser.removeLeadingString` (parse.go:790): removes the first `n` leading runes from
    /// the beginning of `re`. Returns the replacement for `re`.
    fn remove_leading_string(&mut self, re: usize, n: usize) -> usize {
        if self.nodes[re].op == Op::Concat && !self.nodes[re].sub.is_empty() {
            // Removing a leading string in a concatenation
            // might simplify the concatenation.
            let sub = self.nodes[re].sub[0];
            let sub = self.remove_leading_string(sub, n);
            self.nodes[re].sub[0] = sub;
            if self.nodes[sub].op == Op::EmptyMatch {
                self.reuse(sub);
                match self.nodes[re].sub.len() {
                    0 | 1 => {
                        // Impossible but handle.
                        self.nodes[re].op = Op::EmptyMatch;
                        self.nodes[re].sub = Vec::new();
                    }
                    2 => {
                        let old = re;
                        let re = self.nodes[old].sub[1];
                        self.reuse(old);
                        return re;
                    }
                    _ => {
                        self.nodes[re].sub.remove(0);
                    }
                }
            }
            return re;
        }

        if self.nodes[re].op == Op::Literal {
            let rune = &mut self.nodes[re].rune;
            rune.copy_within(n.., 0);
            rune.truncate(rune.len() - n);
            if self.nodes[re].rune.is_empty() {
                self.nodes[re].op = Op::EmptyMatch;
            }
        }
        re
    }

    /// Go `parser.leadingRegexp` (parse.go:827): the leading regexp that `re` begins with.
    fn leading_regexp(&self, re: usize) -> Option<usize> {
        let node = &self.nodes[re];
        if node.op == Op::EmptyMatch {
            return None;
        }
        if node.op == Op::Concat && !node.sub.is_empty() {
            let sub = node.sub[0];
            if self.nodes[sub].op == Op::EmptyMatch {
                return None;
            }
            return Some(sub);
        }
        Some(re)
    }

    /// Go `parser.removeLeadingRegexp` (parse.go:844): removes the leading regexp in `re`.
    /// Returns the replacement for `re`. If `reuse` is true, passes the removed regexp (if no
    /// longer needed) to `reuse`.
    fn remove_leading_regexp(&mut self, re: usize, reuse: bool) -> usize {
        if self.nodes[re].op == Op::Concat && !self.nodes[re].sub.is_empty() {
            if reuse {
                let first = self.nodes[re].sub[0];
                self.reuse(first);
            }
            self.nodes[re].sub.remove(0);
            match self.nodes[re].sub.len() {
                0 => {
                    self.nodes[re].op = Op::EmptyMatch;
                    self.nodes[re].sub = Vec::new();
                }
                1 => {
                    let old = re;
                    let re = self.nodes[old].sub[0];
                    self.reuse(old);
                    return re;
                }
                _ => {}
            }
            return re;
        }
        if reuse {
            self.reuse(re);
        }
        self.new_node(Op::EmptyMatch)
    }

    /// Go `isCharClass` (parse.go:1302): can `re` be represented as a character class? A
    /// single-rune literal string, char class, ., and .|\n.
    fn is_char_class(&self, re: usize) -> bool {
        let n = &self.nodes[re];
        n.op == Op::Literal && n.rune.len() == 1
            || n.op == Op::CharClass
            || n.op == Op::AnyCharNotNL
            || n.op == Op::AnyChar
    }

    /// Go `matchRune` (parse.go:1310): does `re` match `r`?
    fn match_rune(&self, re: usize, r: u32) -> bool {
        let n = &self.nodes[re];
        match n.op {
            Op::Literal => n.rune.len() == 1 && n.rune[0] == r,
            Op::CharClass => n
                .rune
                .as_chunks::<2>()
                .0
                .iter()
                .any(|&[lo, hi]| (lo..=hi).contains(&r)),
            Op::AnyCharNotNL => r != '\n' as u32,
            Op::AnyChar => true,
            _ => false,
        }
    }

    /// Go `mergeCharClass` (parse.go:1345): makes `dst = dst|src`. The caller must ensure that
    /// `dst.op >= src.op`, to reduce the amount of copying.
    fn merge_char_class(&mut self, dst: usize, src: usize) {
        match self.nodes[dst].op {
            Op::AnyChar => {
                // src doesn't add anything.
            }
            Op::AnyCharNotNL => {
                // src might add \n
                if self.match_rune(src, '\n' as u32) {
                    self.nodes[dst].op = Op::AnyChar;
                }
            }
            Op::CharClass => {
                // src is simpler, so either literal or char class
                let (src_op, src_rune, src_flags) = {
                    let s = &self.nodes[src];
                    (s.op, s.rune.clone(), s.flags)
                };
                let d = &mut self.nodes[dst];
                let rune = std::mem::take(&mut d.rune);
                d.rune = if src_op == Op::Literal {
                    append_literal(rune, src_rune[0], src_flags)
                } else {
                    append_class(rune, &src_rune)
                };
            }
            Op::Literal => {
                // both literal
                let (src_r0, src_flags) = (self.nodes[src].rune[0], self.nodes[src].flags);
                let d = &mut self.nodes[dst];
                if src_r0 == d.rune[0] && src_flags == d.flags {
                    return;
                }
                d.op = Op::CharClass;
                let (r0, flags) = (d.rune[0], d.flags);
                d.rune = append_literal(Vec::new(), r0, flags);
                d.rune = append_literal(std::mem::take(&mut d.rune), src_r0, src_flags);
            }
            _ => {}
        }
    }

    /// Go `parser.parseVerticalBar` (parse.go:1330): handles a | in the input.
    fn parse_vertical_bar(&mut self) -> Result<(), Error> {
        self.concat()?;

        // The concatenation we just parsed is on top of the stack.
        // If it sits above an opVerticalBar, swap it below
        // (things below an opVerticalBar become an alternation).
        // Otherwise, push a new vertical bar.
        if !self.swap_vertical_bar() {
            let id = self.op(Op::Pseudo)?;
            self.nodes[id].vbar = true;
        }
        Ok(())
    }

    /// Whether the stack entry is Go `opVerticalBar`.
    fn is_vertical_bar(&self, re: usize) -> bool {
        self.nodes[re].op == Op::Pseudo && self.nodes[re].vbar
    }

    /// Go `parser.swapVerticalBar` (parse.go:1375): if the top of the stack is an element
    /// followed by an opVerticalBar, swaps the two and returns true. Otherwise returns false.
    fn swap_vertical_bar(&mut self) -> bool {
        // If above and below vertical bar are literal or char class,
        // can merge into a single char class.
        let n = self.stack.len();
        if n >= 3
            && self.is_vertical_bar(self.stack[n - 2])
            && self.is_char_class(self.stack[n - 1])
            && self.is_char_class(self.stack[n - 3])
        {
            let mut re1 = self.stack[n - 1];
            let mut re3 = self.stack[n - 3];
            // Make re3 the more complex of the two.
            if (self.nodes[re1].op as u8) > (self.nodes[re3].op as u8) {
                std::mem::swap(&mut re1, &mut re3);
                self.stack[n - 3] = re3;
            }
            self.merge_char_class(re3, re1);
            self.reuse(re1);
            self.stack.truncate(n - 1);
            return true;
        }

        if n >= 2 {
            let re1 = self.stack[n - 1];
            let re2 = self.stack[n - 2];
            if self.is_vertical_bar(re2) {
                if n >= 3 {
                    // Now out of reach.
                    // Clean opportunistically.
                    self.clean_alt(self.stack[n - 3]);
                }
                self.stack[n - 2] = re1;
                self.stack[n - 1] = re2;
                return true;
            }
        }
        false
    }

    /// Go `parser.parseRightParen` (parse.go:1411): handles a ) in the input.
    fn parse_right_paren(&mut self) -> Result<(), Error> {
        self.concat()?;
        if self.swap_vertical_bar() {
            // pop vertical bar
            self.stack.pop();
        }
        self.alternate()?;

        let n = self.stack.len();
        if n < 2 {
            return Err(err(ErrorCode::UnexpectedParen, self.whole_regexp));
        }
        let re1 = self.stack[n - 1];
        let re2 = self.stack[n - 2];
        self.stack.truncate(n - 2);
        if self.nodes[re2].op != Op::Pseudo || self.nodes[re2].vbar {
            return Err(err(ErrorCode::UnexpectedParen, self.whole_regexp));
        }
        // Restore flags at time of paren.
        self.flags = self.nodes[re2].flags;
        if self.nodes[re2].cap == 0 {
            // Just for grouping.
            self.push(re1)
        } else {
            self.nodes[re2].op = Op::Capture;
            self.nodes[re2].sub = vec![re1];
            self.push(re2)
        }
    }

    /// Go `parser.parsePerlClassEscape` (parse.go:1585): parses a leading Perl character class
    /// escape like \d from the beginning of `s`. If one is present, appends the characters to
    /// `r` and returns the remainder of the string (Go's non-nil `out`).
    fn parse_perl_class_escape<'s>(&self, s: &'s str, r: &mut Vec<u32>) -> Option<&'s str> {
        if !has(self.flags, Flags::PERL_X) || s.len() < 2 || s.as_bytes()[0] != b'\\' {
            return None;
        }
        // `get` is None when byte 2 is inside a multi-byte rune; Go's map lookup misses then too.
        let g = perl_group(s.get(..2)?)?;
        self.append_group(r, g);
        Some(&s[2..])
    }

    /// Go `parser.parseNamedClass` (parse.go:1599): parses a leading POSIX named character class
    /// like [:alnum:] from the beginning of `s`. If one is present, appends the characters to `r`
    /// and returns the remainder of the string.
    fn parse_named_class<'s>(
        &self,
        s: &'s str,
        r: &mut Vec<u32>,
    ) -> Result<Option<&'s str>, Error> {
        if s.len() < 2 || s.as_bytes()[0] != b'[' || s.as_bytes()[1] != b':' {
            return Ok(None);
        }

        let Some(i) = s[2..].find(":]") else {
            return Ok(None);
        };
        let i = i + 2;
        let (name, rest) = (&s[..i + 2], &s[i + 2..]);
        let Some(g) = posix_group(name) else {
            return Err(err(ErrorCode::InvalidCharRange, name));
        };
        self.append_group(r, g);
        Ok(Some(rest))
    }

    /// Go `parser.appendGroup` (parse.go:1617).
    fn append_group(&self, r: &mut Vec<u32>, g: CharGroup) {
        let cur = std::mem::take(r);
        *r = if !has(self.flags, Flags::FOLD_CASE) {
            if g.sign < 0 {
                append_negated_class(cur, g.class)
            } else {
                append_class(cur, g.class)
            }
        } else {
            let tmp = clean_class(&append_folded_class(Vec::new(), g.class));
            if g.sign < 0 {
                append_negated_class(cur, &tmp)
            } else {
                append_class(cur, &tmp)
            }
        };
    }

    /// Go `parser.parseUnicodeClass` (parse.go:1760): parses a leading Unicode character class
    /// like \p{Han} from the beginning of `s`. If one is present, appends the characters to `r`
    /// and returns the remainder of the string.
    fn parse_unicode_class<'s>(
        &self,
        s: &'s str,
        r: &mut Vec<u32>,
    ) -> Result<Option<&'s str>, Error> {
        let b = s.as_bytes();
        if !has(self.flags, Flags::UNICODE_GROUPS)
            || s.len() < 2
            || b[0] != b'\\'
            || (b[1] != b'p' && b[1] != b'P')
        {
            return Ok(None);
        }

        // Committed to parse or return error.
        let mut sign = if b[1] == b'P' { -1 } else { 1 };
        let (c, mut t) = next_rune(&s[2..]);
        let (seq, mut name) = if c != '{' {
            // Single-letter name.
            let seq = &s[..s.len() - t.len()];
            (seq, &seq[2..])
        } else {
            // Name is in braces.
            let Some(end) = s.find('}') else {
                return Err(err(ErrorCode::InvalidCharRange, s));
            };
            t = &s[end + 1..];
            (&s[..=end], &s[3..end])
        };

        // Group can have leading negation too.  \p{^Han} == \P{Han}, \P{^Han} == \p{Han}.
        if let Some(rest) = name.strip_prefix('^') {
            sign = -sign;
            name = rest;
        }

        let Some((tab, fold_tab, tsign)) = unicode_table(name) else {
            return Err(err(ErrorCode::InvalidCharRange, seq));
        };
        if tsign < 0 {
            sign = -sign;
        }

        let cur = std::mem::take(r);
        *r = match fold_tab {
            Some(fold_tab) if has(self.flags, Flags::FOLD_CASE) => {
                // Merge and clean tab and fold in a temporary buffer.
                // This is necessary for the negative case and just tidy
                // for the positive case.
                let tmp = clean_class(&append_table(append_table(Vec::new(), tab), fold_tab));
                if sign > 0 {
                    append_class(cur, &tmp)
                } else {
                    append_negated_class(cur, &tmp)
                }
            }
            _ => {
                if sign > 0 {
                    append_table(cur, tab)
                } else {
                    append_negated_table(cur, tab)
                }
            }
        };
        Ok(Some(t))
    }

    /// Go `parser.parseClass` (parse.go:1836): parses a character class at the beginning of `s`
    /// and pushes it onto the parse stack.
    fn parse_class(&mut self, s: &'a str) -> Result<&'a str, Error> {
        let mut t = &s[1..]; // chop [
        let re = self.new_node(Op::CharClass);
        self.nodes[re].flags = self.flags;
        let mut class: Vec<u32> = Vec::new();

        let mut sign = 1;
        if let Some(rest) = t.strip_prefix('^') {
            sign = -1;
            t = rest;

            // If character class does not match \n, add it here,
            // so that negation later will do the right thing.
            if !has(self.flags, Flags::CLASS_NL) {
                class.extend([u32::from('\n'), u32::from('\n')]);
            }
        }

        let mut first = true; // ] and - are okay as first char in class
        while t.is_empty() || t.as_bytes()[0] != b']' || first {
            // POSIX: - is only okay unescaped as first or last in class.
            // Perl: - is okay anywhere.
            if !t.is_empty()
                && t.as_bytes()[0] == b'-'
                && !has(self.flags, Flags::PERL_X)
                && !first
                && (t.len() == 1 || t.as_bytes()[1] != b']')
            {
                let size = t[1..].chars().next().map_or(0, char::len_utf8);
                return Err(err(ErrorCode::InvalidCharRange, &t[..1 + size]));
            }
            first = false;

            // Look for POSIX [:alnum:] etc.
            if t.len() > 2
                && t.as_bytes()[0] == b'['
                && t.as_bytes()[1] == b':'
                && let Some(nt) = self.parse_named_class(t, &mut class)?
            {
                t = nt;
                continue;
            }

            // Look for Unicode character group like \p{Han}.
            if let Some(nt) = self.parse_unicode_class(t, &mut class)? {
                t = nt;
                continue;
            }

            // Look for Perl character class symbols (extension).
            if let Some(nt) = self.parse_perl_class_escape(t, &mut class) {
                t = nt;
                continue;
            }

            // Single character or simple range.
            let rng = t;
            let (lo, rest) = parse_class_char(t, s)?;
            t = rest;
            let mut hi = lo;
            // [a-] means (a|-) so check for final ].
            if t.len() >= 2 && t.as_bytes()[0] == b'-' && t.as_bytes()[1] != b']' {
                t = &t[1..];
                let (h, rest) = parse_class_char(t, s)?;
                hi = h;
                t = rest;
                if hi < lo {
                    return Err(err(
                        ErrorCode::InvalidCharRange,
                        &rng[..rng.len() - t.len()],
                    ));
                }
            }
            class = if has(self.flags, Flags::FOLD_CASE) {
                append_folded_range(class, lo, hi)
            } else {
                append_range(class, lo, hi)
            };
        }
        t = &t[1..]; // chop ]

        let mut class = clean_class(&class);
        if sign < 0 {
            class = negate_class(&class);
        }
        self.nodes[re].rune = class;
        self.push(re)?;
        Ok(t)
    }

    /// Go `parse` (parse.go:891), the non-`Literal` path: the main loop over the pattern, then
    /// the final concat/alternate. Go's `ErrLarge` / `ErrNestingDepth` panics come back as `Err`.
    fn run(&mut self, s: &'a str) -> Result<usize, Error> {
        let mut t = s;
        let mut last_repeat = "";
        while !t.is_empty() {
            let mut repeat = "";
            'big: {
                match t.as_bytes()[0] {
                    b'(' => {
                        if has(self.flags, Flags::PERL_X) && t.len() >= 2 && t.as_bytes()[1] == b'?'
                        {
                            // Flag changes and non-capturing groups.
                            t = self.parse_perl_flags(t)?;
                            break 'big;
                        }
                        self.num_cap += 1;
                        let id = self.op(Op::Pseudo)?;
                        self.nodes[id].cap = self.num_cap;
                        t = &t[1..];
                    }
                    b'|' => {
                        self.parse_vertical_bar()?;
                        t = &t[1..];
                    }
                    b')' => {
                        self.parse_right_paren()?;
                        t = &t[1..];
                    }
                    b'^' => {
                        if has(self.flags, Flags::ONE_LINE) {
                            self.op(Op::BeginText)?;
                        } else {
                            self.op(Op::BeginLine)?;
                        }
                        t = &t[1..];
                    }
                    b'$' => {
                        if has(self.flags, Flags::ONE_LINE) {
                            let id = self.op(Op::EndText)?;
                            self.nodes[id].flags = self.nodes[id].flags | Flags::WAS_DOLLAR;
                        } else {
                            self.op(Op::EndLine)?;
                        }
                        t = &t[1..];
                    }
                    b'.' => {
                        if has(self.flags, Flags::DOT_NL) {
                            self.op(Op::AnyChar)?;
                        } else {
                            self.op(Op::AnyCharNotNL)?;
                        }
                        t = &t[1..];
                    }
                    b'[' => {
                        t = self.parse_class(t)?;
                    }
                    c @ (b'*' | b'+' | b'?') => {
                        let before = t;
                        let op = match c {
                            b'*' => Op::Star,
                            b'+' => Op::Plus,
                            _ => Op::Quest,
                        };
                        let after = &t[1..];
                        let after = self.repeat(op, 0, 0, before, after, last_repeat)?;
                        repeat = before;
                        t = after;
                    }
                    b'{' => {
                        let before = t;
                        let Some((min, max, after)) = parse_repeat(t) else {
                            // If the repeat cannot be parsed, { is a literal.
                            self.literal(u32::from('{'))?;
                            t = &t[1..];
                            break 'big;
                        };
                        if !(0..=1000).contains(&min) || max > 1000 || (max >= 0 && min > max) {
                            // Numbers were too big, or max is present and min > max.
                            return Err(err(
                                ErrorCode::InvalidRepeatSize,
                                &before[..before.len() - after.len()],
                            ));
                        }
                        let after =
                            self.repeat(Op::Repeat, min, max, before, after, last_repeat)?;
                        repeat = before;
                        t = after;
                    }
                    b'\\' => {
                        if has(self.flags, Flags::PERL_X) && t.len() >= 2 {
                            match t.as_bytes()[1] {
                                b'A' => {
                                    self.op(Op::BeginText)?;
                                    t = &t[2..];
                                    break 'big;
                                }
                                b'b' => {
                                    self.op(Op::WordBoundary)?;
                                    t = &t[2..];
                                    break 'big;
                                }
                                b'B' => {
                                    self.op(Op::NoWordBoundary)?;
                                    t = &t[2..];
                                    break 'big;
                                }
                                b'C' => {
                                    // any byte; not supported
                                    return Err(err(ErrorCode::InvalidEscape, &t[..2]));
                                }
                                b'Q' => {
                                    // \Q ... \E: the ... is always literals
                                    let (lit, rest) =
                                        t[2..].split_once("\\E").unwrap_or((&t[2..], ""));
                                    t = rest;
                                    for c in lit.chars() {
                                        self.literal(u32::from(c))?;
                                    }
                                    break 'big;
                                }
                                b'z' => {
                                    self.op(Op::EndText)?;
                                    t = &t[2..];
                                    break 'big;
                                }
                                _ => {}
                            }
                        }

                        let re = self.new_node(Op::CharClass);
                        self.nodes[re].flags = self.flags;

                        // Look for Unicode character group like \p{Han}
                        if t.len() >= 2 && (t.as_bytes()[1] == b'p' || t.as_bytes()[1] == b'P') {
                            let mut r = Vec::new();
                            if let Some(rest) = self.parse_unicode_class(t, &mut r)? {
                                self.nodes[re].rune = r;
                                t = rest;
                                self.push(re)?;
                                break 'big;
                            }
                        }

                        // Perl character class escape.
                        let mut r = Vec::new();
                        if let Some(rest) = self.parse_perl_class_escape(t, &mut r) {
                            self.nodes[re].rune = r;
                            t = rest;
                            self.push(re)?;
                            break 'big;
                        }
                        self.reuse(re);

                        // Ordinary single-character escape.
                        let (c, rest) = parse_escape(t)?;
                        t = rest;
                        self.literal(c)?;
                    }
                    _ => {
                        let (c, rest) = next_rune(t);
                        t = rest;
                        self.literal(u32::from(c))?;
                    }
                }
            }
            last_repeat = repeat;
        }

        self.concat()?;
        if self.swap_vertical_bar() {
            // pop vertical bar
            self.stack.pop();
        }
        self.alternate()?;

        if self.stack.len() != 1 {
            return Err(err(ErrorCode::MissingParen, s));
        }
        Ok(self.stack[0])
    }

    /// Go `parser.parsePerlFlags` (parse.go:1141): parses a Perl flag setting or non-capturing
    /// group or both, like (?i) or (?: or (?i:. Returns `s` without the prefix and updates the
    /// parse state. The caller must have ensured that `s` begins with "(?".
    fn parse_perl_flags(&mut self, s: &'a str) -> Result<&'a str, Error> {
        let t = s;
        let b = t.as_bytes();

        // Check for named captures, first introduced in Python's regexp library.
        // As usual, there are three slightly different syntaxes:
        //
        //   (?P<name>expr)   the original, introduced by Python
        //   (?<name>expr)    the .NET alteration, adopted by Perl 5.10
        //   (?'name'expr)    another .NET alteration, adopted by Perl 5.10
        //
        // Perl 5.10 gave in and implemented the Python version too,
        // but they claim that the last two are the preferred forms.
        // PCRE and languages based on it (specifically, PHP and Ruby)
        // support all three as well. EcmaScript 4 uses only the Python form.
        //
        // In both the open source world (via Code Search) and the
        // Google source tree, (?P<expr>name) and (?<expr>name) are the
        // dominant forms of named captures and both are supported.
        let starts_with_p = t.len() > 4 && b[2] == b'P' && b[3] == b'<';
        let starts_with_name = t.len() > 3 && b[2] == b'<';

        if starts_with_p || starts_with_name {
            // position of expr start
            let expr_start_pos = if starts_with_name { 3 } else { 4 };

            // Pull out name.
            let Some(end) = t.find('>') else {
                return Err(err(ErrorCode::InvalidNamedCapture, s));
            };

            let capture = &t[..=end]; // "(?P<name>" or "(?<name>"
            let name = &t[expr_start_pos..end]; // "name"
            if !is_valid_capture_name(name) {
                return Err(err(ErrorCode::InvalidNamedCapture, capture));
            }

            // Like ordinary capture, but named.
            self.num_cap += 1;
            let id = self.op(Op::Pseudo)?;
            self.nodes[id].cap = self.num_cap;
            self.nodes[id].name = name.to_string();
            return Ok(&t[end + 1..]);
        }

        // Non-capturing group. Might also twiddle Perl flags.
        let mut t = &t[2..]; // skip (?
        let mut flags = self.flags;
        let mut sign = 1;
        let mut saw_flag = false;
        while !t.is_empty() {
            let (c, rest) = next_rune(t);
            t = rest;
            match c {
                // Flags.
                'i' => {
                    flags = flags | Flags::FOLD_CASE;
                    saw_flag = true;
                }
                'm' => {
                    flags = flags & !Flags::ONE_LINE;
                    saw_flag = true;
                }
                's' => {
                    flags = flags | Flags::DOT_NL;
                    saw_flag = true;
                }
                'U' => {
                    flags = flags | Flags::NON_GREEDY;
                    saw_flag = true;
                }

                // Switch to negation.
                '-' => {
                    if sign < 0 {
                        break;
                    }
                    sign = -1;
                    // Invert flags so that | above turn into &^ and vice versa.
                    // We'll invert flags again before using it below.
                    flags = !flags;
                    saw_flag = false;
                }

                // End of flags, starting group or not.
                ':' | ')' => {
                    if sign < 0 {
                        if !saw_flag {
                            break;
                        }
                        flags = !flags;
                    }
                    if c == ':' {
                        // Open new group
                        self.op(Op::Pseudo)?;
                    }
                    self.flags = flags;
                    return Ok(t);
                }

                _ => break,
            }
        }

        Err(err(ErrorCode::InvalidPerlOp, &s[..s.len() - t.len()]))
    }

    /// Converts the node `id` and its subtree to an owned `Regexp`.
    fn build_regexp(&mut self, id: usize) -> Regexp {
        let n = std::mem::replace(&mut self.nodes[id], Node::new(Op::EmptyMatch));
        Regexp {
            op: n.op,
            flags: n.flags,
            sub: n.sub.into_iter().map(|s| self.build_regexp(s)).collect(),
            rune: n.rune,
            min: n.min,
            max: n.max,
            cap: n.cap,
            name: n.name,
        }
    }
}

/// Go `parse` (parse.go:891), reached via `Parse` (parse.go:887).
pub(super) fn parse(s: &str, flags: Flags) -> Result<Regexp, Error> {
    if has(flags, Flags::LITERAL) {
        // Trivial parser for literal string.
        return Ok(literal_regexp(s, flags));
    }

    // Otherwise, must do real work.
    let mut p = Parser::new(s, flags);
    let root = p.run(s)?;
    Ok(p.build_regexp(root))
}

/// Go `literalRegexp` (parse.go:867). Go's `checkUTF8` (parse.go:2202) is not ported: a Rust
/// `&str` is always valid UTF-8, so `ErrInvalidUTF8` cannot occur.
fn literal_regexp(s: &str, flags: Flags) -> Regexp {
    Regexp {
        op: Op::Literal,
        flags,
        sub: Vec::new(),
        rune: s.chars().map(u32::from).collect(),
        min: 0,
        max: 0,
        cap: 0,
        name: String::new(),
    }
}

/// Go `minFoldRune` (parse.go:380): the minimum rune fold-equivalent to `r`.
fn min_fold_rune(r: u32) -> u32 {
    if r < u32::from(MIN_FOLD) || r > u32::from(MAX_FOLD) {
        return r;
    }
    let mut m = r;
    let r0 = r;
    let mut r = fold(r);
    while r != r0 {
        m = m.min(r);
        r = fold(r);
    }
    m
}

/// Go `parseRepeat` (parse.go:1102): parses {min} (max=min) or {min,} (max=-1) or {min,max}.
/// Returns `(min, max, rest)`, or `None` if `s` is not of that form (Go's `ok == false`). If `s`
/// has the right form but the values are too big, `min == -1`.
fn parse_repeat(s: &str) -> Option<(i32, i32, &str)> {
    let mut s = s.strip_prefix('{')?;
    let (mut min, rest) = parse_int(s)?;
    s = rest;
    if s.is_empty() {
        return None;
    }
    let max = if s.as_bytes()[0] != b',' {
        min
    } else {
        s = &s[1..];
        if s.is_empty() {
            return None;
        }
        if s.as_bytes()[0] == b'}' {
            -1
        } else {
            let (m, rest) = parse_int(s)?;
            s = rest;
            if m < 0 {
                // parseInt found too big a number
                min = -1;
            }
            m
        }
    };
    let rest = s.strip_prefix('}')?;
    Some((min, max, rest))
}

/// Go `parseInt` (parse.go:1273): parses a decimal integer; -1 is "too big a number".
fn parse_int(s: &str) -> Option<(i32, &str)> {
    let b = s.as_bytes();
    if b.first().is_none_or(|c| !c.is_ascii_digit()) {
        return None;
    }
    // Disallow leading zeros.
    if b.len() >= 2 && b[0] == b'0' && b[1].is_ascii_digit() {
        return None;
    }
    let ndigits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    // Have digits, compute value.
    let mut n: i32 = 0;
    for &d in &b[..ndigits] {
        // Avoid overflow.
        if n >= 100_000_000 {
            n = -1;
            break;
        }
        n = n * 10 + i32::from(d - b'0');
    }
    Some((n, &s[ndigits..]))
}

/// Go `isValidCaptureName` (parse.go:1260): whether `name` is `[A-Za-z0-9_]+`.
fn is_valid_capture_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Go `nextRune` (parse.go:2213). Input is valid UTF-8, so `ErrInvalidUTF8` cannot occur. On
/// empty input Go's `DecodeRuneInString` gives `(RuneError, 0)`, so this returns U+FFFD and `s`.
fn next_rune(s: &str) -> (char, &str) {
    match s.chars().next() {
        Some(c) => (c, &s[c.len_utf8()..]),
        None => ('\u{FFFD}', s),
    }
}

/// Go `parser.parseEscape` (parse.go:1445): parses an escape sequence at the beginning of `s`
/// and returns the rune and the rest of the string.
fn parse_escape(s: &str) -> Result<(u32, &str), Error> {
    let t = &s[1..];
    if t.is_empty() {
        return Err(err(ErrorCode::TrailingBackslash, ""));
    }
    let (c, mut t) = next_rune(t);

    // Go's `break Switch`: leave the block, then report ErrInvalidEscape.
    'bad: {
        match c {
            // Octal escapes.
            '0'..='7' => {
                // Single non-zero digit is a backreference; not supported
                if c != '0'
                    && !t
                        .as_bytes()
                        .first()
                        .is_some_and(|d| (b'0'..=b'7').contains(d))
                {
                    break 'bad;
                }
                // Consume up to three octal digits; already have one.
                let mut r = u32::from(c) - u32::from('0');
                for _ in 1..3 {
                    match t.as_bytes().first() {
                        Some(&d) if (b'0'..=b'7').contains(&d) => {
                            r = r * 8 + u32::from(d - b'0');
                            t = &t[1..];
                        }
                        _ => break,
                    }
                }
                return Ok((r, t));
            }

            // Hexadecimal escapes.
            'x' => {
                if t.is_empty() {
                    break 'bad;
                }
                let (c, rest) = next_rune(t);
                t = rest;
                if c == '{' {
                    // Any number of digits in braces.
                    // Perl accepts any text at all; it ignores all text
                    // after the first non-hex digit. We require only hex digits,
                    // and at least one.
                    let mut nhex = 0;
                    let mut r: u32 = 0;
                    loop {
                        if t.is_empty() {
                            break 'bad;
                        }
                        let (c, rest) = next_rune(t);
                        t = rest;
                        if c == '}' {
                            break;
                        }
                        let Some(v) = c.to_digit(16) else {
                            break 'bad;
                        };
                        r = r * 16 + v;
                        if r > MAX_RUNE {
                            break 'bad;
                        }
                        nhex += 1;
                    }
                    if nhex == 0 {
                        break 'bad;
                    }
                    return Ok((r, t));
                }

                // Easy case: two hex digits.
                let x = c.to_digit(16);
                let (c, rest) = next_rune(t);
                t = rest;
                let y = c.to_digit(16);
                let (Some(x), Some(y)) = (x, y) else {
                    break 'bad;
                };
                return Ok((x * 16 + y, t));
            }

            // C escapes. There is no case 'b', to avoid misparsing
            // the Perl word-boundary \b as the C backspace \b
            // when in POSIX mode. In Perl, /\b/ means word-boundary
            // but /[\b]/ means backspace. We don't support that.
            // If you want a backspace, embed a literal backspace
            // character or use \x08.
            'a' => return Ok((0x07, t)),
            'f' => return Ok((0x0C, t)),
            'n' => return Ok((u32::from('\n'), t)),
            'r' => return Ok((u32::from('\r'), t)),
            't' => return Ok((u32::from('\t'), t)),
            'v' => return Ok((0x0B, t)),

            _ => {
                if c.is_ascii() && !c.is_ascii_alphanumeric() {
                    // Escaped non-word characters are always themselves.
                    // PCRE is not quite so rigorous: it accepts things like
                    // \q, but we don't. We once rejected \_, but too many
                    // programs and people insist on using it, so allow \_.
                    return Ok((u32::from(c), t));
                }
            }
        }
    }
    Err(err(ErrorCode::InvalidEscape, &s[..s.len() - t.len()]))
}

/// Go `parser.parseClassChar` (parse.go:1561): parses a character class character at the
/// beginning of `s` and returns it.
fn parse_class_char<'a>(s: &'a str, whole_class: &str) -> Result<(u32, &'a str), Error> {
    if s.is_empty() {
        return Err(err(ErrorCode::MissingBracket, whole_class));
    }

    // Allow regular escape sequences even though
    // many need not be escaped in this context.
    if s.starts_with('\\') {
        return parse_escape(s);
    }

    let (c, rest) = next_rune(s);
    Ok((u32::from(c), rest))
}

/// Go `aliases` (parse.go:1657) after `initAliases` (parse.go:1664): `unicode.CategoryAliases`
/// and `unicode.Scripts` keyed by `canonicalName`.
type Aliases = (HashMap<String, &'static str>, HashMap<String, &'static str>);

fn aliases() -> &'static Aliases {
    static ALIASES: OnceLock<Aliases> = OnceLock::new();
    ALIASES.get_or_init(|| {
        let categories = CATEGORY_ALIASES
            .iter()
            .map(|&(name, actual)| (canonical_name(name), actual))
            .collect();
        let scripts = SCRIPTS
            .iter()
            .map(|&(name, _)| (canonical_name(name), name))
            .collect();
        (categories, scripts)
    })
}

/// Go `canonicalName` (parse.go:1680): the canonical lookup string for `name`. It has a leading
/// uppercase letter and then lowercase letters, and omits all underscores, spaces and hyphens.
fn canonical_name(name: &str) -> String {
    let mut b = String::with_capacity(name.len());
    let mut first = true;
    for c in name.chars() {
        let c = match c {
            '_' | '-' | ' ' => continue,
            _ if first => {
                first = false;
                c.to_ascii_uppercase()
            }
            _ => c.to_ascii_lowercase(),
        };
        b.push(c);
    }
    b
}

/// Go `unicodeTable` (parse.go:1720): the table identified by `name`, the table of additional
/// fold-equivalent code points, and the sign (negative means the result should be inverted).
/// `None` is Go's `tab == nil`.
fn unicode_table(name: &str) -> Option<(&'static RangeTable, Option<&'static RangeTable>, i32)> {
    let name = canonical_name(name);

    // Special cases: Any, Assigned, and ASCII.
    // Also LC is the only non-canonical Categories key, so handle it here.
    match name.as_str() {
        "Any" => return Some((&ANY_TABLE, Some(&ANY_TABLE), 1)),
        "Assigned" => return Some((&CN, Some(&CN), -1)), // invert Cn (unassigned)
        "Ascii" => return Some((&ASCII_TABLE, Some(&ASCII_FOLD_TABLE), 1)),
        "Lc" => return category("LC").map(|t| (t, fold_category("LC"), 1)),
        _ => {}
    }
    if let Some(t) = category(&name) {
        return Some((t, fold_category(&name), 1));
    }
    if let Some(t) = script(&name) {
        return Some((t, fold_script(&name), 1));
    }

    // unicode.CategoryAliases makes liberal use of underscores in its names
    // (they are defined that way by Unicode), but we want to match ignoring
    // the underscores, so use our own map with canonical names.
    let (categories, scripts) = aliases();
    if let Some(&actual) = categories.get(&name) {
        return category(actual).map(|t| (t, fold_category(actual), 1));
    }
    if let Some(&actual) = scripts.get(&name) {
        return script(actual).map(|t| (t, fold_script(actual), 1));
    }
    None
}

/// Go `cleanClass` (parse.go:1932): sorts the ranges (pairs of elements of `r`), merges them,
/// and eliminates duplicates. Returns a new vector where Go sorts and merges in place.
fn clean_class(r: &[u32]) -> Vec<u32> {
    // Sort by lo increasing, hi decreasing to break ties.
    let mut pairs: Vec<[u32; 2]> = r.as_chunks::<2>().0.to_vec();
    pairs.sort_by(|a, b| a[0].cmp(&b[0]).then(b[1].cmp(&a[1])));

    // Merge abutting, overlapping.
    let mut merged: Vec<[u32; 2]> = Vec::with_capacity(pairs.len());
    for [lo, hi] in pairs {
        match merged.last_mut() {
            Some(last) if lo <= last[1] + 1 => {
                // merge with previous range
                if hi > last[1] {
                    last[1] = hi;
                }
            }
            // new disjoint range
            _ => merged.push([lo, hi]),
        }
    }
    merged.into_iter().flatten().collect()
}

/// Go `inCharClass` (parse.go:1964): whether `r` is in the class. Assumes the class has been
/// cleaned by `clean_class`.
pub(super) fn in_char_class(r: u32, class: &[u32]) -> bool {
    class
        .as_chunks::<2>()
        .0
        .binary_search_by(|&[lo, hi]| {
            if r > hi {
                std::cmp::Ordering::Less
            } else if r < lo {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Go `appendLiteral` (parse.go:1979): appends the literal `x` to the class `r`.
fn append_literal(r: Vec<u32>, x: u32, flags: Flags) -> Vec<u32> {
    if has(flags, Flags::FOLD_CASE) {
        return append_folded_range(r, x, x);
    }
    append_range(r, x, x)
}

/// Go `appendRange` (parse.go:1987): appends the range lo-hi to the class `r`. Re-exported via
/// mod.rs::append_range.
pub(super) fn append_range(mut r: Vec<u32>, lo: u32, hi: u32) -> Vec<u32> {
    // Expand last range or next to last range if it overlaps or abuts.
    // Checking two ranges helps when appending case-folded
    // alphabets, so that one range can be expanding A-Z and the
    // other expanding a-z.
    let n = r.len();
    for i in [2, 4] {
        // twice, using i=2, i=4
        if n >= i {
            let (rlo, rhi) = (r[n - i], r[n - i + 1]);
            if lo <= rhi + 1 && rlo <= hi + 1 {
                if lo < rlo {
                    r[n - i] = lo;
                }
                if hi > rhi {
                    r[n - i + 1] = hi;
                }
                return r;
            }
        }
    }

    r.push(lo);
    r.push(hi);
    r
}

/// Go `appendFoldedRange` (parse.go:2020): appends the range lo-hi and its case
/// folding-equivalent runes to the class `r`.
fn append_folded_range(mut r: Vec<u32>, mut lo: u32, mut hi: u32) -> Vec<u32> {
    let (min_fold, max_fold) = (u32::from(MIN_FOLD), u32::from(MAX_FOLD));
    // Optimizations.
    if lo <= min_fold && hi >= max_fold {
        // Range is full: folding can't add more.
        return append_range(r, lo, hi);
    }
    if hi < min_fold || lo > max_fold {
        // Range is outside folding possibilities.
        return append_range(r, lo, hi);
    }
    if lo < min_fold {
        // [lo, minFold-1] needs no folding.
        r = append_range(r, lo, min_fold - 1);
        lo = min_fold;
    }
    if hi > max_fold {
        // [maxFold+1, hi] needs no folding.
        r = append_range(r, max_fold + 1, hi);
        hi = max_fold;
    }

    // Brute force. Depend on appendRange to coalesce ranges on the fly.
    for c in lo..=hi {
        r = append_range(r, c, c);
        let mut f = fold(c);
        while f != c {
            r = append_range(r, f, f);
            f = fold(f);
        }
    }
    r
}

/// Go `appendClass` (parse.go:2055): appends the class `x` to the class `r`. Assumes `x` is clean.
fn append_class(mut r: Vec<u32>, x: &[u32]) -> Vec<u32> {
    for &[lo, hi] in x.as_chunks::<2>().0 {
        r = append_range(r, lo, hi);
    }
    r
}

/// Go `appendFoldedClass` (parse.go:2063): appends the case folding of the class `x` to `r`.
fn append_folded_class(mut r: Vec<u32>, x: &[u32]) -> Vec<u32> {
    for &[lo, hi] in x.as_chunks::<2>().0 {
        r = append_folded_range(r, lo, hi);
    }
    r
}

/// Go `appendNegatedClass` (parse.go:2072): appends the negation of the class `x` to `r`.
/// Assumes `x` is clean.
fn append_negated_class(mut r: Vec<u32>, x: &[u32]) -> Vec<u32> {
    let mut next_lo = 0;
    for &[lo, hi] in x.as_chunks::<2>().0 {
        // Go: nextLo <= lo-1
        if next_lo < lo {
            r = append_range(r, next_lo, lo - 1);
        }
        next_lo = hi + 1;
    }
    if next_lo <= MAX_RUNE {
        r = append_range(r, next_lo, MAX_RUNE);
    }
    r
}

/// The `(lo, hi, stride)` of every range of `x`: Go's `R16` loop then its `R32` loop.
fn table_ranges(x: &RangeTable) -> impl Iterator<Item = (u32, u32, u32)> {
    let r16 = x
        .r16
        .iter()
        .map(|g| (u32::from(g.lo), u32::from(g.hi), u32::from(g.stride)));
    let r32 = x.r32.iter().map(|g| (g.lo, g.hi, g.stride));
    r16.chain(r32)
}

/// Go `appendTable` (parse.go:2088): appends `x` to the class `r`.
fn append_table(mut r: Vec<u32>, x: &RangeTable) -> Vec<u32> {
    for (lo, hi, stride) in table_ranges(x) {
        if stride == 1 {
            r = append_range(r, lo, hi);
            continue;
        }
        let mut c = lo;
        while c <= hi {
            r = append_range(r, c, c);
            c += stride;
        }
    }
    r
}

/// Go `appendNegatedTable` (parse.go:2113): appends the negation of `x` to the class `r`.
fn append_negated_table(mut r: Vec<u32>, x: &RangeTable) -> Vec<u32> {
    let mut next_lo = 0; // lo end of next class to add
    for (lo, hi, stride) in table_ranges(x) {
        if stride == 1 {
            if next_lo < lo {
                r = append_range(r, next_lo, lo - 1);
            }
            next_lo = hi + 1;
            continue;
        }
        let mut c = lo;
        while c <= hi {
            if next_lo < c {
                r = append_range(r, next_lo, c - 1);
            }
            next_lo = c + 1;
            c += stride;
        }
    }
    if next_lo <= MAX_RUNE {
        r = append_range(r, next_lo, MAX_RUNE);
    }
    r
}

/// Go `negateClass` (parse.go:2155): returns the negation of the class `r`. Assumes `r` is
/// already clean. Builds a new vector where Go overwrites `r` in place.
fn negate_class(r: &[u32]) -> Vec<u32> {
    let mut next_lo = 0; // lo end of next class to add
    let mut out = Vec::with_capacity(r.len() + 2);
    for &[lo, hi] in r.as_chunks::<2>().0 {
        if next_lo < lo {
            out.push(next_lo);
            out.push(lo - 1);
        }
        next_lo = hi + 1;
    }
    if next_lo <= MAX_RUNE {
        out.push(next_lo);
        out.push(MAX_RUNE);
    }
    out
}
