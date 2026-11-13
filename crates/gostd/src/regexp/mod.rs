//! Go 1.27.4 regexp (RE2): the leftmost-first matcher over `regexp::syntax` programs. The
//! backtrack and onepass engines are deferred, so there is no `Engine` type. The matcher
//! lands with graphify-rs-aa6.7.
#![allow(unused_variables)] // graphify-rs-aa6.7: stubs until the code PR
#![allow(dead_code)] // graphify-rs-aa6.7: the placeholder field is unused until the code PR

pub mod syntax;

/// A compiled regular expression (Go `Regexp`, regexp.go:81).
#[derive(Debug, Clone)]
pub struct Regexp {
    expr: String,
}

impl Regexp {
    /// Go `Compile` (regexp.go:129).
    pub fn compile(expr: &str) -> Result<Regexp, syntax::Error> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `MustCompile` (regexp.go:309); panics on a parse error.
    pub fn must_compile(expr: &str) -> Regexp {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `Regexp.String` (regexp.go:102): the source text used to compile.
    pub fn as_str(&self) -> &str {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `NumSubexp` (regexp.go:336).
    pub fn num_subexp(&self) -> usize {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `SubexpNames` (regexp.go:345): index 0 is always "".
    pub fn subexp_names(&self) -> &[String] {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `MatchString` (regexp.go:505).
    pub fn match_string(&self, s: &str) -> bool {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindStringIndex` (regexp.go:842).
    pub fn find_string_index(&self, s: &str) -> Option<(usize, usize)> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindStringSubmatch` (regexp.go:885); `None` entries are unmatched groups.
    pub fn find_string_submatch<'s>(&self, s: &'s str) -> Option<Vec<Option<&'s str>>> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindAllString` (regexp.go:1091).
    pub fn find_all_string<'s>(&self, s: &'s str, n: isize) -> Vec<&'s str> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindAllStringIndex` (regexp.go:1105).
    pub fn find_all_string_index(&self, s: &str, n: isize) -> Vec<(usize, usize)> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindAllStringSubmatch` (regexp.go:1125).
    pub fn find_all_string_submatch<'s>(&self, s: &'s str, n: isize) -> Vec<Vec<Option<&'s str>>> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindSubmatch` (regexp.go:867).
    pub fn find_submatch<'b>(&self, b: &'b [u8]) -> Option<Vec<Option<&'b [u8]>>> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindSubmatchIndex` (regexp.go:903).
    pub fn find_submatch_index(&self, b: &[u8]) -> Option<Vec<Option<(usize, usize)>>> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `FindAllSubmatchIndex` (regexp.go:1135).
    pub fn find_all_submatch_index(&self, b: &[u8], n: isize) -> Vec<Vec<Option<(usize, usize)>>> {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `ReplaceAllString` (regexp.go:551); `$` in `repl` is expanded.
    pub fn replace_all_string(&self, src: &str, repl: &str) -> String {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `ReplaceAllStringFunc` (regexp.go:575).
    pub fn replace_all_string_func(&self, src: &str, f: impl FnMut(&str) -> String) -> String {
        todo!("graphify-rs-aa6.7")
    }

    /// Go `Split` (regexp.go:1291).
    pub fn split<'s>(&self, s: &'s str, n: isize) -> Vec<&'s str> {
        todo!("graphify-rs-aa6.7")
    }
}

/// Go `QuoteMeta` (regexp.go:701).
pub fn quote_meta(s: &str) -> String {
    todo!("graphify-rs-aa6.7")
}
