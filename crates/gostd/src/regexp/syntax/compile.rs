//! Port of Go 1.27.4 regexp/syntax/compile.go: Compile, the compiler, frag and patchList.

use super::{EmptyOp, Error, Flags, Inst, InstOp, Op, Prog, Regexp};
use crate::unicode::simple_fold;

/// Go `unicode.MaxRune`.
const MAX_RUNE: u32 = 0x10FFFF;

/// Go `patchList` (compile.go:19): a list of instruction pointers still to be patched, stored in
/// the unfilled `out`/`arg` fields themselves. `head >> 1` is the instruction index and `head & 1`
/// selects `arg` (1) or `out` (0). `head == 0` is the empty list (instruction 0 is always `Fail`).
#[derive(Debug, Clone, Copy, Default)]
struct PatchList {
    head: u32,
    tail: u32,
}

impl PatchList {
    /// Go `makePatchList` (compile.go:23).
    fn new(n: u32) -> PatchList {
        PatchList { head: n, tail: n }
    }

    /// Go `patchList.patch` (compile.go:27).
    fn patch(self, p: &mut Prog, val: u32) {
        let mut head = self.head;
        while head != 0 {
            let i = &mut p.inst[(head >> 1) as usize];
            if head & 1 == 0 {
                head = i.out;
                i.out = val;
            } else {
                head = i.arg;
                i.arg = val;
            }
        }
    }

    /// Go `patchList.append` (compile.go:41).
    fn append(self, p: &mut Prog, l2: PatchList) -> PatchList {
        if self.head == 0 {
            return l2;
        }
        if l2.head == 0 {
            return self;
        }

        let i = &mut p.inst[(self.tail >> 1) as usize];
        if self.tail & 1 == 0 {
            i.out = l2.head;
        } else {
            i.arg = l2.head;
        }
        PatchList {
            head: self.head,
            tail: l2.tail,
        }
    }
}

/// Go `frag` (compile.go:62): a compiled program fragment.
#[derive(Debug, Clone, Copy, Default)]
struct Frag {
    /// Index of first instruction.
    i: u32,
    /// Where to record end instruction.
    out: PatchList,
    /// Whether fragment can match empty string.
    nullable: bool,
}

/// Go `compiler` (compile.go:68).
struct Compiler {
    p: Prog,
}

/// Go `Compile` (compile.go:71). Expects a simplified regexp. Always returns `Ok`, as in Go.
pub(super) fn compile(re: &Regexp) -> Result<Prog, Error> {
    let mut c = Compiler::new();
    let f = c.compile(re);
    let m = c.inst(InstOp::Match).i;
    f.out.patch(&mut c.p, m);
    c.p.start = f.i as usize;
    Ok(c.p)
}

/// Go `anyRuneNotNL` (compile.go:86).
const ANY_RUNE_NOT_NL: [u32; 4] = [0, '\n' as u32 - 1, '\n' as u32 + 1, MAX_RUNE];
/// Go `anyRune` (compile.go:87).
const ANY_RUNE: [u32; 2] = [0, MAX_RUNE];

impl Compiler {
    /// Go `compiler.init` (compile.go:80).
    fn new() -> Compiler {
        let mut c = Compiler {
            p: Prog {
                inst: Vec::new(),
                start: 0,
                // implicit ( and ) for whole match $0
                num_cap: 2,
            },
        };
        c.inst(InstOp::Fail);
        c
    }

