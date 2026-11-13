//! Go 1.27.4 regexp (RE2): the leftmost-first matcher over `regexp::syntax` programs. The
//! backtrack and onepass engines are deferred (graphify-rs-aa6.8), so there is no `Engine` type:
//! every match runs on the NFA machine of `exec`, which gives the same results as all three Go
//! engines. Strings and byte slices are searched as bytes; the `str` methods slice the subject at
//! match positions, which are always rune boundaries.

mod exec;
pub mod syntax;

use crate::strconv;
use crate::unicode;
use crate::utf8::decode_rune;
use syntax::{EmptyOp, Flags, Prog};

/// A compiled regular expression (Go `Regexp`, regexp.go:81).
#[derive(Debug, Clone)]
pub struct Regexp {
    expr: String,
    prog: Prog,
    num_subexp: usize,
    subexp_names: Vec<String>,
    /// Required prefix in unanchored matches (Go `prefix`).
    prefix: String,
    /// First rune of `prefix`, `None` if it is empty (Go `prefixRune`).
    prefix_rune: Option<char>,
    /// Empty-width conditions required at the start of a match (Go `cond`).
    cond: EmptyOp,
}

impl Regexp {
    /// Go `Compile` (regexp.go:129) via `compile` (regexp.go:166): `syntax.Perl`, leftmost-first.
    pub fn compile(expr: &str) -> Result<Regexp, syntax::Error> {
        let re = syntax::parse(expr, Flags::PERL)?;
        let num_subexp = re.max_cap() as usize;
        let subexp_names = re.cap_names();
        let re = re.simplify();
        let prog = syntax::compile(&re)?;
        let cond = prog.start_cond();
        let prefix = prog.prefix().0;
        let prefix_rune = prefix.chars().next();
        Ok(Regexp {
            expr: expr.to_string(),
            prog,
            num_subexp,
            subexp_names,
            prefix,
            prefix_rune,
            cond,
        })
    }

    /// Go `MustCompile` (regexp.go:309); panics on a parse error.
    pub fn must_compile(expr: &str) -> Regexp {
        match Regexp::compile(expr) {
            Ok(re) => re,
            Err(err) => panic!("regexp: Compile({}): {err}", quote(expr)),
        }
    }

    /// Go `Regexp.String` (regexp.go:102): the source text used to compile.
    pub fn as_str(&self) -> &str {
        &self.expr
    }

    /// Go `NumSubexp` (regexp.go:336).
    pub fn num_subexp(&self) -> usize {
        self.num_subexp
    }

    /// Go `SubexpNames` (regexp.go:345): index 0 is always "".
    pub fn subexp_names(&self) -> &[String] {
        &self.subexp_names
    }

    /// Go `Regexp.find` (exec.go:521) without the reader, onepass and backtrack paths: the
    /// leftmost-first match in `b` from `pos`, as `ncap` positions (-1 for an unset group).
    fn find(&self, b: &[u8], pos: usize, ncap: usize) -> Option<Vec<isize>> {
        exec::find(self, b, pos, ncap)
    }

    /// The `(start, end)` of every group `0..=num_subexp` of the match `m`, `None` if unset (or
    /// beyond `m`): the loop of Go `FindSubmatch` (regexp.go:867). This also does the job of Go
    /// `Regexp.pad` (regexp.go:733), which appends -1s for groups the program does not capture.
    fn submatches(&self, m: &[isize]) -> Vec<Option<(usize, usize)>> {
        (0..=self.num_subexp)
            .map(|i| {
                if 2 * i + 1 < m.len() && m[2 * i] >= 0 {
                    Some((m[2 * i] as usize, m[2 * i + 1] as usize))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Go `Regexp.matches` (regexp.go:747), collected: the successive matches in `b`, at most
    /// `max` of them if `max > 0`. An empty match right after the previous match is skipped.
    fn matches(&self, b: &[u8], mut max: isize, ncap: usize) -> Vec<Vec<isize>> {
        let mut out = Vec::new();
        if max == 0 {
            return out;
        }
        let end = b.len();
        let mut pos = 0;
        let mut prev_match_end = -1;
        while pos <= end {
            let Some(m) = self.find(b, pos, ncap) else {
                break;
            };

            let mut accept = true;
            if m[1] == pos as isize {
                // We've found an empty match.
                if m[0] == prev_match_end {
                    // We don't allow an empty match right after a previous match, so ignore it.
                    accept = false;
                }
                let width = exec::step(b, pos).1;
                if width > 0 {
                    pos += width;
                } else {
                    pos = end + 1;
                }
            } else {
                pos = m[1] as usize;
            }
            prev_match_end = m[1];

            if accept {
                out.push(m);
                if max > 0 {
                    max -= 1;
                    if max == 0 {
                        break;
                    }
                }
            }
        }
        out
    }

    /// Go `MatchString` (regexp.go:505).
    pub fn match_string(&self, s: &str) -> bool {
        self.find(s.as_bytes(), 0, 0).is_some()
    }

    /// Go `FindStringIndex` (regexp.go:842).
    pub fn find_string_index(&self, s: &str) -> Option<(usize, usize)> {
        let m = self.find(s.as_bytes(), 0, 2)?;
        Some((m[0] as usize, m[1] as usize))
    }

    /// Go `FindStringSubmatch` (regexp.go:885); `None` entries are unmatched groups.
    pub fn find_string_submatch<'s>(&self, s: &'s str) -> Option<Vec<Option<&'s str>>> {
        let m = self.find(s.as_bytes(), 0, self.prog.num_cap)?;
        Some(
            self.submatches(&m)
                .into_iter()
                .map(|p| p.map(|(a, e)| &s[a..e]))
                .collect(),
        )
    }

    /// Go `FindAllString` (regexp.go:1091), via `allString` (regexp.go:945).
    pub fn find_all_string<'s>(&self, s: &'s str, n: isize) -> Vec<&'s str> {
        self.matches(s.as_bytes(), n, 2)
            .iter()
            .map(|m| &s[m[0] as usize..m[1] as usize])
            .collect()
    }

