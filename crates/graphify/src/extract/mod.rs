//! Port of graphify-go `internal/extract`: the per-file extractors and the cross-file resolver.
//!
//! Shared API (plan Task 1): the `Result` types are fixed; the bodies come with
//! graphify-rs-ihc.16 (plan Task 15).
#![allow(dead_code, unused_variables)]

#[allow(clippy::module_inception)]
mod extract;

pub use extract::{
    Call, Def, Imp, ImportAlias, LabelInputs, MdRef, ModInvoke, ModRef, NullLabelRef, Result,
    SegVal, TypeRef, file, file_from_bytes,
};
