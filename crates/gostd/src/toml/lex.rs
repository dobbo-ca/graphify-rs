//! BurntSushi/toml v1.4.0 `lex.go`, TOML 1.0 only (`lx.tomlNext` is always false). The Go lexer is
//! a state machine of `stateFn`s that sends items over a channel the parser drains one at a time;
//! here each `stateFn` is a `St`, the channel is a queue, and `next_item` runs states only while
//! the queue is empty, so a lexer error beyond the first parser error is never reached (as in Go).
//! Only the line of an item's `Position` is kept: it is the only part `ParseError.Error()` prints.

use crate::strconv;
use crate::unicode;
use crate::utf8::{RUNE_ERROR, decode_rune};
use std::collections::VecDeque;

/// lex.go:47 `const eof = 0`.
pub(super) const EOF: char = '\0';

/// lex.go:13 `itemType` (the types the TOML 1.0 lexer emits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemType {
    Error,
    Eof,
    Text,
    String,
    StringEsc,
    RawString,
    MultilineString,
    RawMultilineString,
    Bool,
    Integer,
    Float,
    Datetime,
    Array,
    ArrayEnd,
    TableStart,
    TableEnd,
    ArrayTableStart,
    ArrayTableEnd,
    KeyStart,
    KeyEnd,
    CommentStart,
    InlineTableStart,
    InlineTableEnd,
}

/// lex.go:73 `item`. For `ItemType::Error`, `val` is the error text (Go `item.err.Error()`).
#[derive(Debug, Clone)]
pub(super) struct Item {
    pub typ: ItemType,
    pub val: String,
    /// Go `item.pos.Line`.
    pub line: usize,
}

/// The `stateFn`s of lex.go. A `None` state is Go's `nil` (the lexer is done or failed).
#[derive(Debug, Clone, Copy)]
enum St {
    Top,
    TopEnd,
    TableStart,
    TableEnd,
    ArrayTableEnd,
    TableNameStart,
    TableNameEnd,
    BareName,
    QuotedName,
    KeyStart,
    KeyNameStart,
    KeyEnd,
    Value,
    ArrayValue,
    ArrayValueEnd,
    ArrayEnd,
    InlineTableValue,
    InlineTableValueEnd,
    InlineTableEnd,
    Str,
    MultilineString,
    RawString,
    MultilineRawString,
    MultilineStringEscape,
    StringEscape,
    ShortUnicodeEscape,
    LongUnicodeEscape,
    NumberOrDateStart,
    NumberOrDate,
    Datetime,
    HexInteger,
    OctalInteger,
    BinaryInteger,
    DecimalNumber,
    DecimalNumberStart,
    BaseNumberOrDate,
    Float,
    Bool,
    CommentStart,
    Comment,
}

/// lex.go:56 `lexer`. `input` is Go's `string`: bytes that may be invalid UTF-8.
pub(super) struct Lexer<'a> {
    input: &'a [u8],
    start: usize,
    pos: usize,
    line: usize,
    state: Option<St>,
    items: VecDeque<Item>,
    esc: bool,
    /// Widths of the last 4 runes read (lex.go:63), `prev_widths[0]` the most recent.
    prev_widths: [usize; 4],
    nprev: usize,
    at_eof: bool,
    stack: Vec<St>,
}

/// `%q` of a rune (strconv.QuoteRune). `strconv::quote` of the one rune differs only in which
/// quote it escapes.
fn quote_rune(r: char) -> String {
    let mut buf = [0u8; 4];
    let q = strconv::quote(r.encode_utf8(&mut buf).as_bytes());
    let inner = &q[1..q.len() - 1];
    match r {
        '"' => "'\"'".to_string(),
        '\'' => "'\\''".to_string(),
        _ => format!("'{inner}'"),
    }
}

/// lex.go:1206 `isWhitespace`.
fn is_whitespace(r: char) -> bool {
    r == '\t' || r == ' '
}

/// lex.go:1207 `isNL`.
fn is_nl(r: char) -> bool {
    r == '\n' || r == '\r'
}

/// lex.go:1208 `isControl`: control characters except \t, \r, \n.
fn is_control(r: char) -> bool {
    match r {
        '\t' | '\r' | '\n' => false,
        _ => r <= '\x1f' || r == '\x7f',
    }
}

/// lex.go:1216 `isDigit`.
fn is_digit(r: char) -> bool {
    r.is_ascii_digit()
}

/// lex.go:1217 `isBinary`.
fn is_binary(r: char) -> bool {
    r == '0' || r == '1'
}