    /// Go `FindAllStringIndex` (regexp.go:1105), via `allStringIndex` (regexp.go:963).
    pub fn find_all_string_index(&self, s: &str, n: isize) -> Vec<(usize, usize)> {
        self.matches(s.as_bytes(), n, 2)
            .iter()
            .map(|m| (m[0] as usize, m[1] as usize))
            .collect()
    }

    /// Go `FindAllStringSubmatch` (regexp.go:1125), via `allStringSubmatch` (regexp.go:991).
    pub fn find_all_string_submatch<'s>(&self, s: &'s str, n: isize) -> Vec<Vec<Option<&'s str>>> {
        self.matches(s.as_bytes(), n, self.prog.num_cap)
            .iter()
            .map(|m| {
                self.submatches(m)
                    .into_iter()
                    .map(|p| p.map(|(a, e)| &s[a..e]))
                    .collect()
            })
            .collect()
    }

    /// Go `FindSubmatch` (regexp.go:867).
    pub fn find_submatch<'b>(&self, b: &'b [u8]) -> Option<Vec<Option<&'b [u8]>>> {
        let m = self.find(b, 0, self.prog.num_cap)?;
        Some(
            self.submatches(&m)
                .into_iter()
                .map(|p| p.map(|(a, e)| &b[a..e]))
                .collect(),
        )
    }

    /// Go `FindSubmatchIndex` (regexp.go:903).
    pub fn find_submatch_index(&self, b: &[u8]) -> Option<Vec<Option<(usize, usize)>>> {
        let m = self.find(b, 0, self.prog.num_cap)?;
        Some(self.submatches(&m))
    }

    /// Go `FindAllSubmatchIndex` (regexp.go:1135), via `allSubmatchIndex` (regexp.go:1004).
    pub fn find_all_submatch_index(&self, b: &[u8], n: isize) -> Vec<Vec<Option<(usize, usize)>>> {
        self.matches(b, n, self.prog.num_cap)
            .iter()
            .map(|m| self.submatches(m))
            .collect()
    }

    /// Go `ReplaceAllString` (regexp.go:551); `$` in `repl` is expanded.
    pub fn replace_all_string(&self, src: &str, repl: &str) -> String {
        let n = if repl.contains('$') {
            2 * (self.num_subexp + 1)
        } else {
            2
        };
        self.replace_all(src, n, |dst, m| self.expand(dst, repl, src, m))
    }

    /// Go `ReplaceAllStringFunc` (regexp.go:575); the result of `f` is not expanded.
    pub fn replace_all_string_func(&self, src: &str, mut f: impl FnMut(&str) -> String) -> String {
        self.replace_all(src, 2, |dst, m| {
            dst.push_str(&f(&src[m[0] as usize..m[1] as usize]));
        })
    }

    /// Go `Regexp.replaceAll` (regexp.go:582) for a string `src`: `repl` appends the replacement
    /// for the match `m` (at most `nmatch` positions) to the output.
    fn replace_all(
        &self,
        src: &str,
        nmatch: usize,
        mut repl: impl FnMut(&mut String, &[isize]),
    ) -> String {
        let b = src.as_bytes();
        let mut last_match_end = 0; // end position of the most recent match
        let mut search_pos = 0; // position where we next look for a match
        let mut buf = String::new();
        let nmatch = nmatch.min(self.prog.num_cap);

        while search_pos <= b.len() {
            let Some(a) = self.find(b, search_pos, nmatch) else {
                break; // no more matches
            };
            let (a0, a1) = (a[0] as usize, a[1] as usize);

            // Copy the unmatched characters before this match.
            buf.push_str(&src[last_match_end..a0]);

            // Now insert a copy of the replacement string, but not for a match of the empty
            // string immediately after another match. (Otherwise, we get double replacement for
            // patterns that match both empty and nonempty strings.)
            if a1 > last_match_end || a0 == 0 {
                repl(&mut buf, &a);
            }
            last_match_end = a1;

            // Advance past this match; always advance at least one character.
            let width = decode_rune(&b[search_pos..]).1;
            if search_pos + width > a1 {
                search_pos += width;
            } else if search_pos + 1 > a1 {
                // This clause is only needed at the end of the input string. In that case,
                // decode_rune returns width=0.
                search_pos += 1;
            } else {
                search_pos = a1;
            }
        }

        // Copy the unmatched characters after the last match.
        buf.push_str(&src[last_match_end..]);
        buf
    }

    /// Go `Regexp.expand` (regexp.go:1177) for a string `src`: appends `template` to `dst`,
    /// replacing `$1`, `${1}`, `$name` and `${name}` by the text of that group in the match `m`
    /// and `$$` by `$`. A group that is unset or out of range expands to nothing.
    fn expand(&self, dst: &mut String, template: &str, src: &str, m: &[isize]) {
        let group = |i: usize| -> Option<&str> {
            (2 * i + 1 < m.len() && m[2 * i] >= 0)
                .then(|| &src[m[2 * i] as usize..m[2 * i + 1] as usize])
        };
        let mut template = template;
        while !template.is_empty() {
            let Some((before, after)) = template.split_once('$') else {
                break;
            };
            dst.push_str(before);
            template = after;
            if let Some(rest) = template.strip_prefix('$') {
                // Treat $$ as $.
                dst.push('$');
                template = rest;
                continue;
            }
            let Some((name, num, rest)) = extract(template) else {
                // Malformed; treat $ as raw text.
                dst.push('$');
                continue;
            };
            template = rest;
            if let Some(num) = num {
                dst.push_str(group(num).unwrap_or_default());
            } else {
                for (i, namei) in self.subexp_names.iter().enumerate() {
                    if let Some(text) = group(i).filter(|_| name == namei) {
                        dst.push_str(text);
                        break;
                    }
                }
            }
        }
        dst.push_str(template);
    }

    /// Go `Split` (regexp.go:1291).
    pub fn split<'s>(&self, s: &'s str, n: isize) -> Vec<&'s str> {
        if n == 0 {
            return Vec::new();
        }

        if !self.expr.is_empty() && s.is_empty() {
            return vec![""];
        }

        let matches = self.find_all_string_index(s, n);
        let mut strings = Vec::with_capacity(matches.len());

        let mut beg = 0;
        let mut end = 0;
        for (m0, m1) in matches {
            if n > 0 && strings.len() as isize >= n - 1 {
                break;
            }

            end = m0;
            if m1 != 0 {
                strings.push(&s[beg..end]);
            }
            beg = m1;
        }

        if end != s.len() {
            strings.push(&s[beg..]);
        }

        strings
    }
}

