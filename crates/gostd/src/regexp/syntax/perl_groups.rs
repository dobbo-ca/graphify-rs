//! Port of Go 1.27.4 regexp/syntax/perl_groups.go: the \d \s \w and POSIX `[[:name:]]` class tables.

use super::parse::CharGroup;

/// Go `code1` (perl_groups.go:9): `\d`.
const CODE1: &[u32] = &[0x30, 0x39];
/// Go `code2` (perl_groups.go:13): `\s`.
const CODE2: &[u32] = &[0x9, 0xa, 0xc, 0xd, 0x20, 0x20];
/// Go `code3` (perl_groups.go:19): `\w`.
const CODE3: &[u32] = &[0x30, 0x39, 0x41, 0x5a, 0x5f, 0x5f, 0x61, 0x7a];
/// Go `code4` (perl_groups.go:36): `[:alnum:]`.
const CODE4: &[u32] = &[0x30, 0x39, 0x41, 0x5a, 0x61, 0x7a];
/// Go `code5` (perl_groups.go:42): `[:alpha:]`.
const CODE5: &[u32] = &[0x41, 0x5a, 0x61, 0x7a];
/// Go `code6` (perl_groups.go:47): `[:ascii:]`.
const CODE6: &[u32] = &[0x0, 0x7f];
/// Go `code7` (perl_groups.go:51): `[:blank:]`.
const CODE7: &[u32] = &[0x9, 0x9, 0x20, 0x20];
/// Go `code8` (perl_groups.go:56): `[:cntrl:]`.
const CODE8: &[u32] = &[0x0, 0x1f, 0x7f, 0x7f];
/// Go `code9` (perl_groups.go:61): `[:digit:]`.
const CODE9: &[u32] = &[0x30, 0x39];
/// Go `code10` (perl_groups.go:65): `[:graph:]`.
const CODE10: &[u32] = &[0x21, 0x7e];
/// Go `code11` (perl_groups.go:69): `[:lower:]`.
const CODE11: &[u32] = &[0x61, 0x7a];
/// Go `code12` (perl_groups.go:73): `[:print:]`.
const CODE12: &[u32] = &[0x20, 0x7e];
/// Go `code13` (perl_groups.go:77): `[:punct:]`.
const CODE13: &[u32] = &[0x21, 0x2f, 0x3a, 0x40, 0x5b, 0x60, 0x7b, 0x7e];
/// Go `code14` (perl_groups.go:84): `[:space:]`.
const CODE14: &[u32] = &[0x9, 0xd, 0x20, 0x20];
/// Go `code15` (perl_groups.go:89): `[:upper:]`.
const CODE15: &[u32] = &[0x41, 0x5a];
/// Go `code16` (perl_groups.go:93): `[:word:]`.
const CODE16: &[u32] = &[0x30, 0x39, 0x41, 0x5a, 0x5f, 0x5f, 0x61, 0x7a];
/// Go `code17` (perl_groups.go:100): `[:xdigit:]`.
const CODE17: &[u32] = &[0x30, 0x39, 0x41, 0x46, 0x61, 0x66];

const fn pos(class: &'static [u32]) -> CharGroup {
    CharGroup { sign: 1, class }
}

const fn neg(class: &'static [u32]) -> CharGroup {
    CharGroup { sign: -1, class }
}

/// Go `perlGroup` map lookup (perl_groups.go:26): `\d \D \s \S \w \W`; key includes the backslash.
/// A missing key is Go's zero `charGroup` (sign 0), here `None`.
pub(super) fn perl_group(name: &str) -> Option<CharGroup> {
    Some(match name {
        r"\d" => pos(CODE1),
        r"\D" => neg(CODE1),
        r"\s" => pos(CODE2),
        r"\S" => neg(CODE2),
        r"\w" => pos(CODE3),
        r"\W" => neg(CODE3),
        _ => return None,
    })
}

/// Go `posixGroup` map lookup (perl_groups.go:104): `[:alnum:]`, `[:^alnum:]`, ... (key includes
/// brackets/colons). A missing key is Go's zero `charGroup` (sign 0), here `None`.
pub(super) fn posix_group(name: &str) -> Option<CharGroup> {
    Some(match name {
        "[:alnum:]" => pos(CODE4),
        "[:^alnum:]" => neg(CODE4),
        "[:alpha:]" => pos(CODE5),
        "[:^alpha:]" => neg(CODE5),
        "[:ascii:]" => pos(CODE6),
        "[:^ascii:]" => neg(CODE6),
        "[:blank:]" => pos(CODE7),
        "[:^blank:]" => neg(CODE7),
        "[:cntrl:]" => pos(CODE8),
        "[:^cntrl:]" => neg(CODE8),
        "[:digit:]" => pos(CODE9),
        "[:^digit:]" => neg(CODE9),
        "[:graph:]" => pos(CODE10),
        "[:^graph:]" => neg(CODE10),
        "[:lower:]" => pos(CODE11),
        "[:^lower:]" => neg(CODE11),
        "[:print:]" => pos(CODE12),
        "[:^print:]" => neg(CODE12),
        "[:punct:]" => pos(CODE13),
        "[:^punct:]" => neg(CODE13),
        "[:space:]" => pos(CODE14),
        "[:^space:]" => neg(CODE14),
        "[:upper:]" => pos(CODE15),
        "[:^upper:]" => neg(CODE15),
        "[:word:]" => pos(CODE16),
        "[:^word:]" => neg(CODE16),
        "[:xdigit:]" => pos(CODE17),
        "[:^xdigit:]" => neg(CODE17),
        _ => return None,
    })
}
