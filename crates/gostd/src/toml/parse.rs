//! BurntSushi/toml v1.4.0 `parse.go` (and the `Decode` into `map[string]any` of `decode.go:136`,
//! where `unifyAnything` stores the parser's mapping as is), TOML 1.0 only. Go panics with a
//! `ParseError` and `parse` recovers it; here every step returns `Result`. Go's `p.bug` panics
//! (a non-`ParseError` panic crashes the program) are a `ParseError` whose message starts with
//! `BUG: `; none is reachable from input.
//!
//! The mapping is Go's `p.mapping`; Go maps are shared references, so every walk down the
//! context is a path walk from the root here. `topHash` of an inline table is built as in Go
//! beside the mapping (it is what ends up inside a `[]any`).

use super::lex::{Item, ItemType, Lexer, is_hex};
use super::{Datetime, DatetimeKind, ParseError, Table, Value};
use crate::strconv::{self, NumErrorKind};
use std::collections::{HashMap, HashSet};

/// Go `Key` (meta.go:92).
type Key = Vec<String>;

/// The four layouts of `dtTypes` (parse.go:269) in order; TOML 1.1 entries are skipped.
const LAYOUTS: [&[u8]; 4] = [
    b"2006-01-02T15:04:05.999999999Z07:00", // time.RFC3339Nano, time.Local
    b"2006-01-02T15:04:05.999999999",       // internal.LocalDatetime
    b"2006-01-02",                          // internal.LocalDate
    b"15:04:05.999999999",                  // internal.LocalTime
];

/// meta.go:96 `Key.String` with `dblQuotedReplacer` (encode.go:30).
pub(super) fn key_string(k: &[String]) -> String {
    let mut b = String::new();
    for (i, kk) in k.iter().enumerate() {
        if i > 0 {
            b.push('.');
        }
        if kk.is_empty() {
            b.push_str("\"\"");
        } else if kk
            .chars()
            .all(|r| r.is_ascii_alphanumeric() || r == '_' || r == '-')
        {
            b.push_str(kk);
        } else {
            b.push('"');
            for c in kk.chars() {
                match c {
                    '"' => b.push_str("\\\""),
                    '\\' => b.push_str("\\\\"),
                    '\u{8}' => b.push_str("\\b"),
                    '\t' => b.push_str("\\t"),
                    '\n' => b.push_str("\\n"),
                    '\u{c}' => b.push_str("\\f"),
                    '\r' => b.push_str("\\r"),
                    _ if c < ' ' || c == '\u{7f}' => {
                        b.push_str(&format!("\\u{:04x}", u32::from(c)));
                    }
                    _ => b.push(c),
                }
            }
            b.push('"');
        }
    }
    b
}

/// parse.go:487 `current`: the full key name of the current context.
fn current_of(context: &[String], current_key: &str) -> String {
    if current_key.is_empty() {
        return key_string(context);
    }
    if context.is_empty() {
        return current_key.to_string();
    }
    format!("{}.{}", key_string(context), current_key)
}

/// A `ParseError` whose `LastKey` is `p.current()` (parse.go:78 `panicf`).
fn mk_err(msg: String, line: usize, context: &[String], current_key: &str) -> ParseError {
    ParseError {
        message: msg,
        line,
        last_key: current_of(context, current_key),
    }
}

/// parse.go:36 `parse`, then `Decode` (decode.go:167).
pub(super) fn parse(data: &[u8]) -> Result<Table, ParseError> {
    // Read over BOM (parse.go:50): UTF-16 BOM isn't strictly valid, but some tools add it anyway.
    let data = if let Some(r) = data
        .strip_prefix(b"\xff\xfe")
        .or_else(|| data.strip_prefix(b"\xfe\xff"))
    {
        r
    } else if let Some(r) = data.strip_prefix(b"\xef\xbb\xbf") {
        r
    } else {
        data
    };

    // Examine first few bytes for NULL bytes; this probably means it's a UTF-16 file (parse.go:60).
    let ex = data.len().min(6);
    if data[..ex].contains(&0) {
        return Err(ParseError {
            message:
                "files cannot contain NULL bytes; probably using UTF-16; TOML files must be UTF-8"
                    .to_string(),
            line: 1,
            last_key: String::new(),
        });
    }

    let mut p = Parser {
        lx: Lexer::new(data),
        context: Key::new(),
        current_key: String::new(),
        pos_line: 0,
        key_is_array: HashMap::new(),
        mapping: Table::new(),
        implicits: HashSet::new(),
    };
    loop {
        let item = p.next()?;
        if item.typ == ItemType::Eof {
            break;
        }
        p.top_level(item)?;
    }
    Ok(p.mapping)
}

