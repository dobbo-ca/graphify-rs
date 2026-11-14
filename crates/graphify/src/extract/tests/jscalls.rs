//! graphify-go `internal/extract/jscalls_test.go`.

use crate::extract::file_from_bytes;

// One walk over the file must attribute each call to the body it sits in, in
// source order, and ignore calls outside every registered body.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_js_calls_one_walk() {
    crate::extract::kinds::register_kinds();
    let src = r#"import x from "./x";
top();
function a() { one(); x.two(() => three()); const m = require("m"); }
const b = () => four(), c = function () { five.six(); };
export class K {
  m() { this.seven(); }
  n() {}
  o() { eight(); }
}
between();
exports.p = () => { nine(); };
export default function d() { ten(); }
after.all();
"#;
    let res = file_from_bytes("src/f.ts", src.as_bytes());
    let got: Vec<(String, String, String, String)> = res
        .calls
        .iter()
        .map(|n| {
            let c = res.call(n);
            (c.caller_id, c.callee, c.recv, c.loc.to_string())
        })
        .collect();
    let want = [
        ("src_f_ts_a", "one", "", "L3"),
        ("src_f_ts_a", "two", "x", "L3"),
        ("src_f_ts_a", "three", "", "L3"),
        ("src_f_ts_a", "require", "", "L3"),
        ("src_f_ts_b", "four", "", "L4"),
        ("src_f_ts_c", "six", "five", "L4"),
        ("src_f_ts_k_m", "seven", "this", "L6"),
        ("src_f_ts_k_o", "eight", "", "L8"),
        ("src_f_ts_p", "nine", "", "L11"),
        ("src_f_ts_d", "ten", "", "L12"),
    ];
    assert_eq!(got.len(), want.len(), "calls = {got:?}\nwant {want:?}");
    for (i, w) in want.iter().enumerate() {
        let g = &got[i];
        assert_eq!(
            (g.0.as_str(), g.1.as_str(), g.2.as_str(), g.3.as_str()),
            *w,
            "call {i}"
        );
    }
    assert!(
        res.imps.len() == 2 && res.imps[1].spec == "m",
        "imports = {:?}, want ./x then m",
        res.imps
    );
}