    /// Go `compiler.compile` (compile.go:89).
    fn compile(&mut self, re: &Regexp) -> Frag {
        match re.op {
            Op::NoMatch => self.fail(),
            Op::EmptyMatch => self.nop(),
            Op::Literal => {
                if re.rune.is_empty() {
                    return self.nop();
                }
                let mut f = Frag::default();
                for (j, r) in re.rune.iter().enumerate() {
                    let f1 = self.rune(std::slice::from_ref(r), re.flags);
                    if j == 0 {
                        f = f1;
                    } else {
                        f = self.cat(f, f1);
                    }
                }
                f
            }
            Op::CharClass => self.rune(&re.rune, re.flags),
            Op::AnyCharNotNL => self.rune(&ANY_RUNE_NOT_NL, Flags(0)),
            Op::AnyChar => self.rune(&ANY_RUNE, Flags(0)),
            Op::BeginLine => self.empty(EmptyOp::BEGIN_LINE),
            Op::EndLine => self.empty(EmptyOp::END_LINE),
            Op::BeginText => self.empty(EmptyOp::BEGIN_TEXT),
            Op::EndText => self.empty(EmptyOp::END_TEXT),
            Op::WordBoundary => self.empty(EmptyOp::WORD_BOUNDARY),
            Op::NoWordBoundary => self.empty(EmptyOp::NO_WORD_BOUNDARY),
            Op::Capture => {
                let bra = self.cap((re.cap << 1) as u32);
                let sub = self.compile(&re.sub[0]);
                let ket = self.cap(((re.cap << 1) | 1) as u32);
                let f = self.cat(bra, sub);
                self.cat(f, ket)
            }
            Op::Star => {
                let f1 = self.compile(&re.sub[0]);
                self.star(f1, non_greedy(re))
            }
            Op::Plus => {
                let f1 = self.compile(&re.sub[0]);
                self.plus(f1, non_greedy(re))
            }
            Op::Quest => {
                let f1 = self.compile(&re.sub[0]);
                self.quest(f1, non_greedy(re))
            }
            Op::Concat => {
                if re.sub.is_empty() {
                    return self.nop();
                }
                let mut f = Frag::default();
                for (i, sub) in re.sub.iter().enumerate() {
                    if i == 0 {
                        f = self.compile(sub);
                    } else {
                        let g = self.compile(sub);
                        f = self.cat(f, g);
                    }
                }
                f
            }
            Op::Alternate => {
                let mut f = Frag::default();
                for sub in &re.sub {
                    let g = self.compile(sub);
                    f = self.alt(f, g);
                }
                f
            }
            Op::Repeat | Op::Pseudo => panic!("regexp: unhandled case in compile"),
        }
    }

    /// Go `compiler.inst` (compile.go:161).
    fn inst(&mut self, op: InstOp) -> Frag {
        // TODO (Go): impose length limit
        let f = Frag {
            i: self.p.inst.len() as u32,
            out: PatchList::default(),
            nullable: true,
        };
        self.p.inst.push(Inst {
            op,
            out: 0,
            arg: 0,
            rune: Vec::new(),
        });
        f
    }

    /// Go `compiler.nop` (compile.go:168).
    fn nop(&mut self) -> Frag {
        let mut f = self.inst(InstOp::Nop);
        f.out = PatchList::new(f.i << 1);
        f
    }

    /// Go `compiler.fail` (compile.go:174).
    fn fail(&mut self) -> Frag {
        Frag::default()
    }

    /// Go `compiler.cap` (compile.go:178).
    fn cap(&mut self, arg: u32) -> Frag {
        let mut f = self.inst(InstOp::Capture);
        f.out = PatchList::new(f.i << 1);
        self.p.inst[f.i as usize].arg = arg;

        if self.p.num_cap < arg as usize + 1 {
            self.p.num_cap = arg as usize + 1;
        }
        f
    }

    /// Go `compiler.cat` (compile.go:189).
    fn cat(&mut self, f1: Frag, f2: Frag) -> Frag {
        // concat of failure is failure
        if f1.i == 0 || f2.i == 0 {
            return Frag::default();
        }

        // TODO (Go): elide nop

        f1.out.patch(&mut self.p, f2.i);
        Frag {
            i: f1.i,
            out: f2.out,
            nullable: f1.nullable && f2.nullable,
        }
    }

    /// Go `compiler.alt` (compile.go:201).
    fn alt(&mut self, f1: Frag, f2: Frag) -> Frag {
        // alt of failure is other
        if f1.i == 0 {
            return f2;
        }
        if f2.i == 0 {
            return f1;
        }

        let mut f = self.inst(InstOp::Alt);
        let i = &mut self.p.inst[f.i as usize];
        i.out = f1.i;
        i.arg = f2.i;
        f.out = f1.out.append(&mut self.p, f2.out);
        f.nullable = f1.nullable || f2.nullable;
        f
    }