/// lex.go:1218 `isOctal`.
fn is_octal(r: char) -> bool {
    ('0'..='7').contains(&r)
}

/// lex.go:1219 `isHex`: `r|0x20` folds 'A'-'F' onto 'a'-'f'.
pub(super) fn is_hex(r: char) -> bool {
    r.is_ascii_digit() || {
        let l = u32::from(r) | 0x20;
        (u32::from('a')..=u32::from('f')).contains(&l)
    }
}

/// lex.go:1220 `isBareKeyChar` with `tomlNext` false.
fn is_bare_key_char(r: char) -> bool {
    r.is_ascii_alphanumeric() || r == '_' || r == '-'
}

/// lex.go:1126 `runeOrEOF`.
fn rune_or_eof(r: char) -> String {
    if r == EOF {
        "end of file".to_string()
    } else {
        format!("'{r}'")
    }
}

impl<'a> Lexer<'a> {
    /// lex.go:93 `lex`.
    pub(super) fn new(input: &'a [u8]) -> Self {
        Lexer {
            input,
            start: 0,
            pos: 0,
            line: 1,
            state: Some(St::Top),
            items: VecDeque::new(),
            esc: false,
            prev_widths: [0; 4],
            nprev: 0,
            at_eof: false,
            stack: Vec::new(),
        }
    }

    /// lex.go:80 `nextItem`: run states until one has queued an item. A `nil` state with nothing
    /// queued is a Go nil-func panic; the parser never asks again after an error or EOF, so this
    /// reports it as an error item instead.
    pub(super) fn next_item(&mut self) -> Item {
        loop {
            if let Some(it) = self.items.pop_front() {
                return it;
            }
            match self.state {
                Some(st) => self.state = self.run(st),
                None => {
                    return Item {
                        typ: ItemType::Error,
                        val: "BUG: lexer has no state".to_string(),
                        line: self.line,
                    };
                }
            }
        }
    }

    /// lex.go:109 `push`.
    fn push(&mut self, st: St) {
        self.stack.push(st);
    }

    /// lex.go:113 `pop`.
    fn pop(&mut self) -> Option<St> {
        self.stack
            .pop()
            .or_else(|| self.errorf("BUG in lexer: no states to pop".to_string()))
    }

