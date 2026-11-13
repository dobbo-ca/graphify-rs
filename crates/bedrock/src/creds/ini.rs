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
        todo!("graphify-rs-u2o.6")
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
    todo!("graphify-rs-u2o.6")
}
