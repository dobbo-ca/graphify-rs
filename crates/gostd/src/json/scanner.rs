//! Go `scanner.go`.

use super::Error;

/// Go `Valid` (`scanner.go:24`).
pub fn valid(data: &[u8]) -> bool {
    check_valid(data).is_ok()
}

/// Go `checkValid` (`scanner.go:33`): `Ok` or the `SyntaxError`.
pub(super) fn check_valid(data: &[u8]) -> Result<(), Error> {
    let mut scan = Scanner::new();
    for &c in data {
        scan.bytes += 1;
        if scan.step(c) == Op::Error {
            return Err(scan.err.expect("scanError sets err"));
        }
    }
    if scan.eof() == Op::Error {
        return Err(scan.err.expect("scanError sets err"));
    }
    Ok(())
}

/// Go scanner opcodes (`scanner.go:118`); the derived order is Go's, `>= SkipSpace` is used by
/// `appendCompact`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Op {
    Continue,
    BeginLiteral,
    BeginObject,
    ObjectKey,
    ObjectValue,
    EndObject,
    BeginArray,
    ArrayValue,
    EndArray,
    SkipSpace,
    End,
    Error,
}

/// Go parseState values (`scanner.go:140`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Parse {
    ObjectKey,
    ObjectValue,
    ArrayValue,
}

/// Go `maxNestingDepth` (`scanner.go:148`).
const MAX_NESTING_DEPTH: usize = 10000;

type Step = fn(&mut Scanner, u8) -> Op;

/// Go `scanner` (`scanner.go:68`); `step` is the state function.
pub(super) struct Scanner {
    step: Step,
    end_top: bool,
    parse_state: Vec<Parse>,
    pub(super) err: Option<Error>,
    pub(super) bytes: i64,
}

impl Scanner {
    /// Go `newScanner` (`scanner.go:95`).
    pub(super) fn new() -> Self {
        Scanner {
            step: state_begin_value,
            end_top: false,
            parse_state: Vec::new(),
            err: None,
            bytes: 0,
        }
    }

    /// Calls the current state function, Go `scan.step(scan, c)`.
    pub(super) fn step(&mut self, c: u8) -> Op {
        (self.step)(self, c)
    }

    /// Go `scanner.eof` (`scanner.go:161`).
    pub(super) fn eof(&mut self) -> Op {
        if self.err.is_some() {
            return Op::Error;
        }
        if self.end_top {
            return Op::End;
        }
        self.step(b' ');
        if self.end_top {
            return Op::End;
        }
        if self.err.is_none() {
            self.err = Some(Error::Syntax {
                msg: "unexpected end of JSON input".to_string(),
                offset: self.bytes,
            });
        }
        Op::Error
    }

    /// Go `scanner.pushParseState` (`scanner.go:180`).
    fn push_parse_state(&mut self, c: u8, new: Parse, success: Op) -> Op {
        self.parse_state.push(new);
        if self.parse_state.len() <= MAX_NESTING_DEPTH {
            return success;
        }
        self.error(c, "exceeded max depth")
    }

    /// Go `scanner.popParseState` (`scanner.go:190`).
    fn pop_parse_state(&mut self) {
        self.parse_state.pop();
        if self.parse_state.is_empty() {
            self.step = state_end_top;
            self.end_top = true;
        } else {
            self.step = state_end_value;
        }
    }

    /// Go `scanner.error` (`scanner.go:593`).
    fn error(&mut self, c: u8, context: &str) -> Op {
        self.step = state_error;
        self.err = Some(Error::Syntax {
            msg: format!("invalid character {} {context}", quote_char(c)),
            offset: self.bytes,
        });
        Op::Error
    }
}

/// Go `isSpace` (`scanner.go:201`).
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

/// Go `stateBeginValueOrEmpty` (`scanner.go:206`).
fn state_begin_value_or_empty(s: &mut Scanner, c: u8) -> Op {
    if is_space(c) {
        return Op::SkipSpace;
    }
    if c == b']' {
        return state_end_value(s, c);
    }
    state_begin_value(s, c)
}

/// Go `stateBeginValue` (`scanner.go:217`).
fn state_begin_value(s: &mut Scanner, c: u8) -> Op {
    if is_space(c) {
        return Op::SkipSpace;
    }
    match c {
        b'{' => {
            s.step = state_begin_string_or_empty;
            s.push_parse_state(c, Parse::ObjectKey, Op::BeginObject)
        }
        b'[' => {
            s.step = state_begin_value_or_empty;
            s.push_parse_state(c, Parse::ArrayValue, Op::BeginArray)
        }
        b'"' => {
            s.step = state_in_string;
            Op::BeginLiteral
        }
        b'-' => {
            s.step = state_neg;
            Op::BeginLiteral
        }
        b'0' => {
            s.step = state0;
            Op::BeginLiteral
        }
        b't' => {
            s.step = state_t;
            Op::BeginLiteral
        }
        b'f' => {
            s.step = state_f;
            Op::BeginLiteral
        }
        b'n' => {
            s.step = state_n;
            Op::BeginLiteral
        }
        b'1'..=b'9' => {
            s.step = state1;
            Op::BeginLiteral
        }
        _ => s.error(c, "looking for beginning of value"),
    }
}

