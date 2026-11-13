//! golang.org/x/text v0.27.0 (UCD 15.0.0): `cases.Fold` and `norm.NFKC` / `norm.NFKD`, as
//! graphify-go calls them (`internal/idutil/idutil.go:23,86`, `internal/export/export.go:443`).
//! These use Unicode 15.0.0, not the 17.0.0 of `crate::unicode`: x/text v0.27.0 ships
//! `tables15.0.0.go`.

pub mod cases;
pub mod norm;
