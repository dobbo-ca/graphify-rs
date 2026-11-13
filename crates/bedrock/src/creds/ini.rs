//! Port of aws-sdk-go-v2 internal/ini v1.8.0 (tokenize.go, parse.go, strings.go, sections.go).

use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// One property value: a string, or the sub-properties of an indented block (Go `Value.mp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IniValue {
    Str(String),
    Map(BTreeMap<String, String>),
}

impl IniValue {
    /// Go `fmt.Sprintf("%v", ...)`: the string, or `map[a:b b:c]` with sorted keys.
    pub(crate) fn display(&self) -> String {
        match self {
            IniValue::Str(s) => s.clone(),
            IniValue::Map(m) => {
                let pairs: Vec<String> = m.iter().map(|(k, v)| format!("{k}:{v}")).collect();
                format!("map[{}]", pairs.join(" "))
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Section {
    /// Keys lowercased.
    pub values: BTreeMap<String, IniValue>,
}

impl Section {
    /// The string value of `key` (a map value gives None).
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        match self.values.get(key) {
            Some(IniValue::Str(s)) => Some(s),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sections {
    /// Section names as written (case-sensitive), after Go's header normalisation.
    pub sections: BTreeMap<String, Section>,
}

impl Sections {
    pub(crate) fn get(&self, section: &str) -> Option<&Section> {
        self.sections.get(section)
    }
}

/// Go `ini.Parse`; the parser never fails.
pub(crate) fn parse(text: &str) -> Sections {
    let mut p = Parser {
        sections: Sections::default(),
        csection: String::new(),
        ckey: String::new(),
    };
    // tokenize.go:9-27: one token per line, tried in this order; anything else is ignored.
    for line in text.split('\n') {
        if line.trim().is_empty() || is_line_comment(line) {
            continue;
        }
        let indented = line.starts_with([' ', '\t']);
        if let Some((typ, name)) = as_profile(line) {
            p.profile(typ, name);
        } else if let Some((k, v)) = as_property(line) {
            p.property(k, v);
        } else if let Some((k, v)) = as_sub_property(line) {
            p.sub_property(k, v);
        } else if indented {
            p.continuation(line.trim_start_matches([' ', '\t']));
        }
    }
    p.sections
}

/// parse.go:25-30 parser state; an empty `csection` or `ckey` means none (as in Go).
struct Parser {
    sections: Sections,
    csection: String,
    ckey: String,
}

impl Parser {
    /// parse.go:42-52
    fn profile(&mut self, typ: String, name: String) {
        let name = if typ.is_empty() {
            name
        } else {
            format!("{typ} {name}")
        };
        self.ckey.clear();
        self.csection = name.clone();
        self.sections.sections.entry(name).or_default();
    }

    /// parse.go:54-76; a later duplicate key wins.
    fn property(&mut self, key: String, value: String) {
        if self.csection.is_empty() {
            return; // LEGACY: don't error on "global" properties
        }
        self.ckey.clone_from(&key);
        self.sections
            .sections
            .entry(self.csection.clone())
            .or_default()
            .values
            .insert(key, IniValue::Str(value));
    }

    /// parse.go:78-100
    fn sub_property(&mut self, key: String, value: String) {
        if self.csection.is_empty() {
            return;
        }
        let section = self
            .sections
            .sections
            .entry(self.csection.clone())
            .or_default();
        let flat = self.ckey.is_empty()
            || matches!(section.values.get(&self.ckey), Some(IniValue::Str(s)) if !s.is_empty());
        if flat {
            let value = trim_property_comment(&value).trim().to_string();
            self.property(key, value);
            return;
        }
        let slot = section
            .values
            .entry(self.ckey.clone())
            .or_insert_with(|| IniValue::Map(BTreeMap::new()));
        if matches!(slot, IniValue::Str(_)) {
            *slot = IniValue::Map(BTreeMap::new());
        }
        if let IniValue::Map(m) = slot {
            m.insert(key, value);
        }
    }

    /// parse.go:102-115; only a non-empty string value takes continuation lines.
    fn continuation(&mut self, value: &str) {
        if self.ckey.is_empty() {
            return;
        }
        let slot = self
            .sections
            .sections
            .get_mut(&self.csection)
            .and_then(|s| s.values.get_mut(&self.ckey));
        if let Some(IniValue::Str(s)) = slot
            && !s.is_empty()
        {
            s.push('\n');
            s.push_str(value);
        }
    }
}

/// tokenize.go:29-33
fn is_line_comment(line: &str) -> bool {
    let t = line.trim_start_matches([' ', '\t']);
    t.starts_with('#') || t.starts_with(';')
}

/// Go `strings.Cut(s, sep)` prefix.
fn cut_before<'a>(s: &'a str, sep: &str) -> &'a str {
    s.find(sep).map_or(s, |i| &s[..i])
}

/// strings.go:7-11
fn trim_profile_comment(s: &str) -> &str {
    cut_before(cut_before(s, "#"), ";")
}

/// strings.go:13-19
fn trim_property_comment(s: &str) -> &str {
    let s = cut_before(s, " #");
    let s = cut_before(s, " ;");
    let s = cut_before(s, "\t#");
    cut_before(s, "\t;")
}

/// strings.go:22-38: the first `=` or `:` splits, whichever comes first.
fn split_property(s: &str) -> Option<(&str, &str)> {
    let sep = match (s.find('='), s.find(':')) {
        (None, _) => ':',
        (Some(eq), Some(colon)) if colon < eq => ':',
        _ => '=',
    };
    let (k, v) = s.split_once(sep)?;
    Some((k.trim(), v.trim()))
}

/// strings.go:41-58
fn split_profile(s: &str) -> (&str, &str) {
    let mut first = 0;
    for (i, c) in s.char_indices() {
        if c == ' ' || c == '\t' {
            if first == 0 {
                first = i;
            }
        } else if first != 0 {
            return (&s[..first], &s[i..]);
        }
    }
    if first == 0 { ("", s) } else { ("", "") }
}

/// strings.go:68-73: one pair of matching quotes. Go slices `s[1:len-1]` here and panics on a
/// lone quote character; that value is kept as is.
fn unquote(s: &str) -> &str {
    let quoted =
        (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"'));
    if quoted && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// tokenize.go:35-49: `[ type name ]` with a comment after it.
fn as_profile(line: &str) -> Option<(String, String)> {
    let t = trim_profile_comment(line).trim();
    if !(t.starts_with('[') && t.ends_with(']')) {
        return None;
    }
    let (typ, name) = split_profile(t[1..t.len() - 1].trim());
    Some((typ.to_string(), name.to_string()))
}

/// tokenize.go:51-65: a property starts in column 0.
fn as_property(line: &str) -> Option<(String, String)> {
    if line.starts_with([' ', '\t']) {
        return None;
    }
    let t = trim_property_comment(line).trim_end_matches([' ', '\t']);
    let (k, v) = split_property(t)?;
    Some((lower(k), unquote(v).to_string()))
}

/// tokenize.go:67-80: an indented `key = value`; no comment trimming here.
fn as_sub_property(line: &str) -> Option<(String, String)> {
    if !line.starts_with([' ', '\t']) {
        return None;
    }
    let (k, v) = split_property(line.trim_start_matches([' ', '\t']))?;
    Some((lower(k), unquote(v).to_string()))
}

/// Go `strings.ToLower`: per character, no context-sensitive rules.
fn lower(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}