/// parse.go:14 `parser`. `key_is_array` is `keyInfo` reduced to what `isArray` reads
/// (`tomlType == tomlArray`); the key positions and `ordered` are never read.
struct Parser<'a> {
    lx: Lexer<'a>,
    /// Full key for the current hash in scope.
    context: Key,
    /// Base key name for everything except hashes.
    current_key: String,
    /// Line of the current position in the TOML file (`p.pos.Line`).
    pos_line: usize,
    key_is_array: HashMap<String, bool>,
    mapping: Table,
    /// Implicit keys (e.g. "key.group.names").
    implicits: HashSet<String>,
}

impl Parser<'_> {
    /// parse.go:69 `panicItemf` / `panicErr`.
    fn err_item(&self, line: usize, msg: String) -> ParseError {
        mk_err(msg, line, &self.context, &self.current_key)
    }

    /// parse.go:78 `panicf`.
    fn err_pos(&self, msg: String) -> ParseError {
        self.err_item(self.pos_line, msg)
    }

    /// parse.go:128 `bug`.
    fn bug(&self, msg: String) -> ParseError {
        self.err_pos(format!("BUG: {msg}"))
    }

    /// parse.go:87 `next`: a lexer error item is a `ParseError`.
    fn next(&mut self) -> Result<Item, ParseError> {
        let it = self.lx.next_item();
        if it.typ == ItemType::Error {
            return Err(self.err_item(it.line, it.val));
        }
        Ok(it)
    }

    /// parse.go:103 `nextPos`.
    fn next_pos(&mut self) -> Result<Item, ParseError> {
        let it = self.next()?;
        self.pos_line = it.line;
        Ok(it)
    }

    /// parse.go:132 `expect` and `assertEqual`.
    fn expect(&mut self, typ: ItemType) -> Result<Item, ParseError> {
        let it = self.next()?;
        self.assert_equal(typ, it.typ)?;
        Ok(it)
    }

    fn assert_equal(&self, expected: ItemType, got: ItemType) -> Result<(), ParseError> {
        if expected != got {
            return Err(self.bug(format!("Expected '{expected:?}' but got '{got:?}'.")));
        }
        Ok(())
    }

    /// parse.go:143 `topLevel`.
    fn top_level(&mut self, item: Item) -> Result<(), ParseError> {
        match item.typ {
            ItemType::CommentStart => {
                // # ..
                self.expect(ItemType::Text)?;
            }
            ItemType::TableStart => {
                // [ .. ]
                let key = self.header_key(ItemType::TableEnd)?;
                self.add_context(&key, false)?;
                self.set_type("", false);
            }
            ItemType::ArrayTableStart => {
                // [[ .. ]]
                let key = self.header_key(ItemType::ArrayTableEnd)?;
                self.add_context(&key, true)?;
                self.set_type("", false);
            }
            ItemType::KeyStart => {
                // key = ..
                let outer_context = self.context.clone();
                let key = self.key_parts()?;
                let Some((last, parents)) = key.split_last() else {
                    return Err(self.bug("empty key".to_string()));
                };

                // The current key is the last part.
                self.current_key = last.clone();

                // All the other parts (if any) are the context; need to set each part as
                // implicit.
                for part in parents {
                    let mut kc = self.context.clone();
                    kc.push(part.clone());
                    self.add_implicit_context(&kc)?;
                }

                // Set value.
                let v_item = self.next()?;
                let (val, is_array) = self.value(&v_item, false)?;
                self.set_value(last, val)?;
                self.set_type(last, is_array);

                // Remove the context we added (preserving any context from [tbl] lines).
                self.context = outer_context;
                self.current_key.clear();
            }
            other => return Err(self.bug(format!("Unexpected type at top level: {other:?}"))),
        }
        Ok(())
    }

    /// The name of a `[..]` or `[[..]]` header: key parts up to `end` (parse.go:147, :162).
    fn header_key(&mut self, end: ItemType) -> Result<Key, ParseError> {
        let mut name = self.next_pos()?;
        let mut key = Key::new();
        while name.typ != end && name.typ != ItemType::Eof {
            key.push(self.key_string_of(&name)?);
            name = self.next()?;
        }
        self.assert_equal(end, name.typ)?;
        Ok(key)
    }

    /// All the key parts of `a.b` up to `KeyEnd` (parse.go:177, :390).
    fn key_parts(&mut self) -> Result<Key, ParseError> {
        let mut k = self.next_pos()?;
        let mut key = Key::new();
        while k.typ != ItemType::KeyEnd && k.typ != ItemType::Eof {
            key.push(self.key_string_of(&k)?);
            k = self.next()?;
        }
        self.assert_equal(ItemType::KeyEnd, k.typ)?;
        Ok(key)
    }

    /// parse.go:215 `keyString`: a string for a key (or part of a key in a table name).
    fn key_string_of(&mut self, it: &Item) -> Result<String, ParseError> {
        match it.typ {
            ItemType::Text => Ok(it.val.clone()),
            ItemType::String
            | ItemType::StringEsc
            | ItemType::MultilineString
            | ItemType::RawString
            | ItemType::RawMultilineString => match self.value(it, false)?.0 {
                Value::String(s) => Ok(s),
                _ => Err(self.bug("key is not a string".to_string())),
            },
            other => Err(self.bug(format!("Unexpected key type: {other:?}"))),
        }
    }

    /// parse.go:236 `value`: the Go value of an item and whether its tomlType is `tomlArray`.
    fn value(&mut self, it: &Item, parent_is_array: bool) -> Result<(Value, bool), ParseError> {
        match it.typ {
            ItemType::String | ItemType::RawString => Ok((Value::String(it.val.clone()), false)),
            ItemType::StringEsc => Ok((Value::String(self.replace_escapes(it, &it.val)?), false)),
            ItemType::MultilineString => {
                let s = strip_escaped_newlines(strip_first_newline(&it.val));
                Ok((Value::String(self.replace_escapes(it, &s)?), false))
            }
            ItemType::RawMultilineString => Ok((
                Value::String(strip_first_newline(&it.val).to_string()),
                false,
            )),
            ItemType::Integer => self.value_integer(it),
            ItemType::Float => self.value_float(it),
            ItemType::Bool => match it.val.as_str() {
                "true" => Ok((Value::Bool(true), false)),
                "false" => Ok((Value::Bool(false), false)),
                v => Err(self.bug(format!("Expected boolean value, but got '{v}'."))),
            },
            ItemType::Datetime => self.value_datetime(it),
            ItemType::Array => self.value_array(),
            ItemType::InlineTableStart => self.value_inline_table(parent_is_array),
            other => Err(self.bug(format!("Unexpected value type: {other:?}"))),
        }
    }

    /// parse.go:268 `valueInteger`: `strconv.ParseInt(.., 0, 64)` after the lexer's checks.
    fn value_integer(&self, it: &Item) -> Result<(Value, bool), ParseError> {
        let q = || strconv::quote(it.val.as_bytes());
        if !num_underscores_ok(&it.val) {
            return Err(self.err_item(
                it.line,
                format!(
                    "Invalid integer {}: underscores must be surrounded by digits",
                    q()
                ),
            ));
        }
        if num_has_leading_zero(&it.val) {
            return Err(self.err_item(
                it.line,
                format!("Invalid integer {}: cannot have leading zeroes", q()),
            ));
        }

        match strconv::parse_int(&it.val, 0, 64) {
            Ok(n) => Ok((Value::Integer(n), false)),
            // Out of range is a legitimate user error (errParseRange); the lexer cannot
            // provide any other bad integer.
            Err(e) if e.err == NumErrorKind::Range => {
                Err(self.err_item(it.line, format!("{} is out of range for int64", it.val)))
            }
            Err(_) => Err(self.bug(format!("Expected integer value, but got '{}'.", it.val))),
        }
    }

    /// parse.go:296 `valueFloat`.
    fn value_float(&self, it: &Item) -> Result<(Value, bool), ParseError> {
        let q = || strconv::quote(it.val.as_bytes());
        let parts: Vec<&str> = it
            .val
            .split(['.', 'e', 'E'])
            .filter(|p| !p.is_empty())
            .collect();
        for part in &parts {
            if !num_underscores_ok(part) {
                return Err(self.err_item(
                    it.line,
                    format!(
                        "Invalid float {}: underscores must be surrounded by digits",
                        q()
                    ),
                ));
            }
        }
        if let Some(first) = parts.first()
            && num_has_leading_zero(first)
        {
            return Err(self.err_item(
                it.line,
                format!("Invalid float {}: cannot have leading zeroes", q()),
            ));
        }
        if !num_periods_ok(&it.val) {
            // As a special case, numbers like '123.' or '1.e2', which are valid as far as
            // Go/strconv are concerned, must be rejected because TOML says that a fractional
            // part consists of '.' followed by 1+ digits.
            return Err(self.err_item(
                it.line,
                format!(
                    "Invalid float {}: '.' must be followed by one or more digits",
                    q()
                ),
            ));
        }
        let mut val = it.val.replace('_', "");
        let mut signbit = false;
        if val == "+nan" || val == "-nan" {
            signbit = val == "-nan";
            val = "nan".to_string();
        }
        match strconv::parse_float(&val) {
            Ok(mut num) => {
                if signbit {
                    num = num.copysign(-1.0);
                }
                Ok((Value::Float(num), false))
            }
            Err(e) if e.err == NumErrorKind::Range => {
                Err(self.err_item(it.line, format!("{} is out of range for float64", it.val)))
            }
            Err(_) => Err(self.err_item(it.line, format!("Invalid float value: {}", q()))),
        }
    }

    /// parse.go:327 `valueDatetime`.
    fn value_datetime(&self, it: &Item) -> Result<(Value, bool), ParseError> {
        let val = it.val.replace('z', "Z").replace(['t', ' '], "T"); // datetimeRepl
        for (idx, layout) in LAYOUTS.iter().enumerate() {
            if let Some(dt) = parse_layout(idx, val.as_bytes()) {
                if missing_leading_zero(val.as_bytes(), layout) {
                    break;
                }
                return Ok((Value::Datetime(dt), false));
            }
        }
        Err(self.err_item(
            it.line,
            format!("invalid datetime: {}", strconv::quote(val.as_bytes())),
        ))
    }

    /// parse.go:384 `valueArray`.
    fn value_array(&mut self) -> Result<(Value, bool), ParseError> {
        let key = self.current_key.clone();
        self.set_type(&key, true);

        let mut array = Vec::new();
        loop {
            let it = self.next()?;
            if it.typ == ItemType::ArrayEnd {
                break;
            }
            if it.typ == ItemType::CommentStart {
                self.expect(ItemType::Text)?;
                continue;
            }
            let (val, _) = self.value(&it, true)?;
            array.push(val);
        }
        Ok((Value::Array(array), true))
    }

    /// parse.go:419 `valueInlineTable`. `top` is Go's `topHash`.
    fn value_inline_table(&mut self, parent_is_array: bool) -> Result<(Value, bool), ParseError> {
        let mut top = Table::new();
        let outer_context = self.context.clone();
        let outer_key = self.current_key.clone();

        self.context.push(self.current_key.clone());
        let prev_context = self.context.clone();
        self.current_key.clear();

        self.implicits.insert(key_string(&prev_context));
        self.add_context(&prev_context, parent_is_array)?;

        // Loop over all table key/value pairs.
        loop {
            let it = self.next()?;
            if it.typ == ItemType::InlineTableEnd {
                break;
            }
            if it.typ == ItemType::CommentStart {
                self.expect(ItemType::Text)?;
                continue;
            }

            // Read all key parts.
            let key = self.key_parts()?;
            let Some((last, parents)) = key.split_last() else {
                return Err(self.bug("empty key".to_string()));
            };

            // The current key is the last part.
            self.current_key = last.clone();

            // All the other parts (if any) are the context; need to set each part as implicit.
            for part in parents {
                let mut kc = self.context.clone();
                kc.push(part.clone());
                self.add_implicit_context(&kc)?;
            }

            // Set the value.
            let v_item = self.next()?;
            let (val, is_array) = self.value(&v_item, false)?;
            self.set_value(last, val.clone())?;
            self.set_type(last, is_array);

            let mut hash = &mut top;
            for c in parents {
                let h = hash
                    .entry(c.clone())
                    .or_insert_with(|| Value::Table(Table::new()));
                match h {
                    Value::Table(t) => hash = t,
                    _ => {
                        return Err(self.err_pos(format!(
                            "{} is not a table",
                            strconv::quote(key_string(&self.context).as_bytes())
                        )));
                    }
                }
            }
            hash.insert(last.clone(), val);

            // Restore context.
            self.context = prev_context.clone();
        }
        self.context = outer_context;
        self.current_key = outer_key;
        Ok((Value::Table(top), false))
    }

    /// parse.go:513 `stripEscapedNewlines` and parse.go:535 `replaceEscapes`.
    fn replace_escapes(&self, it: &Item, s: &str) -> Result<String, ParseError> {
        let mut b = String::with_capacity(s.len());
        let mut skip = 0;
        let bytes = s.as_bytes();
        for (i, c) in s.char_indices() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            if c != '\\' {
                b.push(c);
                continue;
            }

            let Some(&esc) = bytes.get(i + 1) else {
                return Err(self.bug("Escape sequence at end of string.".to_string()));
            };
            match esc {
                b' ' | b'\t' => {
                    return Err(
                        self.err_item(it.line, format!("invalid escape: '\\{}'", char::from(esc)))
                    );
                }
                b'b' => {
                    b.push('\u{8}');
                    skip = 1;
                }
                b't' => {
                    b.push('\t');
                    skip = 1;
                }
                b'n' => {
                    b.push('\n');
                    skip = 1;
                }
                b'f' => {
                    b.push('\u{c}');
                    skip = 1;
                }
                b'r' => {
                    b.push('\r');
                    skip = 1;
                }
                // `\e` and `\x` are TOML 1.1 only (p.tomlNext): the lexer has refused them.
                b'e' | b'x' => {}
                b'"' => {
                    b.push('"');
                    skip = 1;
                }
                b'\\' => {
                    b.push('\\');
                    skip = 1;
                }
                // The lexer guarantees the correct number of characters are present.
                b'u' => {
                    b.push(self.ascii_escape_to_unicode(it, s.get(i + 2..i + 6))?);
                    skip = 5;
                }
                b'U' => {
                    b.push(self.ascii_escape_to_unicode(it, s.get(i + 2..i + 10))?);
                    skip = 9;
                }
                other => {
                    return Err(self.bug(format!(
                        "Expected valid escape code after \\, but got {}.",
                        strconv::quote(&[other])
                    )));
                }
            }
        }
        Ok(b)
    }

    /// parse.go:607 `asciiEscapeToUnicode`.
    fn ascii_escape_to_unicode(&self, it: &Item, s: Option<&str>) -> Result<char, ParseError> {
        let Some(s) = s else {
            return Err(self.bug("escape is too short".to_string()));
        };
        let Ok(hex) = strconv::parse_uint(&s.to_lowercase(), 16, 32) else {
            return Err(self.bug(format!(
                "Could not parse '{s}' as a hexadecimal number, but the lexer claims it's OK"
            )));
        };
        match u32::try_from(hex).ok().and_then(char::from_u32) {
            Some(c) => Ok(c),
            None => Err(self.err_item(
                it.line,
                format!("Escaped character '\\u{s}' is not valid UTF-8."),
            )),
        }
    }

    /// parse.go:575 `addContext`: set the current context of the parser, where the context is
    /// either a hash or an array of hashes, depending on `array`. Establishing the context also
    /// makes sure that the key isn't a duplicate, and will create implicit hashes automatically.
    fn add_context(&mut self, key: &[String], array: bool) -> Result<(), ParseError> {
        let Some((last, parents)) = key.split_last() else {
            return Err(self.bug("empty key".to_string()));
        };
        // Always start at the top level and drill down for our context.
        let mut hash: &mut Table = &mut self.mapping;
        let mut key_context = Key::with_capacity(key.len());

        // We only need implicit hashes for the parents.
        for k in parents {
            key_context.push(k.clone());

            // No key? Make an implicit hash and move on.
            if !hash.contains_key(k) {
                self.implicits.insert(key_string(&key_context));
                hash.insert(k.clone(), Value::Table(Table::new()));
            }

            // If the hash context is actually an array of tables, then set the hash context to
            // the last element in that array. Otherwise, it better be a table, since this MUST
            // be a key group (by virtue of it not being the last element in a key).
            hash = match hash.get_mut(k) {
                Some(Value::ArrayOfTables(t)) => match t.last_mut() {
                    Some(last_table) => last_table,
                    None => {
                        return Err(mk_err(
                            "BUG: empty array of tables".to_string(),
                            self.pos_line,
                            &self.context,
                            &self.current_key,
                        ));
                    }
                },
                Some(Value::Table(t)) => t,
                _ => {
                    return Err(mk_err(
                        format!(
                            "Key '{}' was already created as a hash.",
                            key_string(&key_context)
                        ),
                        self.pos_line,
                        &self.context,
                        &self.current_key,
                    ));
                }
            };
        }

        self.context = key_context;
        if array {
            // If this is the first element for this array, then allocate a new list of tables
            // for it.
            if !hash.contains_key(last) {
                hash.insert(last.clone(), Value::ArrayOfTables(Vec::new()));
            }

            // Add a new table. But make sure the key hasn't already been used for something else.
            match hash.get_mut(last) {
                Some(Value::ArrayOfTables(tables)) => tables.push(Table::new()),
                _ => {
                    return Err(mk_err(
                        format!(
                            "Key '{}' was already created and cannot be used as an array.",
                            key_string(key)
                        ),
                        self.pos_line,
                        &self.context,
                        &self.current_key,
                    ));
                }
            }
        } else {
            self.set_value(last, Value::Table(Table::new()))?;
        }
        self.context.push(last.clone());
        Ok(())
    }

    /// parse.go:641 `addImplicitContext`.
    fn add_implicit_context(&mut self, key: &[String]) -> Result<(), ParseError> {
        self.implicits.insert(key_string(key));
        self.add_context(key, false)
    }

    /// parse.go:631 `setValue`: sets the given key to the given value in the current context.
    /// It will make sure that the key hasn't already been defined, account for implicit key
    /// groups.
    fn set_value(&mut self, key: &str, value: Value) -> Result<(), ParseError> {
        let mut hash: &mut Table = &mut self.mapping;
        for (i, k) in self.context.iter().enumerate() {
            let key_context = || key_string(&self.context[..=i]);
            hash = match hash.get_mut(k) {
                None => {
                    return Err(mk_err(
                        format!(
                            "BUG: Context for key '{}' has not been established.",
                            key_context()
                        ),
                        self.pos_line,
                        &self.context,
                        &self.current_key,
                    ));
                }
                // The context is a table of hashes. Pick the most recent table defined as the
                // current hash.
                Some(Value::ArrayOfTables(t)) => match t.last_mut() {
                    Some(last_table) => last_table,
                    None => {
                        return Err(mk_err(
                            "BUG: empty array of tables".to_string(),
                            self.pos_line,
                            &self.context,
                            &self.current_key,
                        ));
                    }
                },
                Some(Value::Table(t)) => t,
                Some(_) => {
                    return Err(mk_err(
                        format!("Key '{}' has already been defined.", key_context()),
                        self.pos_line,
                        &self.context,
                        &self.current_key,
                    ));
                }
            };
        }
        let mut key_context = self.context.clone();
        key_context.push(key.to_string());
        let ks = key_string(&key_context);

        if hash.contains_key(key) {
            // Normally redefining keys isn't allowed, but the key could have been defined
            // implicitly and it's allowed to be redefined concretely. (See the
            // `valid/implicit-and-explicit-after.toml` in toml-test.)
            //
            // But we have to make sure to stop marking it as an implicit. (So that another
            // redefinition provokes an error.)
            //
            // Note that since it has already been defined (as a hash), we don't want to
            // overwrite it. So our business is done.
            if self.key_is_array.get(&ks).copied().unwrap_or(false) {
                self.implicits.remove(&ks);
                hash.insert(key.to_string(), value);
                return Ok(());
            }
            if self.implicits.remove(&ks) {
                return Ok(());
            }
            // Otherwise, we have a concrete key trying to override a previous key, which is
            // *always* wrong.
            return Err(mk_err(
                format!("Key '{ks}' has already been defined."),
                self.pos_line,
                &self.context,
                &self.current_key,
            ));
        }

        hash.insert(key.to_string(), value);
        Ok(())
    }

    /// parse.go:688 `setType`: records the type of the value at `key` (empty: the current
    /// context) after `setValue`.
    fn set_type(&mut self, key: &str, is_array: bool) {
        let mut key_context = self.context.clone();
        if !key.is_empty() {
            // allow type setting for hashes
            key_context.push(key.to_string());
        }
        // Special case to make empty keys ("" = 1) work.
        if key_context.is_empty() {
            key_context = vec![String::new()];
        }
        self.key_is_array.insert(key_string(&key_context), is_array);
    }
}

