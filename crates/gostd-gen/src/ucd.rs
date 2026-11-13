//! A UCD file parser with the semantics of golang.org/x/text/internal/ucd (v0.27.0) `Parser`
//! without `KeepRanges`: fields are split at `;` and trimmed, `#` starts a comment, lines that
//! start with `@` are skipped, and a range in field 0 (`0041..005A`, or a UnicodeData
//! `<..., First>` / `<..., Last>` pair) yields one record per code point.

/// One record: the fields of its line, for one code point of field 0.
pub struct Record<'a> {
    pub rune: u32,
    pub fields: Vec<&'a str>,
}

impl Record<'_> {
    /// Field `i` (empty if the line has fewer fields), as `Parser.String`.
    pub fn string(&self, i: usize) -> &str {
        self.fields.get(i).copied().unwrap_or("")
    }

    /// Field `i` as a hex code point, as `Parser.Rune` for `i > 0`.
    pub fn rune_at(&self, i: usize) -> u32 {
        parse_rune(self.string(i))
    }
}

fn parse_rune(s: &str) -> u32 {
    let s = s.strip_prefix("U+").unwrap_or(s);
    u32::from_str_radix(s, 16).unwrap_or_else(|_| panic!("ucd: bad code point {s:?}"))
}

/// Calls `f` for every record of `text`, in file order.
pub fn parse<'a>(text: &'a str, mut f: impl FnMut(&Record<'a>)) {
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let body = line.find('#').map_or(line, |i| &line[..i]);
        if body.starts_with('@') {
            continue;
        }
        let fields: Vec<&str> = body.split(';').map(str::trim).collect();
        let (first, last) = if let Some((a, b)) = fields[0].split_once("..") {
            (parse_rune(a), parse_rune(b))
        } else if fields.len() > 1 && fields[1].ends_with("First>") {
            // UnicodeData.txt: the next line must be the matching <..., Last>.
            let next = lines.next().expect("ucd: unmatched <* First>");
            let nf: Vec<&str> = next.split(';').map(str::trim).collect();
            let name = |s: &str| {
                s.trim_start_matches('<')
                    .split(", ")
                    .next()
                    .unwrap_or("")
                    .to_owned()
            };
            assert!(
                nf.len() == fields.len()
                    && nf[1].ends_with("Last>")
                    && name(nf[1]) == name(fields[1])
                    && nf[2..] == fields[2..],
                "ucd: unmatched <* First> at {line:?}"
            );
            (parse_rune(fields[0]), parse_rune(nf[0]))
        } else {
            let r = parse_rune(fields[0]);
            (r, r)
        };
        for rune in first..=last {
            f(&Record {
                rune,
                fields: fields.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(text: &str) -> Vec<(u32, Vec<String>)> {
        let mut out = Vec::new();
        parse(text, |r| {
            out.push((r.rune, r.fields.iter().map(|s| s.to_string()).collect()))
        });
        out
    }

    #[test]
    fn unicode_data_first_last_expands() {
        let text = "3400;<CJK Ideograph Extension A, First>;Lo;0;L;;;;;N;;;;;\n\
                    3402;<CJK Ideograph Extension A, Last>;Lo;0;L;;;;;N;;;;;\n\
                    0041;LATIN CAPITAL LETTER A;Lu;0;L;;;;;N;;;;0061;\n";
        let got = collect(text);
        assert_eq!(got.len(), 4);
        assert_eq!(got[0].0, 0x3400);
        assert_eq!(got[2].0, 0x3402);
        assert_eq!(got[2].1[2], "Lo");
        assert_eq!(got[3].0, 0x41);
        assert_eq!(got[3].1[2], "Lu");
        assert_eq!(got[3].1[13], "0061");
        assert_eq!(got[3].1[12], "");
    }

    #[test]
    fn scripts_range_comment_and_missing() {
        let text = "# Scripts\n\n# @missing: 0000..10FFFF; Unknown\n\
                    0041..0043    ; Latin # L&   [3] LATIN CAPITAL LETTER A..C\n\
                    00AA          ; Latin # Lo       FEMININE ORDINAL INDICATOR\n";
        let got = collect(text);
        let runes: Vec<u32> = got.iter().map(|g| g.0).collect();
        assert_eq!(runes, [0x41, 0x42, 0x43, 0xAA]);
        assert!(got.iter().all(|g| g.1[1] == "Latin"));
    }

    #[test]
    fn case_folding_fields() {
        let text = "0041; C; 0061; # LATIN CAPITAL LETTER A\n00DF; F; 0073 0073; # LATIN SMALL LETTER SHARP S\n";
        let mut got = Vec::new();
        parse(text, |r| {
            got.push((r.rune, r.string(1).to_owned(), r.string(2).to_owned()))
        });
        assert_eq!(got[0], (0x41, "C".into(), "0061".into()));
        assert_eq!(got[1], (0xDF, "F".into(), "0073 0073".into()));
    }
}
