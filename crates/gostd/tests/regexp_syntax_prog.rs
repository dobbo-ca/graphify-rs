//! Go 1.27.4 regexp/syntax/prog_test.go
//!
//! TestCompile: ported
//! BenchmarkEmptyOpContext, BenchmarkIsWordChar: benchmarks, not Test funcs

use gostd::regexp::syntax::{Flags, compile, parse};

const COMPILE_TESTS: &[(&str, &str)] = &[
    (
        "a",
        r#"  0	fail
  1*	rune1 "a" -> 2
  2	match
"#,
    ),
    (
        "[A-M][n-z]",
        r#"  0	fail
  1*	rune "AM" -> 2
  2	rune "nz" -> 3
  3	match
"#,
    ),
    (
        "",
        r#"  0	fail
  1*	nop -> 2
  2	match
"#,
    ),
    (
        "a?",
        r#"  0	fail
  1	rune1 "a" -> 3
  2*	alt -> 1, 3
  3	match
"#,
    ),
    (
        "a??",
        r#"  0	fail
  1	rune1 "a" -> 3
  2*	alt -> 3, 1
  3	match
"#,
    ),
    (
        "a+",
        r#"  0	fail
  1*	rune1 "a" -> 2
  2	alt -> 1, 3
  3	match
"#,
    ),
    (
        "a+?",
        r#"  0	fail
  1*	rune1 "a" -> 2
  2	alt -> 3, 1
  3	match
"#,
    ),
    (
        "a*",
        r#"  0	fail
  1	rune1 "a" -> 2
  2*	alt -> 1, 3
  3	match
"#,
    ),
    (
        "a*?",
        r#"  0	fail
  1	rune1 "a" -> 2
  2*	alt -> 3, 1
  3	match
"#,
    ),
    (
        "a+b+",
        r#"  0	fail
  1*	rune1 "a" -> 2
  2	alt -> 1, 3
  3	rune1 "b" -> 4
  4	alt -> 3, 5
  5	match
"#,
    ),
    (
        "(a+)(b+)",
        r#"  0	fail
  1*	cap 2 -> 2
  2	rune1 "a" -> 3
  3	alt -> 2, 4
  4	cap 3 -> 5
  5	cap 4 -> 6
  6	rune1 "b" -> 7
  7	alt -> 6, 8
  8	cap 5 -> 9
  9	match
"#,
    ),
    (
        "a+|b+",
        r#"  0	fail
  1	rune1 "a" -> 2
  2	alt -> 1, 6
  3	rune1 "b" -> 4
  4	alt -> 3, 6
  5*	alt -> 1, 3
  6	match
"#,
    ),
    (
        "A[Aa]",
        r#"  0	fail
  1*	rune1 "A" -> 2
  2	rune "A"/i -> 3
  3	match
"#,
    ),
    (
        "(?:(?:^).)",
        r#"  0	fail
  1*	empty 4 -> 2
  2	anynotnl -> 3
  3	match
"#,
    ),
    (
        "(?:|a)+",
        r#"  0	fail
  1	nop -> 4
  2	rune1 "a" -> 4
  3*	alt -> 1, 2
  4	alt -> 3, 5
  5	match
"#,
    ),
    (
        "(?:|a)*",
        r#"  0	fail
  1	nop -> 4
  2	rune1 "a" -> 4
  3	alt -> 1, 2
  4	alt -> 3, 6
  5*	alt -> 3, 6
  6	match
"#,
    ),
];

#[test]
#[ignore = "graphify-rs-aa6.6"]
fn test_compile() {
    for (regexp, want) in COMPILE_TESTS {
        let re = parse(regexp, Flags::PERL).unwrap();
        let p = compile(&re).unwrap();
        let have = format!("{p}");
        assert_eq!(&have, want, "compiled {regexp:?}");
    }
}