/// parse.go:504 `stripFirstNewline`.
fn strip_first_newline(s: &str) -> &str {
    if let Some(r) = s.strip_prefix('\n') {
        r
    } else if let Some(r) = s.strip_prefix("\r\n") {
        r
    } else {
        s
    }
}

/// parse.go:521 `stripEscapedNewlines`: removes whitespace after line-ending backslashes in
/// multiline strings. A line-ending backslash is an unescaped \ followed only by whitespace
/// until the next newline. After a line-ending backslash, all whitespace is removed until the
/// next non-whitespace character.
fn strip_escaped_newlines(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    let mut s = s;
    let mut i = 0;
    loop {
        let Some(ix) = s[i..].find('\\') else {
            b.push_str(s);
            return b;
        };
        i += ix;
        let bytes = s.as_bytes();

        if bytes.get(i + 1) == Some(&b'\\') {
            // Escaped backslash.
            i += 2;
            continue;
        }
        // Scan until the next non-whitespace.
        let mut j = i + 1;
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
            j += 1;
        }
        if j == i + 1 {
            // Not a whitespace escape.
            i += 1;
            continue;
        }
        if !s[i..j].contains('\n') {
            // This is not a line-ending backslash. (It's a bad escape sequence, but we can let
            // replaceEscapes catch it.)
            i += 1;
            continue;
        }
        b.push_str(&s[..i]);
        s = &s[j..];
        i = 0;
    }
}