/// Go `stateBeginStringOrEmpty` (`scanner.go:255`).
fn state_begin_string_or_empty(s: &mut Scanner, c: u8) -> Op {
    if is_space(c) {
        return Op::SkipSpace;
    }
    if c == b'}' {
        let n = s.parse_state.len();
        s.parse_state[n - 1] = Parse::ObjectValue;
        return state_end_value(s, c);
    }
    state_begin_string(s, c)
}

/// Go `stateBeginString` (`scanner.go:268`).
fn state_begin_string(s: &mut Scanner, c: u8) -> Op {
    if is_space(c) {
        return Op::SkipSpace;
    }
    if c == b'"' {
        s.step = state_in_string;
        return Op::BeginLiteral;
    }
    s.error(c, "looking for beginning of object key string")
}

/// Go `stateEndValue` (`scanner.go:281`).
fn state_end_value(s: &mut Scanner, c: u8) -> Op {
    let n = s.parse_state.len();
    if n == 0 {
        // Completed top-level before the current byte.
        s.step = state_end_top;
        s.end_top = true;
        return state_end_top(s, c);
    }
    if is_space(c) {
        s.step = state_end_value;
        return Op::SkipSpace;
    }
    match s.parse_state[n - 1] {
        Parse::ObjectKey => {
            if c == b':' {
                s.parse_state[n - 1] = Parse::ObjectValue;
                s.step = state_begin_value;
                return Op::ObjectKey;
            }
            s.error(c, "after object key")
        }
        Parse::ObjectValue => {
            if c == b',' {
                s.parse_state[n - 1] = Parse::ObjectKey;
                s.step = state_begin_string;
                return Op::ObjectValue;
            }
            if c == b'}' {
                s.pop_parse_state();
                return Op::EndObject;
            }
            s.error(c, "after object key:value pair")
        }
        Parse::ArrayValue => {
            if c == b',' {
                s.step = state_begin_value;
                return Op::ArrayValue;
            }
            if c == b']' {
                s.pop_parse_state();
                return Op::EndArray;
            }
            s.error(c, "after array element")
        }
    }
}

/// Go `stateEndTop` (`scanner.go:330`).
fn state_end_top(s: &mut Scanner, c: u8) -> Op {
    if !is_space(c) {
        // Complain about non-space byte on next call.
        s.error(c, "after top-level value");
    }
    Op::End
}

/// Go `stateInString` (`scanner.go:339`).
fn state_in_string(s: &mut Scanner, c: u8) -> Op {
    if c == b'"' {
        s.step = state_end_value;
        return Op::Continue;
    }
    if c == b'\\' {
        s.step = state_in_string_esc;
        return Op::Continue;
    }
    if c < 0x20 {
        return s.error(c, "in string literal");
    }
    Op::Continue
}

/// Go `stateInStringEsc` (`scanner.go:355`).
fn state_in_string_esc(s: &mut Scanner, c: u8) -> Op {
    match c {
        b'b' | b'f' | b'n' | b'r' | b't' | b'\\' | b'/' | b'"' => {
            s.step = state_in_string;
            Op::Continue
        }
        b'u' => {
            s.step = state_in_string_esc_u;
            Op::Continue
        }
        _ => s.error(c, "in string escape code"),
    }
}

/// The shared body of Go `stateInStringEscU`, `...U1`, `...U12`, `...U123` (`scanner.go:368-398`).
fn hex_step(s: &mut Scanner, c: u8, next: Step) -> Op {
    if c.is_ascii_hexdigit() {
        s.step = next;
        return Op::Continue;
    }
    // numbers
    s.error(c, "in \\u hexadecimal character escape")
}

/// Go `stateInStringEscU` (`scanner.go:368`).
fn state_in_string_esc_u(s: &mut Scanner, c: u8) -> Op {
    hex_step(s, c, state_in_string_esc_u1)
}

/// Go `stateInStringEscU1` (`scanner.go:378`).
fn state_in_string_esc_u1(s: &mut Scanner, c: u8) -> Op {
    hex_step(s, c, state_in_string_esc_u12)
}

/// Go `stateInStringEscU12` (`scanner.go:388`).
fn state_in_string_esc_u12(s: &mut Scanner, c: u8) -> Op {
    hex_step(s, c, state_in_string_esc_u123)
}

/// Go `stateInStringEscU123` (`scanner.go:398`).
fn state_in_string_esc_u123(s: &mut Scanner, c: u8) -> Op {
    hex_step(s, c, state_in_string)
}

/// Go `stateNeg` (`scanner.go:408`).
fn state_neg(s: &mut Scanner, c: u8) -> Op {
    if c == b'0' {
        s.step = state0;
        return Op::Continue;
    }
    if (b'1'..=b'9').contains(&c) {
        s.step = state1;
        return Op::Continue;
    }
    s.error(c, "in numeric literal")
}

