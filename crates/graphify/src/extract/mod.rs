//! Port of graphify-go `internal/extract`: the per-file extractors and the cross-file resolver.
//!
//! Shared API (plan Task 1): the `Result` types are fixed; the bodies come with
//! graphify-rs-ihc.16 (plan Task 15).
#![allow(dead_code, unused_variables, clippy::ptr_arg)]

mod bash;
pub(crate) mod builder;
mod c;
mod component;
mod cpp;
mod csharp;
mod csharp_resolve;
mod dispatch;
#[allow(clippy::module_inception)]
mod extract;
mod golang;
mod java;
mod javascript;
mod jsmodules;
mod json;
mod julia;
pub mod kinds;
mod kotlin;
mod lua;
mod markdown;
mod mcpconfig;
mod nulllabel;
mod nulllabel_resolve;
mod php;
mod python;
mod resolve;
mod resultjson;
mod ruby;
mod rust;
mod scala;
mod terraform;
mod verilog;
mod zig;

pub use extract::{
    Call, Def, Imp, ImportAlias, LabelInputs, MdRef, ModInvoke, ModRef, NullLabelRef, Result,
    SegVal, TypeRef, file, file_from_bytes,
};
pub use mcpconfig::is_mcp_config_path;
pub use resolve::{join, resolve, resolve_parts};

#[cfg(test)]
mod tests;