/// parse.go:455 `numHasLeadingZero`: allowing for '0', +/- signs, and base prefixes.
fn num_has_leading_zero(s: &str) -> bool {
    let b = s.as_bytes();
    // Allow 0b, 0o, 0x
    if b.len() > 1 && b[0] == b'0' && !matches!(b[1], b'b' | b'o' | b'x') {
        return true;
    }
    b.len() > 2 && (b[0] == b'-' || b[0] == b'+') && b[1] == b'0'
}

/// parse.go:465 `numUnderscoresOK`: each underscore is surrounded by characters that are not
/// underscores.
fn num_underscores_ok(s: &str) -> bool {
    if matches!(s, "nan" | "+nan" | "-nan" | "inf" | "-inf" | "+inf") {
        return true;
    }
    let mut accept = false;
    for r in s.chars() {
        if r == '_' && !accept {
            return false;
        }
        // isHex is a superset of all the permissable characters surrounding an underscore.
        accept = is_hex(r);
    }
    accept
}

/// parse.go:486 `numPeriodsOK`: every period in s is followed by a digit.
fn num_periods_ok(s: &str) -> bool {
    let mut period = false;
    for r in s.chars() {
        if period && !r.is_ascii_digit() {
            return false;
        }
        period = r == '.';
    }
    !period
}

