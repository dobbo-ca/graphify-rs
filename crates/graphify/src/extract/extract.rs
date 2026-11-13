//! Port of `extract.go` (the `Result` types, `File`, `FileFromBytes`) and the `Result` JSON of
//! `resultjson.go`. Field names follow Go; the cache stores `Result` as JSON with Go's field
//! names as keys.
//!
//! `Result` here is Go's `extract.Result`; std's is written `std::result::Result`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;

use gostd::json::{DecodeState, Error as JsonError, Unmarshal};

use crate::model::{Edge, Loc, Node};

/// Go `extract.Def`: a definition a call or reference can bind to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Def {
    pub id: String,
    pub name: String,
    pub file: String,
    pub owner: String,
    /// Go `Abstract`: a bodiless declaration.
    pub is_abstract: bool,
    /// Go `Value`: a const, no markdown span names it.
    pub value: bool,
    /// Go `Export`: a member of an exported JS object.
    pub export: bool,
}

/// Go `extract.Call`: indexes into `Result::call_text`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Call {
    pub caller_id: u32,
    pub callee: u32,
    pub loc: Loc,
    pub recv: u32,
}

/// Go `extract.ImportAlias`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportAlias {
    pub local: String,
    pub imported: String,
    pub module_stem: String,
    pub spec: String,
    pub loc: Loc,
    pub in_func: bool,
}

/// Go `extract.TypeRef`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeRef {
    pub from_id: String,
    pub name: String,
    pub relation: String,
    pub file: String,
    pub loc: Loc,
    pub stub: bool,
    pub deco: bool,
    pub qual: String,
    pub arg: bool,
}

/// Go `extract.Imp`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imp {
    pub file_id: String,
    pub file: String,
    pub spec: String,
    pub loc: Loc,
    pub type_only: bool,
    pub rel: bool,
    pub export: bool,
    /// Go `Dyn`: JS/TS `import(Spec)`.
    pub dynamic: u8,
    pub names: Vec<String>,
}

/// Go `extract.ModRef`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModRef {
    pub from_id: String,
    pub source: String,
    pub file: String,
    pub loc: Loc,
}

/// Go `labelInputs` (`nulllabel.go:25`). Its fields are unexported in Go, so its JSON is `{}`;
/// they come with graphify-rs-ihc.31.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabelInputs {}

/// Go `segVal` (`nulllabel.go:19`). JSON `{}` as [`LabelInputs`]; fields with graphify-rs-ihc.31.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SegVal {}

/// Go `extract.NullLabelRef`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NullLabelRef {
    pub node_id: String,
    pub scope: String,
    pub inputs: LabelInputs,
}

/// Go `extract.ModInvoke`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModInvoke {
    pub node_id: String,
    pub dir: String,
    pub args: BTreeMap<String, SegVal>,
    pub arg_var_refs: BTreeMap<String, String>,
}

/// Go `extract.MDRef` (`markdown.go`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MdRef {
    pub from_id: String,
    pub target: String,
    pub file: String,
    pub loc: Loc,
}

/// Go `callText` (`extract.go:53`): a call with its texts looked up.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CallText {
    pub(crate) caller_id: String,
    pub(crate) callee: String,
    pub(crate) recv: String,
    pub(crate) loc: Loc,
}

/// Go `extract.Result`: one file's extraction plus the facts the resolver binds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Result {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub defs: Vec<Def>,
    pub calls: Vec<Call>,
    /// The texts of the calls; the first is empty.
    pub call_text: Vec<String>,
    /// The file the calls were made in.
    pub file: String,
    pub call_mods: Vec<String>,
    pub type_refs: Vec<TypeRef>,
    pub imps: Vec<Imp>,
    pub mod_refs: Vec<ModRef>,
    pub null_labels: Vec<NullLabelRef>,
    pub mod_invokes: Vec<ModInvoke>,
    pub import_aliases: Vec<ImportAlias>,
    pub md_refs: Vec<MdRef>,
    /// A go.mod's directory to its module path (manifest pass only).
    pub go_mods: BTreeMap<String, String>,
    /// Go `Default`: the local name a JS/TS file exports as its default.
    pub default: String,
    pub exports: Vec<String>,
    pub type_aliases: Vec<String>,
    pub java_rets: Vec<String>,
    pub cs_types: Vec<String>,
    /// The parse budget stopped the parse; only the file node is here.
    pub over_budget: bool,
}

impl Result {
    /// Go `Result.call` (`extract.go:58`): the texts of `c`.
    pub(crate) fn call(&self, c: &Call) -> CallText {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `Result.CallsOK`: every call index is inside `call_text` and its first entry is empty.
    pub fn calls_ok(&self) -> bool {
        todo!("graphify-rs-ihc.16")
    }

    /// Go `Result.AppendJSON` (`resultjson.go:15`): the cache form of the result.
    pub fn append_json(&self, dst: &mut Vec<u8>) -> std::result::Result<(), JsonError> {
        todo!("graphify-rs-ihc.16")
    }
}

impl Unmarshal for Result {
    const GO_TYPE: &'static str = "extract.Result";
    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> std::result::Result<(), JsonError> {
        todo!("graphify-rs-ihc.16")
    }
}

/// Go `extract.File`: reads `root/rel` and extracts it.
pub fn file(root: &Path, rel: &str) -> std::io::Result<Result> {
    todo!("graphify-rs-ihc.16")
}

/// Go `extract.FileFromBytes`: extracts `src` as the file `rel` (slash-separated).
pub fn file_from_bytes(rel: &str, src: &[u8]) -> Result {
    todo!("graphify-rs-ihc.16")
}

/// Go `toUTF8` (`extract.go:625`): valid UTF-8 is kept (borrowed); UTF-16 with a BOM is decoded
/// (an unpaired surrogate becomes U+FFFD, an odd last byte is dropped); anything else is latin-1.
pub(crate) fn to_utf8(src: &[u8]) -> Cow<'_, [u8]> {
    todo!("graphify-rs-ihc.16")
}