    /// lex.go:122 `current`. The slice is empty when `start > pos` (Go would panic).
    fn current_bytes(&self) -> &'a [u8] {
        self.input.get(self.start..self.pos).unwrap_or(&[])
    }

    fn current(&self) -> String {
        String::from_utf8_lossy(self.current_bytes()).into_owned()
    }

    /// lex.go:145 `emit`.
    fn emit(&mut self, typ: ItemType) {
        // Needed for multiline strings ending with an incomplete UTF-8 sequence.
        if self.start > self.pos {
            let b = self.input.get(self.pos).copied().unwrap_or(0);
            self.errorf(format!("invalid UTF-8 byte: 0x{b:02x}"));
            return;
        }
        let val = self.current();
        self.items.push_back(Item {
            typ,
            val,
            line: self.line,
        });
        self.start = self.pos;
    }

    /// lex.go:154 `emitTrim`.
    fn emit_trim(&mut self, typ: ItemType) {
        let val = self.current().trim().to_string();
        self.items.push_back(Item {
            typ,
            val,
            line: self.line,
        });
        self.start = self.pos;
    }

    /// lex.go:159 `next`. Returns `EOF` at the end (and again if called twice: Go panics).
    fn next(&mut self) -> char {
        if self.at_eof {
            return EOF;
        }
        if self.pos >= self.input.len() {
            self.at_eof = true;
            return EOF;
        }

        if self.input[self.pos] == b'\n' {
            self.line += 1;
        }
        self.prev_widths[3] = self.prev_widths[2];
        self.prev_widths[2] = self.prev_widths[1];
        self.prev_widths[1] = self.prev_widths[0];
        if self.nprev < 4 {
            self.nprev += 1;
        }

        let (r, w) = decode_rune(&self.input[self.pos..]);
        if r == RUNE_ERROR && w == 1 {
            let b = self.input[self.pos];
            self.errorf(format!("invalid UTF-8 byte: 0x{b:02x}"));
            return RUNE_ERROR;
        }

        // Note: don't use peek() here, as this calls next().
        if is_control(r)
            || (r == '\r'
                && (self.input.len() - 1 == self.pos || self.input[self.pos + 1] != b'\n'))
        {
            self.error_control_char(r);
            return RUNE_ERROR;
        }

        self.prev_widths[0] = w;
        self.pos += w;
        r
    }

    /// lex.go:196 `ignore`.
    fn ignore(&mut self) {
        self.start = self.pos;
    }

    /// lex.go:201 `backup`: up to 4 runes between calls to `next`.
    fn backup(&mut self) {
        if self.at_eof {
            self.at_eof = false;
            return;
        }
        if self.nprev < 1 {
            return; // Go panics: "BUG in lexer: backed up too far"
        }
        let w = self.prev_widths[0];
        self.prev_widths[0] = self.prev_widths[1];
        self.prev_widths[1] = self.prev_widths[2];
        self.prev_widths[2] = self.prev_widths[3];
        self.nprev -= 1;

        self.pos = self.pos.saturating_sub(w);
        if self.pos < self.input.len() && self.input[self.pos] == b'\n' {
            self.line = self.line.saturating_sub(1);
        }
    }

    /// lex.go:223 `accept`.
    fn accept(&mut self, valid: char) -> bool {
        if self.next() == valid {
            return true;
        }
        self.backup();
        false
    }

    /// lex.go:231 `peek`.
    fn peek(&mut self) -> char {
        let r = self.next();
        self.backup();
        r
    }

    /// lex.go:238 `skip`.
    fn skip(&mut self, pred: fn(char) -> bool) {
        loop {
            let r = self.next();
            if pred(r) {
                continue;
            }
            self.backup();
            self.ignore();
            return;
        }
    }

    /// lex.go:257 `error` and lex.go:293 `errorf`: at EOF the position is the last column of the
    /// previous line.
    fn errorf(&mut self, msg: String) -> Option<St> {
        let line = if self.at_eof {
            self.line.saturating_sub(1)
        } else {
            self.line
        };
        self.items.push_back(Item {
            typ: ItemType::Error,
            val: msg,
            line,
        });
        None
    }

    /// lex.go:269 `errorPrevLine`.
    fn error_prev_line(&mut self, msg: String) -> Option<St> {
        self.items.push_back(Item {
            typ: ItemType::Error,
            val: msg,
            line: self.line.saturating_sub(1),
        });
        None
    }

    /// lex.go:305 `errorControlChar` through `errorPos` (lex.go:281): the current line.
    fn error_control_char(&mut self, cc: char) -> Option<St> {
        self.items.push_back(Item {
            typ: ItemType::Error,
            val: format!(
                "TOML files cannot contain control characters: '0x{:02x}'",
                u32::from(cc)
            ),
            line: self.line,
        });
        None
    }

    /// lex.go:1196 `lexSkip`.
    fn skip_to(&mut self, next: St) -> Option<St> {
        self.ignore();
        Some(next)
    }

    fn run(&mut self, st: St) -> Option<St> {
        match st {
            St::Top => self.lex_top(),
            St::TopEnd => self.lex_top_end(),
            St::TableStart => self.lex_table_start(),
            St::TableEnd => {
                // lex.go:372 `lexTableEnd`
                self.emit(ItemType::TableEnd);
                Some(St::TopEnd)
            }
            St::ArrayTableEnd => {
                // lex.go:377 `lexArrayTableEnd`
                let r = self.next();
                if r != ']' {
                    return self.errorf(format!(
                        "expected end of table array name delimiter ']', but got {} instead",
                        quote_rune(r)
                    ));
                }
                self.emit(ItemType::ArrayTableEnd);
                Some(St::TopEnd)
            }
            St::TableNameStart => self.lex_table_name_start(),
            St::TableNameEnd => self.lex_table_name_end(),
            St::BareName => {
                // lex.go:422 `lexBareName`
                let r = self.next();
                if is_bare_key_char(r) {
                    return Some(St::BareName);
                }
                self.backup();
                self.emit(ItemType::Text);
                self.pop()
            }
            St::QuotedName => self.lex_quoted_name(),
            St::KeyStart => self.lex_key_start(),
            St::KeyNameStart => self.lex_key_name_start(),
            St::KeyEnd => self.lex_key_end(),
            St::Value => self.lex_value(),
            St::ArrayValue => self.lex_array_value(),
            St::ArrayValueEnd => self.lex_array_value_end(),
            St::ArrayEnd => {
                // lex.go:635 `lexArrayEnd`
                self.ignore();
                self.emit(ItemType::ArrayEnd);
                self.pop()
            }
            St::InlineTableValue => self.lex_inline_table_value(),
            St::InlineTableValueEnd => self.lex_inline_table_value_end(),
            St::InlineTableEnd => {
                // lex.go:704 `lexInlineTableEnd`
                self.ignore();
                self.emit(ItemType::InlineTableEnd);
                self.pop()
            }
            St::Str => self.lex_string(),
            St::MultilineString => self.lex_multiline_string(),
            St::RawString => self.lex_raw_string(),
            St::MultilineRawString => self.lex_multiline_raw_string(),
            St::MultilineStringEscape => {
                // lex.go:845 `lexMultilineStringEscape`
                if is_nl(self.next()) {
                    // \ escaping newline.
                    return Some(St::MultilineString);
                }
                self.backup();
                self.push(St::MultilineString);
                self.lex_string_escape()
            }
            St::StringEscape => self.lex_string_escape(),
            St::ShortUnicodeEscape => self.lex_unicode_escape(4, 'u', "four"),
            St::LongUnicodeEscape => self.lex_unicode_escape(8, 'U', "eight"),
            St::NumberOrDateStart => {
                // lex.go:926 `lexNumberOrDateStart`
                let r = self.next();
                if r == '0' {
                    return Some(St::BaseNumberOrDate);
                }
                if !is_digit(r) {
                    // The only way to reach this state is if the value starts with a digit, so
                    // specifically treat anything else as an error.
                    return self.errorf(format!("expected a digit but got {}", quote_rune(r)));
                }
                Some(St::NumberOrDate)
            }
            St::NumberOrDate => {
                // lex.go:944 `lexNumberOrDate`
                let r = self.next();
                if is_digit(r) {
                    return Some(St::NumberOrDate);
                }
                match r {
                    '-' | ':' => return Some(St::Datetime),
                    '_' => return Some(St::DecimalNumber),
                    '.' | 'e' | 'E' => return Some(St::Float),
                    _ => {}
                }
                self.backup();
                self.emit(ItemType::Integer);
                self.pop()
            }
            St::Datetime => {
                // lex.go:965 `lexDatetime`
                let r = self.next();
                if is_digit(r) {
                    return Some(St::Datetime);
                }
                if matches!(r, '-' | ':' | 'T' | 't' | ' ' | '.' | 'Z' | 'z' | '+') {
                    return Some(St::Datetime);
                }
                self.backup();
                self.emit_trim(ItemType::Datetime);
                self.pop()
            }
            St::HexInteger => self.lex_integer(St::HexInteger, is_hex),
            St::OctalInteger => self.lex_integer(St::OctalInteger, is_octal),
            St::BinaryInteger => self.lex_integer(St::BinaryInteger, is_binary),
            St::DecimalNumber => {
                // lex.go:1041 `lexDecimalNumber`
                let r = self.next();
                if is_digit(r) {
                    return Some(St::DecimalNumber);
                }
                match r {
                    '.' | 'e' | 'E' => return Some(St::Float),
                    '_' => return Some(St::DecimalNumber),
                    _ => {}
                }
                self.backup();
                self.emit(ItemType::Integer);
                self.pop()
            }
            St::DecimalNumberStart => self.lex_decimal_number_start(),
            St::BaseNumberOrDate => self.lex_base_number_or_date(),
            St::Float => {
                // lex.go:1139 `lexFloat`
                let r = self.next();
                if is_digit(r) || matches!(r, '_' | '.' | '-' | '+' | 'e' | 'E') {
                    return Some(St::Float);
                }
                self.backup();
                self.emit(ItemType::Float);
                self.pop()
            }
            St::Bool => self.lex_bool(),
            St::CommentStart => {
                // lex.go:1171 `lexCommentStart`
                self.ignore();
                self.emit(ItemType::CommentStart);
                Some(St::Comment)
            }
            St::Comment => {
                // lex.go:1181 `lexComment`
                let r = self.next();
                if is_nl(r) || r == EOF {
                    self.backup();
                    self.emit(ItemType::Text);
                    return self.pop();
                }
                Some(St::Comment)
            }
        }
    }

    /// lex.go:318 `lexTop`: elements at the top level.
    fn lex_top(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) || is_nl(r) {
            return self.skip_to(St::Top);
        }
        match r {
            '#' => {
                self.push(St::Top);
                return Some(St::CommentStart);
            }
            '[' => return Some(St::TableStart),
            EOF => {
                if self.pos > self.start {
                    return self.errorf("unexpected EOF".to_string());
                }
                self.emit(ItemType::Eof);
                return None;
            }
            _ => {}
        }

        // At this point, the only valid item can be a key, so we back up and let the key lexer do
        // the rest.
        self.backup();
        self.push(St::TopEnd);
        Some(St::KeyStart)
    }

    /// lex.go:346 `lexTopEnd`.
    fn lex_top_end(&mut self) -> Option<St> {
        let r = self.next();
        if r == '#' {
            // a comment will read to a newline for us.
            self.push(St::Top);
            return Some(St::CommentStart);
        }
        if is_whitespace(r) {
            return Some(St::TopEnd);
        }
        if is_nl(r) {
            self.ignore();
            return Some(St::Top);
        }
        if r == EOF {
            self.emit(ItemType::Eof);
            return None;
        }
        self.errorf(format!(
            "expected a top-level item to end with a newline, comment, or EOF, but got {} instead",
            quote_rune(r)
        ))
    }

    /// lex.go:363 `lexTableStart`: '[' has been consumed.
    fn lex_table_start(&mut self) -> Option<St> {
        if self.peek() == '[' {
            self.next();
            self.emit(ItemType::ArrayTableStart);
            self.push(St::ArrayTableEnd);
        } else {
            self.emit(ItemType::TableStart);
            self.push(St::TableEnd);
        }
        Some(St::TableNameStart)
    }

    /// lex.go:386 `lexTableNameStart`.
    fn lex_table_name_start(&mut self) -> Option<St> {
        self.skip(is_whitespace);
        let r = self.peek();
        if r == ']' || r == EOF {
            self.errorf("unexpected end of table name (table names cannot be empty)".to_string())
        } else if r == '.' {
            self.errorf("unexpected table separator (table names cannot be empty)".to_string())
        } else if r == '"' || r == '\'' {
            self.ignore();
            self.push(St::TableNameEnd);
            Some(St::QuotedName)
        } else {
            self.push(St::TableNameEnd);
            Some(St::BareName)
        }
    }

    /// lex.go:403 `lexTableNameEnd`.
    fn lex_table_name_end(&mut self) -> Option<St> {
        self.skip(is_whitespace);
        let r = self.next();
        if is_whitespace(r) {
            Some(St::TableNameEnd)
        } else if r == '.' {
            self.ignore();
            Some(St::TableNameStart)
        } else if r == ']' {
            self.pop()
        } else {
            self.errorf(format!(
                "expected '.' or ']' to end table name, but got {} instead",
                quote_rune(r)
            ))
        }
    }

    /// lex.go:440 `lexQuotedName`.
    fn lex_quoted_name(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) {
            self.skip_to(St::Value)
        } else if r == '"' {
            self.ignore(); // ignore the '"'
            Some(St::Str)
        } else if r == '\'' {
            self.ignore(); // ignore the "'"
            Some(St::RawString)
        } else if r == EOF {
            self.errorf("unexpected EOF; expected value".to_string())
        } else {
            self.errorf(format!(
                "expected value but found {} instead",
                quote_rune(r)
            ))
        }
    }

    /// lex.go:457 `lexKeyStart`: all key parts until a '='.
    fn lex_key_start(&mut self) -> Option<St> {
        self.skip(is_whitespace);
        let r = self.peek();
        if r == '=' || r == EOF {
            return self.errorf("unexpected '=': key name appears blank".to_string());
        }
        if r == '.' {
            return self.errorf("unexpected '.': keys cannot start with a '.'".to_string());
        }
        if r == '"' || r == '\'' {
            self.ignore();
        }
        // Bare key (or the quoted key after the ignore).
        self.emit(ItemType::KeyStart);
        Some(St::KeyNameStart)
    }

    /// lex.go:472 `lexKeyNameStart`.
    fn lex_key_name_start(&mut self) -> Option<St> {
        self.skip(is_whitespace);
        let r = self.peek();
        if r == '=' || r == EOF {
            self.errorf("unexpected '='".to_string())
        } else if r == '.' {
            self.errorf("unexpected '.'".to_string())
        } else if r == '"' || r == '\'' {
            self.ignore();
            self.push(St::KeyEnd);
            Some(St::QuotedName)
        } else {
            self.push(St::KeyEnd);
            Some(St::BareName)
        }
    }

    /// lex.go:490 `lexKeyEnd`: the end of a key, up to the key separator.
    fn lex_key_end(&mut self) -> Option<St> {
        self.skip(is_whitespace);
        let r = self.next();
        if is_whitespace(r) {
            self.skip_to(St::KeyEnd)
        } else if r == EOF {
            self.errorf("unexpected EOF; expected key separator '='".to_string())
        } else if r == '.' {
            self.ignore();
            Some(St::KeyNameStart)
        } else if r == '=' {
            self.emit(ItemType::KeyEnd);
            self.skip_to(St::Value)
        } else {
            self.errorf(format!(
                "expected '.' or '=', but got {} instead",
                quote_rune(r)
            ))
        }
    }

    /// lex.go:509 `lexValue`: whitespace is ignored but NOT newlines.
    fn lex_value(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) {
            return self.skip_to(St::Value);
        }
        if is_digit(r) {
            self.backup(); // avoid an extra state and use the same as above
            return Some(St::NumberOrDateStart);
        }
        match r {
            '[' => {
                self.ignore();
                self.emit(ItemType::Array);
                return Some(St::ArrayValue);
            }
            '{' => {
                self.ignore();
                self.emit(ItemType::InlineTableStart);
                return Some(St::InlineTableValue);
            }
            '"' => {
                if self.accept('"') {
                    if self.accept('"') {
                        self.ignore(); // Ignore """
                        return Some(St::MultilineString);
                    }
                    self.backup();
                }
                self.ignore(); // ignore the '"'
                return Some(St::Str);
            }
            '\'' => {
                if self.accept('\'') {
                    if self.accept('\'') {
                        self.ignore(); // Ignore '''
                        return Some(St::MultilineRawString);
                    }
                    self.backup();
                }
                self.ignore(); // ignore the "'"
                return Some(St::RawString);
            }
            '.' => {
                // special error case, be kind to users
                return self.errorf("floats must start with a digit, not '.'".to_string());
            }
            'i' | 'n' => {
                if (self.accept('n') && self.accept('f')) || (self.accept('a') && self.accept('n'))
                {
                    self.emit(ItemType::Float);
                    return self.pop();
                }
            }
            '-' | '+' => return Some(St::DecimalNumberStart),
            _ => {}
        }
        if unicode::is_letter(r) {
            // Be permissive here; lexBool will give a nice error if the user wrote something like
            // `x = foo`.
            self.backup();
            return Some(St::Bool);
        }
        if r == EOF {
            return self.errorf("unexpected EOF; expected value".to_string());
        }
        self.errorf(format!(
            "expected value but found {} instead",
            quote_rune(r)
        ))
    }

    /// lex.go:575 `lexArrayValue`: '[' or ',' has been consumed; whitespace and newlines ignored.
    fn lex_array_value(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) || is_nl(r) {
            return self.skip_to(St::ArrayValue);
        }
        if r == '#' {
            self.push(St::ArrayValue);
            return Some(St::CommentStart);
        }
        if r == ',' {
            return self.errorf("unexpected comma".to_string());
        }
        if r == ']' {
            return Some(St::ArrayEnd);
        }

        self.backup();
        self.push(St::ArrayValueEnd);
        Some(St::Value)
    }

    /// lex.go:597 `lexArrayValueEnd`.
    fn lex_array_value_end(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) || is_nl(r) {
            self.skip_to(St::ArrayValueEnd)
        } else if r == '#' {
            self.push(St::ArrayValueEnd);
            Some(St::CommentStart)
        } else if r == ',' {
            self.ignore();
            Some(St::ArrayValue) // move on to the next value
        } else if r == ']' {
            Some(St::ArrayEnd)
        } else {
            self.errorf(format!(
                "expected a comma (',') or array terminator (']'), but got {}",
                rune_or_eof(r)
            ))
        }
    }

    /// lex.go:643 `lexInlineTableValue`: one key/value pair; '{' or ',' has been consumed.
    fn lex_inline_table_value(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) {
            return self.skip_to(St::InlineTableValue);
        }
        if is_nl(r) {
            return self.error_prev_line("newlines not allowed within inline tables".to_string());
        }
        if r == '#' {
            self.push(St::InlineTableValue);
            return Some(St::CommentStart);
        }
        if r == ',' {
            return self.errorf("unexpected comma".to_string());
        }
        if r == '}' {
            return Some(St::InlineTableEnd);
        }
        self.backup();
        self.push(St::InlineTableValueEnd);
        Some(St::KeyStart)
    }

    /// lex.go:671 `lexInlineTableValueEnd`.
    fn lex_inline_table_value_end(&mut self) -> Option<St> {
        let r = self.next();
        if is_whitespace(r) {
            self.skip_to(St::InlineTableValueEnd)
        } else if is_nl(r) {
            self.error_prev_line("newlines not allowed within inline tables".to_string())
        } else if r == '#' {
            self.push(St::InlineTableValueEnd);
            Some(St::CommentStart)
        } else if r == ',' {
            self.ignore();
            self.skip(is_whitespace);
            if self.peek() == '}' {
                return self.errorf("trailing comma not allowed in inline tables".to_string());
            }
            Some(St::InlineTableValue)
        } else if r == '}' {
            Some(St::InlineTableEnd)
        } else {
            self.errorf(format!(
                "expected a comma or an inline table terminator '}}', but got {} instead",
                rune_or_eof(r)
            ))
        }
    }

    /// lex.go:711 `lexString`: the opening '"' has been consumed and ignored.
    fn lex_string(&mut self) -> Option<St> {
        let r = self.next();
        if r == EOF {
            return self.errorf("unexpected EOF; expected '\"'".to_string());
        }
        if is_nl(r) {
            return self.error_prev_line("strings cannot contain newlines".to_string());
        }
        if r == '\\' {
            self.push(St::Str);
            return Some(St::StringEscape);
        }
        if r == '"' {
            self.backup();
            if self.esc {
                self.esc = false;
                self.emit(ItemType::StringEsc);
            } else {
                self.emit(ItemType::String);
            }
            self.next();
            self.ignore();
            return self.pop();
        }
        Some(St::Str)
    }

    /// lex.go:737 `lexMultilineString`: the opening '"""' has been consumed and ignored.
    fn lex_multiline_string(&mut self) -> Option<St> {
        let r = self.next();
        match r {
            EOF => self.errorf("unexpected EOF; expected '\"\"\"'".to_string()),
            '\\' => Some(St::MultilineStringEscape),
            '"' => {
                // Found " -> try to read two more "".
                if self.accept('"') {
                    if self.accept('"') {
                        // Peek ahead: the string can contain " and "", including at the end:
                        // """str""""". 6 or more at the end, however, is an error.
                        if self.peek() == '"' {
                            // Check if we already lexed 5 "s; if so we have 6 now, and that's
                            // just too many. The second check is for the edge case
                            // `"""lol \""""""` (two quotes allowed after an escaped one).
                            let cur = self.current_bytes();
                            if cur.ends_with(b"\"\"\"\"\"") && !cur.ends_with(b"\\\"\"\"\"\"") {
                                return self.errorf("unexpected '\"\"\"\"\"\"'".to_string());
                            }
                            self.backup();
                            self.backup();
                            return Some(St::MultilineString);
                        }

                        self.backup(); // backup: don't include the """ in the item.
                        self.backup();
                        self.backup();
                        self.esc = false;
                        self.emit(ItemType::MultilineString);
                        self.next(); // Read over """ again and discard it.
                        self.next();
                        self.next();
                        self.ignore();
                        return self.pop();
                    }
                    self.backup();
                }
                Some(St::MultilineString)
            }
            _ => Some(St::MultilineString),
        }
    }

    /// lex.go:812 `lexRawString`: the opening "'" has been consumed and ignored.
    fn lex_raw_string(&mut self) -> Option<St> {
        let r = self.next();
        if r == EOF {
            self.errorf("unexpected EOF; expected \"'\"".to_string())
        } else if is_nl(r) {
            self.error_prev_line("strings cannot contain newlines".to_string())
        } else if r == '\'' {
            self.backup();
            self.emit(ItemType::RawString);
            self.next();
            self.ignore();
            self.pop()
        } else {
            Some(St::RawString)
        }
    }

    /// lex.go:832 `lexMultilineRawString`.
    fn lex_multiline_raw_string(&mut self) -> Option<St> {
        let r = self.next();
        match r {
            EOF => self.errorf("unexpected EOF; expected \"'''\"".to_string()),
            '\'' => {
                // Found ' -> try to read two more ''.
                if self.accept('\'') {
                    if self.accept('\'') {
                        // Peek ahead: the string can contain ' and '', including at the end:
                        // '''str'''''. 6 or more at the end, however, is an error.
                        if self.peek() == '\'' {
                            if self.current_bytes().ends_with(b"'''''") {
                                return self.errorf("unexpected \"''''''\"".to_string());
                            }
                            self.backup();
                            self.backup();
                            return Some(St::MultilineRawString);
                        }

                        self.backup(); // backup: don't include the ''' in the item.
                        self.backup();
                        self.backup();
                        self.emit(ItemType::RawMultilineString);
                        self.next(); // Read over ''' again and discard it.
                        self.next();
                        self.next();
                        self.ignore();
                        return self.pop();
                    }
                    self.backup();
                }
                Some(St::MultilineRawString)
            }
            _ => Some(St::MultilineRawString),
        }
    }

    /// lex.go:852 `lexStringEscape`: the preceding '\' has been consumed. TOML 1.0: no `\e`, no
    /// `\x`.
    fn lex_string_escape(&mut self) -> Option<St> {
        self.esc = true;
        let r = self.next();
        match r {
            // ' ' and '\t': inside """ .. """ strings you can use \ to escape newlines, and any
            // amount of whitespace can be between the \ and \n.
            'b' | 't' | 'n' | 'f' | 'r' | '"' | ' ' | '\t' | '\\' => self.pop(),
            'u' => Some(St::ShortUnicodeEscape),
            'U' => Some(St::LongUnicodeEscape),
            _ => self.errorf(format!("invalid escape in string '\\{r}'")),
        }
    }

    /// lex.go:895 `lexShortUnicodeEscape` and lex.go:906 `lexLongUnicodeEscape`.
    fn lex_unicode_escape(&mut self, digits: usize, tag: char, word: &str) -> Option<St> {
        for _ in 0..digits {
            let r = self.next();
            if !is_hex(r) {
                return self.errorf(format!(
                    "expected {word} hexadecimal digits after '\\{tag}', but got {} instead",
                    strconv::quote(self.current_bytes())
                ));
            }
        }
        self.pop()
    }

    /// lex.go:975 `lexHexInteger`, `lexOctalInteger` (:989) and `lexBinaryInteger` (:1004).
    fn lex_integer(&mut self, stay: St, is_digit_of_base: fn(char) -> bool) -> Option<St> {
        let r = self.next();
        if is_digit_of_base(r) || r == '_' {
            return Some(stay);
        }
        self.backup();
        self.emit(ItemType::Integer);
        self.pop()
    }

    /// lex.go:1073 `lexDecimalNumberStart`: the sign has been consumed.
    fn lex_decimal_number_start(&mut self) -> Option<St> {
        let r = self.next();

        // Special error cases to give users better error messages
        match r {
            'i' => {
                if !self.accept('n') || !self.accept('f') {
                    return self.errorf(format!("invalid float: '{}'", self.current()));
                }
                self.emit(ItemType::Float);
                return self.pop();
            }
            'n' => {
                if !self.accept('a') || !self.accept('n') {
                    return self.errorf(format!("invalid float: '{}'", self.current()));
                }
                self.emit(ItemType::Float);
                return self.pop();
            }
            '0' => {
                let p = self.peek();
                if matches!(p, 'b' | 'o' | 'x') {
                    return self.errorf(format!(
                        "cannot use sign with non-decimal numbers: '{}{p}'",
                        self.current()
                    ));
                }
            }
            '.' => return self.errorf("floats must start with a digit, not '.'".to_string()),
            _ => {}
        }

        if is_digit(r) {
            return Some(St::DecimalNumber);
        }

        self.errorf(format!("expected a digit but got {}", quote_rune(r)))
    }

    /// lex.go:1105 `lexBaseNumberOrDate`: the initial '0' has been consumed. The `errorf` calls
    /// without a `return` are as in Go: the error is queued, the state carries on.
    fn lex_base_number_or_date(&mut self) -> Option<St> {
        let r = self.next();
        // Note: All datetimes start with at least two digits, so we don't handle date characters
        // (':', '-', etc.) here.
        if is_digit(r) {
            return Some(St::NumberOrDate);
        }
        match r {
            '_' => {
                // Can only be decimal, because there can't be an underscore between the '0' and
                // the base designator, and dates can't contain underscores.
                return Some(St::DecimalNumber);
            }
            '.' | 'e' | 'E' => return Some(St::Float),
            'b' => {
                let p = self.peek();
                if !is_binary(p) {
                    self.errorf(format!("not a binary number: '{}{p}'", self.current()));
                }
                return Some(St::BinaryInteger);
            }
            'o' => {
                let p = self.peek();
                if !is_octal(p) {
                    self.errorf(format!("not an octal number: '{}{p}'", self.current()));
                }
                return Some(St::OctalInteger);
            }
            'x' => {
                let p = self.peek();
                if !is_hex(p) {
                    self.errorf(format!("not a hexidecimal number: '{}{p}'", self.current()));
                }
                return Some(St::HexInteger);
            }
            _ => {}
        }

        self.backup();
        self.emit(ItemType::Integer);
        self.pop()
    }

    /// lex.go:1157 `lexBool`: 'true' or 'false' (any run of letters is read for the error).
    fn lex_bool(&mut self) -> Option<St> {
        let mut s = String::new();
        loop {
            let r = self.next();
            if !unicode::is_letter(r) {
                self.backup();
                break;
            }
            s.push(r);
        }
        match s.as_str() {
            "true" | "false" => {
                self.emit(ItemType::Bool);
                self.pop()
            }
            _ => self.errorf(format!(
                "expected value but found {} instead",
                strconv::quote(s.as_bytes())
            )),
        }
    }
}