/// parse.go:350 `missingLeadingZero`: Go's `time.Parse` accepts numbers without a leading zero;
/// depend on the fact that the separators (- and :) should always be at the same location.
fn missing_leading_zero(d: &[u8], l: &[u8]) -> bool {
    for (i, &c) in l.iter().enumerate() {
        if c == b'.' || c == b'Z' {
            return false;
        }
        if !c.is_ascii_digit() && d.get(i) != Some(&c) {
            return true;
        }
    }
    false
}

/// time/format.go `isDigit`.
fn digit_at(v: &[u8], i: usize) -> bool {
    v.get(i).is_some_and(|b| b.is_ascii_digit())
}

/// time/format.go `getnum`: one or two digits at `i` (two when `fixed`); the value and the index
/// after it.
fn getnum(v: &[u8], i: usize, fixed: bool) -> Option<(i32, usize)> {
    if !digit_at(v, i) {
        return None;
    }
    let d0 = i32::from(v[i] - b'0');
    if !digit_at(v, i + 1) {
        if fixed {
            return None;
        }
        return Some((d0, i + 1));
    }
    Some((d0 * 10 + i32::from(v[i + 1] - b'0'), i + 2))
}

/// time/format.go `daysIn`.
fn days_in(month: i32, year: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Go `time.ParseInLocation(layout, v, ..)` for `LAYOUTS[layout]` (time/format.go:1051 `parse`):
/// the chunks are `2006` `01` `02` (fixed width), `15` (one or two digits), `04` `05` (fixed
/// width), `.999999999` and `Z07:00`. `None` is any parse error.
fn parse_layout(layout: usize, v: &[u8]) -> Option<Datetime> {
    let has_date = layout != 3;
    let has_time = layout != 2;
    let has_zone = layout == 0;
    let mut i = 0;
    let (mut year, mut month, mut day) = (0, 1, 1);
    let (mut hour, mut min, mut sec, mut nsec) = (0, 0, 0, 0u32);
    let mut offset = None;

    if has_date {
        // stdLongYear: at least 4 bytes, the first a digit, all of the 4 digits (atoi).
        if v.len() < 4 || !v[..4].iter().all(u8::is_ascii_digit) {
            return None;
        }
        year = v[..4].iter().fold(0, |n, d| n * 10 + i32::from(d - b'0'));
        i = 4;
        if v.get(i) != Some(&b'-') {
            return None;
        }
        // stdZeroMonth: "month out of range" unless 1..=12.
        (month, i) = getnum(v, i + 1, true)?;
        if !(1..=12).contains(&month) {
            return None;
        }
        if v.get(i) != Some(&b'-') {
            return None;
        }
        (day, i) = getnum(v, i + 1, true)?;
    }
    if has_date && has_time {
        if v.get(i) != Some(&b'T') {
            return None;
        }
        i += 1;
    }
    if has_time {
        // stdHour: one or two digits.
        (hour, i) = getnum(v, i, false)?;
        if hour >= 24 {
            return None;
        }
        if v.get(i) != Some(&b':') {
            return None;
        }
        // stdZeroMinute.
        (min, i) = getnum(v, i + 1, true)?;
        if min >= 60 {
            return None;
        }
        if v.get(i) != Some(&b':') {
            return None;
        }
        // stdZeroSecond.
        (sec, i) = getnum(v, i + 1, true)?;
        if sec >= 60 {
            return None;
        }
        // stdFracSecond9: "." or "," and a digit; any number of digits, the first nine kept
        // (`parseNanoseconds`).
        if v.len() >= i + 2 && (v[i] == b'.' || v[i] == b',') && v[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while digit_at(v, j) {
                j += 1;
            }
            let digits = &v[i + 1..j];
            for k in 0..9 {
                nsec = nsec * 10 + digits.get(k).map_or(0, |d| u32::from(d - b'0'));
            }
            i = j;
        }
    }
    if has_zone {
        // stdISO8601ColonTZ: 'Z' or sign, two digits, ':' and two digits.
        if v.get(i) == Some(&b'Z') {
            offset = Some(0);
            i += 1;
        } else {
            if v.len() < i + 6 || v[i + 3] != b':' {
                return None;
            }
            let (hr, _) = getnum(&v[i + 1..i + 3], 0, true)?;
            let (mm, _) = getnum(&v[i + 4..i + 6], 0, true)?;
            // The range test use > rather than >=, as some people do write offsets of 24 hours
            // or 60 minutes.
            if hr > 24 || mm > 60 {
                return None;
            }
            let off = (hr * 60 + mm) * 60;
            offset = Some(match v[i] {
                b'+' => off,
                b'-' => -off,
                _ => return None,
            });
            i += 6;
        }
    }
    // "extra text"
    if i != v.len() {
        return None;
    }
    // Validate the day of the month.
    if day < 1 || day > days_in(month, year) {
        return None;
    }

    Some(Datetime {
        kind: match layout {
            0 => DatetimeKind::OffsetDateTime,
            1 => DatetimeKind::LocalDateTime,
            2 => DatetimeKind::LocalDate,
            _ => DatetimeKind::LocalTime,
        },
        year,
        month: month as u8,
        day: day as u8,
        hour: hour as u8,
        minute: min as u8,
        second: sec as u8,
        nanosecond: nsec,
        offset_seconds: offset,
    })
}
