//! Go 1.27.4 regexp/syntax/simplify_test.go
//!
//! TestSimplify: ported

use gostd::regexp::syntax::{Flags, parse};

const SIMPLIFY_TESTS: &[(&str, &str)] = &[
    // Already-simple constructs
    (r"a", r"a"),
    (r"ab", r"ab"),
    (r"a|b", r"[ab]"),
    (r"ab|cd", r"ab|cd"),
    (r"(ab)*", r"(ab)*"),
    (r"(ab)+", r"(ab)+"),
    (r"(ab)?", r"(ab)?"),
    (r".", r"(?s:.)"),
    (r"^", r"(?m:^)"),
    (r"$", r"(?m:$)"),
    (r"[ac]", r"[ac]"),
    (r"[^ac]", r"[^ac]"),
    // Posix character classes
    (r"[[:alnum:]]", r"[0-9A-Za-z]"),
    (r"[[:alpha:]]", r"[A-Za-z]"),
    (r"[[:blank:]]", r"[\t ]"),
    (r"[[:cntrl:]]", r"[\x00-\x1f\x7f]"),
    (r"[[:digit:]]", r"[0-9]"),
    (r"[[:graph:]]", r"[!-~]"),
    (r"[[:lower:]]", r"[a-z]"),
    (r"[[:print:]]", r"[ -~]"),
    (r"[[:punct:]]", r"[!-/:-@\[-`\{-~]"),
    (r"[[:space:]]", r"[\t-\r ]"),
    (r"[[:upper:]]", r"[A-Z]"),
    (r"[[:xdigit:]]", r"[0-9A-Fa-f]"),
    // Perl character classes
    (r"\d", r"[0-9]"),
    (r"\s", r"[\t\n\f\r ]"),
    (r"\w", r"[0-9A-Z_a-z]"),
    (r"\D", r"[^0-9]"),
    (r"\S", r"[^\t\n\f\r ]"),
    (r"\W", r"[^0-9A-Z_a-z]"),
    (r"[\d]", r"[0-9]"),
    (r"[\s]", r"[\t\n\f\r ]"),
    (r"[\w]", r"[0-9A-Z_a-z]"),
    (r"[\D]", r"[^0-9]"),
    (r"[\S]", r"[^\t\n\f\r ]"),
    (r"[\W]", r"[^0-9A-Z_a-z]"),
    // Posix repetitions
    (r"a{1}", r"a"),
    (r"a{2}", r"aa"),
    (r"a{5}", r"aaaaa"),
    (r"a{0,1}", r"a?"),
    // The next three are illegible because Simplify inserts (?:)
    // parens instead of () parens to avoid creating extra
    // captured subexpressions. The comments show a version with fewer parens.
    (r"(a){0,2}", r"(?:(a)(a)?)?"),                 //       (aa?)?
    (r"(a){0,4}", r"(?:(a)(?:(a)(?:(a)(a)?)?)?)?"), //   (a(a(aa?)?)?)?
    (r"(a){2,6}", r"(a)(a)(?:(a)(?:(a)(?:(a)(a)?)?)?)?"), // aa(a(a(aa?)?)?)?
    (r"a{0,2}", r"(?:aa?)?"),                       //       (aa?)?
    (r"a{0,4}", r"(?:a(?:a(?:aa?)?)?)?"),           //   (a(a(aa?)?)?)?
    (r"a{2,6}", r"aa(?:a(?:a(?:aa?)?)?)?"),         // aa(a(a(aa?)?)?)?
    (r"a{0,}", r"a*"),
    (r"a{1,}", r"a+"),
    (r"a{2,}", r"aa+"),
    (r"a{5,}", r"aaaaa+"),
    // Test that operators simplify their arguments.
    (r"(?:a{1,}){1,}", r"a+"),
    (r"(a{1,}b{1,})", r"(a+b+)"),
    (r"a{1,}|b{1,}", r"a+|b+"),
    (r"(?:a{1,})*", r"(?:a+)*"),
    (r"(?:a{1,})+", r"a+"),
    (r"(?:a{1,})?", r"(?:a+)?"),
    (r"", r"(?:)"),
    (r"a{0}", r"(?:)"),
    // Character class simplification
    (r"[ab]", r"[ab]"),
    (r"[abc]", r"[a-c]"),
    (r"[a-za-za-z]", r"[a-z]"),
    (r"[A-Za-zA-Za-z]", r"[A-Za-z]"),
    (r"[ABCDEFGH]", r"[A-H]"),
    (r"[AB-CD-EF-GH]", r"[A-H]"),
    (r"[W-ZP-XE-R]", r"[E-Z]"),
    (r"[a-ee-gg-m]", r"[a-m]"),
    (r"[a-ea-ha-m]", r"[a-m]"),
    (r"[a-ma-ha-e]", r"[a-m]"),
    (r"[a-zA-Z0-9 -~]", r"[ -~]"),
    // Empty character classes
    (r"[^[:cntrl:][:^cntrl:]]", r"[^\x00-\x{10FFFF}]"),
    // Full character classes
    (r"[[:cntrl:][:^cntrl:]]", r"(?s:.)"),
    // Unicode case folding.
    (r"(?i)A", r"(?i:A)"),
    (r"(?i)a", r"(?i:A)"),
    (r"(?i)[A]", r"(?i:A)"),
    (r"(?i)[a]", r"(?i:A)"),
    (r"(?i)K", r"(?i:K)"),
    (r"(?i)k", r"(?i:K)"),
    (r"(?i)\x{212a}", "(?i:K)"),
    (r"(?i)[K]", "[Kk\u{212A}]"),
    (r"(?i)[k]", "[Kk\u{212A}]"),
    (r"(?i)[\x{212a}]", "[Kk\u{212A}]"),
    (r"(?i)[a-z]", "[A-Za-z\u{017F}\u{212A}]"),
    (r"(?i)[\x00-\x{FFFD}]", "[\\x00-\u{FFFD}]"),
    (r"(?i)[\x00-\x{10FFFF}]", r"(?s:.)"),
    // Empty string as a regular expression.
    // The empty string must be preserved inside parens in order
    // to make submatches work right, so these tests are less
    // interesting than they might otherwise be. String inserts
    // explicit (?:) in place of non-parenthesized empty strings,
    // to make them easier to spot for other parsers.
    (r"(a|b|c|)", r"([a-c]|(?:))"),
    (r"(a|b|)", r"([ab]|(?:))"),
    (r"(|)", r"()"),
    (r"a()", r"a()"),
    (r"(()|())", r"(()|())"),
    (r"(a|)", r"(a|(?:))"),
    (r"ab()cd()", r"ab()cd()"),
    (r"()", r"()"),
    (r"()*", r"()*"),
    (r"()+", r"()+"),
    (r"()?", r"()?"),
    (r"(){0}", r"(?:)"),
    (r"(){1}", r"()"),
    (r"(){1,}", r"()+"),
    (r"(){0,2}", r"(?:()()?)?"),
];

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_simplify() {
    for (pattern, want) in SIMPLIFY_TESTS {
        let re = parse(pattern, Flags::MATCH_NL | (Flags::PERL & !Flags::ONE_LINE))
            .unwrap_or_else(|e| panic!("Parse({pattern:?}) = error {e}"));
        let s = format!("{}", re.simplify());
        assert_eq!(s, *want, "Simplify({pattern:?})");
    }
}