/// Go `quote` (regexp.go:328): the pattern in backquotes if it can be, else Go-quoted.
fn quote(s: &str) -> String {
    // Go `strconv.CanBackquote` (quote.go:`CanBackquote`).
    if s.chars()
        .all(|c| (c >= ' ' || c == '\t') && !matches!(c, '`' | '\u{7f}' | '\u{feff}'))
    {
        format!("`{s}`")
    } else {
        strconv::quote(s.as_bytes())
    }
}

/// Go `extract` (regexp.go:1226): pulls the name or number from the front of `s` (after a `$`),
/// as `${name}` or `$name`. The number is `None` if the name is not a number (Go `-1`). Returns
/// `None` if `s` does not start with a valid name; `rest` is what follows it.
fn extract(mut s: &str) -> Option<(&str, Option<usize>, &str)> {
    let mut brace = false;
    if let Some(rest) = s.strip_prefix('{') {
        brace = true;
        s = rest;
    }
    let mut i = s
        .char_indices()
        .find(|&(_, c)| !(unicode::is_letter(c) || unicode::is(&unicode::ND, c) || c == '_'))
        .map_or(s.len(), |(i, _)| i);
    if i == 0 {
        return None;
    }
    let name = &s[..i];
    if brace {
        if !s[i..].starts_with('}') {
            return None;
        }
        i += 1;
    }

    let mut num = Some(0usize);
    for c in name.bytes() {
        match num {
            Some(n) if c.is_ascii_digit() && n < 100_000_000 => {
                num = Some(n * 10 + usize::from(c - b'0'));
            }
            _ => {
                num = None;
                break;
            }
        }
    }
    if name.starts_with('0') && name.len() > 1 {
        num = None;
    }

    Some((name, num, &s[i..]))
}

/// Go `QuoteMeta` (regexp.go:701): escapes all regular expression metacharacters in `s`.
pub fn quote_meta(s: &str) -> String {
    // A char loop is correct because all metacharacters are ASCII.
    let mut b = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            b.push('\\');
        }
        b.push(c);
    }
    b
}