/// Go `state1` (`scanner.go:422`).
fn state1(s: &mut Scanner, c: u8) -> Op {
    if c.is_ascii_digit() {
        s.step = state1;
        return Op::Continue;
    }
    state0(s, c)
}

/// Go `state0` (`scanner.go:431`).
fn state0(s: &mut Scanner, c: u8) -> Op {
    if c == b'.' {
        s.step = state_dot;
        return Op::Continue;
    }
    if c == b'e' || c == b'E' {
        s.step = state_e;
        return Op::Continue;
    }
    state_end_value(s, c)
}

/// Go `stateDot` (`scanner.go:445`).
fn state_dot(s: &mut Scanner, c: u8) -> Op {
    if c.is_ascii_digit() {
        s.step = state_dot0;
        return Op::Continue;
    }
    s.error(c, "after decimal point in numeric literal")
}

/// Go `stateDot0` (`scanner.go:455`).
fn state_dot0(s: &mut Scanner, c: u8) -> Op {
    if c.is_ascii_digit() {
        return Op::Continue;
    }
    if c == b'e' || c == b'E' {
        s.step = state_e;
        return Op::Continue;
    }
    state_end_value(s, c)
}

/// Go `stateE` (`scanner.go:468`).
fn state_e(s: &mut Scanner, c: u8) -> Op {
    if c == b'+' || c == b'-' {
        s.step = state_e_sign;
        return Op::Continue;
    }
    state_e_sign(s, c)
}

/// Go `stateESign` (`scanner.go:478`).
fn state_e_sign(s: &mut Scanner, c: u8) -> Op {
    if c.is_ascii_digit() {
        s.step = state_e0;
        return Op::Continue;
    }
    s.error(c, "in exponent of numeric literal")
}

/// Go `stateE0` (`scanner.go:489`).
fn state_e0(s: &mut Scanner, c: u8) -> Op {
    if c.is_ascii_digit() {
        return Op::Continue;
    }
    state_end_value(s, c)
}

/// One byte of a literal: on `want` go to `next`, else error `context` (Go `stateT` ... `stateNul`,
/// `scanner.go:497-586`).
fn literal_step(s: &mut Scanner, c: u8, want: u8, next: Step, context: &str) -> Op {
    if c == want {
        s.step = next;
        return Op::Continue;
    }
    s.error(c, context)
}

/// Go `stateT` (`scanner.go:497`).
fn state_t(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b'r', state_tr, "in literal true (expecting 'r')")
}

/// Go `stateTr` (`scanner.go:506`).
fn state_tr(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b'u', state_tru, "in literal true (expecting 'u')")
}

/// Go `stateTru` (`scanner.go:515`).
fn state_tru(s: &mut Scanner, c: u8) -> Op {
    literal_step(
        s,
        c,
        b'e',
        state_end_value,
        "in literal true (expecting 'e')",
    )
}

/// Go `stateF` (`scanner.go:524`).
fn state_f(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b'a', state_fa, "in literal false (expecting 'a')")
}

/// Go `stateFa` (`scanner.go:533`).
fn state_fa(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b'l', state_fal, "in literal false (expecting 'l')")
}

/// Go `stateFal` (`scanner.go:542`).
fn state_fal(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b's', state_fals, "in literal false (expecting 's')")
}

/// Go `stateFals` (`scanner.go:551`).
fn state_fals(s: &mut Scanner, c: u8) -> Op {
    literal_step(
        s,
        c,
        b'e',
        state_end_value,
        "in literal false (expecting 'e')",
    )
}

/// Go `stateN` (`scanner.go:560`).
fn state_n(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b'u', state_nu, "in literal null (expecting 'u')")
}

/// Go `stateNu` (`scanner.go:569`).
fn state_nu(s: &mut Scanner, c: u8) -> Op {
    literal_step(s, c, b'l', state_nul, "in literal null (expecting 'l')")
}

/// Go `stateNul` (`scanner.go:578`).
fn state_nul(s: &mut Scanner, c: u8) -> Op {
    literal_step(
        s,
        c,
        b'l',
        state_end_value,
        "in literal null (expecting 'l')",
    )
}

/// Go `stateError` (`scanner.go:588`).
fn state_error(_s: &mut Scanner, _c: u8) -> Op {
    Op::Error
}

/// Go `quoteChar` (`scanner.go:600`): c as a quoted character literal.
fn quote_char(c: u8) -> String {
    // special cases - different from quoted strings
    if c == b'\'' {
        return r"'\''".to_string();
    }
    if c == b'"' {
        return r#"'"'"#.to_string();
    }
    // use quoted string with different quotation marks; Go's string(c) is the rune c
    let mut buf = [0u8; 4];
    let s = crate::strconv::quote(char::from(c).encode_utf8(&mut buf).as_bytes());
    format!("'{}'", &s[1..s.len() - 1])
}