    /// Go `compiler.quest` (compile.go:219).
    fn quest(&mut self, f1: Frag, nongreedy: bool) -> Frag {
        let mut f = self.inst(InstOp::Alt);
        let i = &mut self.p.inst[f.i as usize];
        if nongreedy {
            i.arg = f1.i;
            f.out = PatchList::new(f.i << 1);
        } else {
            i.out = f1.i;
            f.out = PatchList::new((f.i << 1) | 1);
        }
        f.out = f.out.append(&mut self.p, f1.out);
        f
    }

    /// Go `compiler.loop` (compile.go:238): the main loop of a plus or star. For plus, it can be
    /// used after changing the entry to `f1.i`. For star, it can be used directly when `f1` can't
    /// match an empty string. (When `f1` can match an empty string, `f1*` must be implemented as
    /// `(f1+)?` to get the priority match order correct.)
    fn loop_(&mut self, f1: Frag, nongreedy: bool) -> Frag {
        let mut f = self.inst(InstOp::Alt);
        let i = &mut self.p.inst[f.i as usize];
        if nongreedy {
            i.arg = f1.i;
            f.out = PatchList::new(f.i << 1);
        } else {
            i.out = f1.i;
            f.out = PatchList::new((f.i << 1) | 1);
        }
        f1.out.patch(&mut self.p, f.i);
        f
    }

    /// Go `compiler.star` (compile.go:252).
    fn star(&mut self, f1: Frag, nongreedy: bool) -> Frag {
        if f1.nullable {
            // Use (f1+)? to get priority match order correct.
            // See golang.org/issue/46123.
            let f = self.plus(f1, nongreedy);
            return self.quest(f, nongreedy);
        }
        self.loop_(f1, nongreedy)
    }

    /// Go `compiler.plus` (compile.go:261).
    fn plus(&mut self, f1: Frag, nongreedy: bool) -> Frag {
        Frag {
            i: f1.i,
            out: self.loop_(f1, nongreedy).out,
            nullable: f1.nullable,
        }
    }

    /// Go `compiler.empty` (compile.go:265).
    fn empty(&mut self, op: EmptyOp) -> Frag {
        let mut f = self.inst(InstOp::EmptyWidth);
        self.p.inst[f.i as usize].arg = op.0 as u32;
        f.out = PatchList::new(f.i << 1);
        f
    }

    /// Go `compiler.rune` (compile.go:272).
    fn rune(&mut self, r: &[u32], flags: Flags) -> Frag {
        let mut f = self.inst(InstOp::Rune);
        f.nullable = false;
        // Go: flags &= FoldCase // only relevant flag is FoldCase
        let mut flags = flags & Flags::FOLD_CASE;
        // Go `unicode.SimpleFold`: runes that are not scalar values (surrogates) fold to themselves.
        if r.len() != 1 || char::from_u32(r[0]).is_none_or(|c| simple_fold(c) as u32 == r[0]) {
            // and sometimes not even that
            flags = flags & !Flags::FOLD_CASE;
        }
        let fold = flags != Flags(0);
        let i = &mut self.p.inst[f.i as usize];
        i.rune = r.to_vec();
        i.arg = flags.0 as u32;
        f.out = PatchList::new(f.i << 1);

        // Special cases for exec machine.
        if !fold && (r.len() == 1 || r.len() == 2 && r[0] == r[1]) {
            i.op = InstOp::Rune1;
        } else if r.len() == 2 && r[0] == 0 && r[1] == MAX_RUNE {
            i.op = InstOp::RuneAny;
        } else if r.len() == 4
            && r[0] == 0
            && r[1] == '\n' as u32 - 1
            && r[2] == '\n' as u32 + 1
            && r[3] == MAX_RUNE
        {
            i.op = InstOp::RuneAnyNotNL;
        }

        f
    }
}

/// Go `re.Flags&NonGreedy != 0`.
fn non_greedy(re: &Regexp) -> bool {
    (re.flags & Flags::NON_GREEDY) != Flags(0)
}
