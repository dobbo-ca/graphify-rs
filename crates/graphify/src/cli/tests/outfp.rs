//! Port of `outfp_test.go`: `TestOutFPChangesWithBinary` and `TestSigOfSeesReplacedFile` here.
//! `TestNoChangeUpdateSkipsWrites`, `TestChangedInputsTakeFullPath` (10 `t.Run` cases) and
//! `TestOverBudgetUpdateRepeatsWarning` set `GRAPHIFY_MTIME_GRANULARITY_MS=0` for whole runs, so
//! they run the binary (`crates/graphify/tests/cli_outfp.rs`).

use super::{out_fp_with, sig_of};
use crate::cli::maintenance::maint_util::git_run;
use crate::detect::WalkReport;
use crate::testutil::TempDir;

// Go `fpRoot` makes a committed git repo; `outFP` reads only its path here.
#[test]
fn test_out_fp_changes_with_binary() {
    let root = TempDir::new();
    git_run(root.path(), &["init", "-q"]);
    let r = root.path().to_str().expect("UTF-8 temp path");
    let walk = WalkReport::default();
    let a = out_fp_with(super::bin_id(), r, &walk, "c", "");
    assert!(
        out_fp_with("other", r, &walk, "c", "") != a,
        "fingerprint ignores binary identity"
    );
}

#[test]
fn test_sig_of_sees_replaced_file() {
    let dir = TempDir::new();
    let p = dir.write("g", b"a");
    let s = sig_of(&p);
    dir.write("g", b"bb");
    assert!(
        sig_of(&p) != s && sig_of(&dir.join("gx")).is_empty(),
        "sig did not change"
    );
}
