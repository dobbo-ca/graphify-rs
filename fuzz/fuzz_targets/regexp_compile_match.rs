#![no_main]
use gostd::regexp::Regexp;
use libfuzzer_sys::fuzz_target;

// data = pattern, b'\n', subject. The pattern must be UTF-8 and compile; the subject is any bytes.
// Every match offset is in bounds and, for a UTF-8 subject, on a char boundary. The length caps
// keep repetition blowups from timing out.
fuzz_target!(|data: &[u8]| {
    let Some(nl) = data.iter().position(|&b| b == b'\n') else {
        return;
    };
    let (pat, rest) = (&data[..nl], &data[nl + 1..]);
    if pat.len() > 1000 || rest.len() > 64 << 10 {
        return;
    }
    let Ok(pat) = std::str::from_utf8(pat) else {
        return;
    };
    let Ok(re) = Regexp::compile(pat) else {
        return;
    };
    let text = std::str::from_utf8(rest).ok();
    for m in re.find_all_submatch_index(rest, -1) {
        for (a, b) in m.into_iter().flatten() {
            assert!(a <= b && b <= rest.len(), "span {a}..{b} of {}", rest.len());
            if let Some(t) = text {
                assert!(
                    t.is_char_boundary(a) && t.is_char_boundary(b),
                    "span {a}..{b} not on a char boundary"
                );
            }
        }
    }
});
