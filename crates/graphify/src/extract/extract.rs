//! Port of `extract.go`: the `Result` types, `File`, `FileFromBytes`, the extension dispatch and
//! `toUTF8`. The cache form of `Result` (`resultjson.go`) is in `resultjson.rs`.
//!
//! `Result` here is Go's `extract.Result`; std's is written `std::result::Result`.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use gostd::json::{DecodeState, EncodeState, Error as JsonError, Marshal, Unmarshal, write_field};

use super::builder::Builder;
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

gostd::json_fields!(decode Def, "extract.Def", {
    id => "ID",
    name => "Name",
    file => "File",
    owner => "Owner",
    is_abstract => "Abstract",
    value => "Value",
    export => "Export",
});

/// Go `extract.Call`: indexes into `Result::call_text`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Call {
    pub caller_id: u32,
    pub callee: u32,
    pub loc: Loc,
    pub recv: u32,
}

gostd::json_fields!(decode Call, "extract.Call", {
    caller_id => "CallerID",
    callee => "Callee",
    loc => "Loc",
    recv => "Recv",
});

/// Go `callText` (`extract.go:53`): a call with its texts looked up.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CallText {
    pub(crate) caller_id: String,
    pub(crate) callee: String,
    pub(crate) recv: String,
    pub(crate) loc: Loc,
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

gostd::json_fields!(decode ImportAlias, "extract.ImportAlias", {
    local => "Local",
    imported => "Imported",
    module_stem => "ModuleStem",
    spec => "Spec",
    loc => "Loc",
    in_func => "InFunc" omitempty,
});

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

gostd::json_fields!(decode TypeRef, "extract.TypeRef", {
    from_id => "FromID",
    name => "Name",
    relation => "Relation",
    file => "File",
    loc => "Loc",
    stub => "Stub" omitempty,
    deco => "Deco" omitempty,
    qual => "Qual" omitempty,
    arg => "Arg" omitempty,
});

/// Go `Imp.Dyn` values: a file's dynamic import, a function's deferred import.
pub(crate) const DYN_IMPORT: u8 = 1;
pub(crate) const DYN_DEFERRED: u8 = 2;

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
    /// Go `Dyn`: JS/TS `import(Spec)` ([`DYN_IMPORT`], [`DYN_DEFERRED`]).
    pub dynamic: u8,
    pub names: Vec<String>,
}

/// Go's struct encoding of `Imp` (`Dyn` as a number, omitempty).
impl Marshal for Imp {
    fn marshal_json(&self, e: &mut EncodeState) -> std::result::Result<(), JsonError> {
        let mut first = true;
        write_field(e, &mut first, "FileID", &self.file_id, false)?;
        write_field(e, &mut first, "File", &self.file, false)?;
        write_field(e, &mut first, "Spec", &self.spec, false)?;
        write_field(e, &mut first, "Loc", &self.loc, false)?;
        write_field(e, &mut first, "TypeOnly", &self.type_only, false)?;
        write_field(e, &mut first, "Rel", &self.rel, false)?;
        write_field(e, &mut first, "Export", &self.export, !self.export)?;
        write_field(
            e,
            &mut first,
            "Dyn",
            &u32::from(self.dynamic),
            self.dynamic == 0,
        )?;
        write_field(e, &mut first, "Names", &self.names, false)?;
        e.write_raw(b"}");
        Ok(())
    }
}

/// Go's struct decoding of `Imp`; `Dyn` goes through a `uint32` (gostd has no `uint8`).
impl Unmarshal for Imp {
    const GO_TYPE: &'static str = "extract.Imp";

    fn unmarshal_json(&mut self, d: &mut DecodeState<'_>) -> std::result::Result<(), JsonError> {
        const KEYS: [&str; 9] = [
            "FileID", "File", "Spec", "Loc", "TypeOnly", "Rel", "Export", "Dyn", "Names",
        ];
        d.read_struct(&KEYS, |d, key| match key {
            "FileID" => d.value(&mut self.file_id),
            "File" => d.value(&mut self.file),
            "Spec" => d.value(&mut self.spec),
            "Loc" => d.value(&mut self.loc),
            "TypeOnly" => d.value(&mut self.type_only),
            "Rel" => d.value(&mut self.rel),
            "Export" => d.value(&mut self.export),
            "Dyn" => {
                let mut v = u32::from(self.dynamic);
                d.value(&mut v)?;
                self.dynamic = v as u8;
                Ok(())
            }
            _ => d.value(&mut self.names),
        })
    }
}

/// Go `extract.ModRef`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModRef {
    pub from_id: String,
    pub source: String,
    pub file: String,
    pub loc: Loc,
}

gostd::json_fields!(decode ModRef, "extract.ModRef", {
    from_id => "FromID",
    source => "Source",
    file => "File",
    loc => "Loc",
});

/// Go `labelInputs` (`nulllabel.go:25`). Its fields are unexported in Go, so its JSON is `{}`
/// and decoding keeps nothing; they come with graphify-rs-ihc.31.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LabelInputs {}

/// Go `segVal` (`nulllabel.go:19`). JSON `{}` as [`LabelInputs`]; fields with graphify-rs-ihc.31.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SegVal {}

/// A Go struct with no exported field: `{}` out, any value skipped in.
macro_rules! empty_go_struct {
    ($($t:ty => $go:literal),*) => {$(
        impl Marshal for $t {
            fn marshal_json(&self, e: &mut EncodeState) -> std::result::Result<(), JsonError> {
                e.write_raw(b"{}");
                Ok(())
            }
        }

        impl Unmarshal for $t {
            const GO_TYPE: &'static str = $go;

            fn unmarshal_json(
                &mut self,
                d: &mut DecodeState<'_>,
            ) -> std::result::Result<(), JsonError> {
                d.read_struct(&[], |_, _| Ok(()))
            }
        }
    )*};
}

empty_go_struct!(LabelInputs => "extract.labelInputs", SegVal => "extract.segVal");

/// Go `extract.NullLabelRef`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NullLabelRef {
    pub node_id: String,
    pub scope: String,
    pub inputs: LabelInputs,
}

gostd::json_fields!(decode NullLabelRef, "extract.NullLabelRef", {
    node_id => "NodeID",
    scope => "Scope",
    inputs => "Inputs",
});

/// Go `extract.ModInvoke`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModInvoke {
    pub node_id: String,
    pub dir: String,
    pub args: BTreeMap<String, SegVal>,
    pub arg_var_refs: BTreeMap<String, String>,
}

gostd::json_fields!(decode ModInvoke, "extract.ModInvoke", {
    node_id => "NodeID",
    dir => "Dir",
    args => "Args",
    arg_var_refs => "ArgVarRefs",
});

/// Go `extract.MDRef` (`markdown.go`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MdRef {
    pub from_id: String,
    pub target: String,
    pub file: String,
    pub loc: Loc,
}

gostd::json_fields!(decode MdRef, "extract.MDRef", {
    from_id => "FromID",
    target => "Target",
    file => "File",
    loc => "Loc",
});

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

gostd::json_fields!(decode Result, "extract.Result", {
    nodes => "Nodes",
    edges => "Edges",
    defs => "Defs",
    calls => "Calls",
    call_text => "CallText",
    file => "File",
    call_mods => "CallMods",
    type_refs => "TypeRefs",
    imps => "Imps",
    mod_refs => "ModRefs",
    null_labels => "NullLabels",
    mod_invokes => "ModInvokes",
    import_aliases => "ImportAliases",
    md_refs => "MDRefs",
    go_mods => "GoMods",
    default => "Default" omitempty,
    exports => "Exports" omitempty,
    type_aliases => "TypeAliases" omitempty,
    java_rets => "JavaRets" omitempty,
    cs_types => "CsTypes" omitempty,
    over_budget => "OverBudget" omitempty,
});

impl Result {
    /// Go `Result.call` (`extract.go:58`): the texts of `c`.
    pub(crate) fn call(&self, c: &Call) -> CallText {
        let t = &self.call_text;
        CallText {
            caller_id: t[c.caller_id as usize].clone(),
            callee: t[c.callee as usize].clone(),
            recv: t[c.recv as usize].clone(),
            loc: c.loc,
        }
    }

    /// Go `Result.CallsOK`: every call index is inside `call_text` and its first entry is empty.
    pub fn calls_ok(&self) -> bool {
        let n = self.call_text.len();
        let inside = |i: u32| (i as usize) < n;
        self.calls
            .iter()
            .all(|c| inside(c.caller_id) && inside(c.callee) && inside(c.recv))
            && (n == 0 || self.call_text[0].is_empty())
    }
}

/// Go `extract.File`: reads `root/rel` and extracts it. A read error is Go's `open <path>: ...`.
pub fn file(root: &Path, rel: &str) -> io::Result<Result> {
    let path = root.join(rel);
    let src = std::fs::read(&path).map_err(|e| {
        let shown = gostd::filepath::join(&[&*root.to_string_lossy(), rel]);
        crate::goerr::path_error("open", Path::new(&shown), &e)
    })?;
    Ok(file_from_bytes(rel, &src))
}

/// Go `extract.FileFromBytes`: extracts `src` as the file `rel` (slash-separated). A parse the
/// budget stops gives the file node alone with `over_budget` set (Go's `recover` of
/// `ts.ErrBudget`). Unsupported extensions give an empty result.
pub fn file_from_bytes(rel: &str, src: &[u8]) -> Result {
    super::kinds::register_kinds();
    match file_from_bytes_inner(rel, src) {
        Some(r) => r,
        None => {
            let mut r = Builder::new(rel).res;
            r.over_budget = true;
            r
        }
    }
}

/// Go `fileFromBytes` (`extract.go:239-307`): `to_utf8`, the MCP config check, then the
/// extension (lowercased Go `filepath.Ext`, or the shebang's for an extensionless file).
pub(crate) fn file_from_bytes_inner(rel: &str, src: &[u8]) -> Option<Result> {
    let src = to_utf8(src);
    let src: &[u8] = &src;
    if super::mcpconfig::is_mcp_config_path(rel) {
        return Some(super::mcpconfig::extract_mcp_config(rel, src));
    }
    let mut ext = gostd::strings::to_lower(gostd::filepath::ext(rel));
    if ext.is_empty() {
        ext = crate::detect::shebang_ext(src).to_string();
    }
    use gositter_grammars::{javascript, tsx, typescript};
    match ext.as_str() {
        ".go" => super::golang::extract_go(rel, src),
        ".js" | ".jsx" | ".mjs" | ".cjs" => {
            super::javascript::extract_js_file(rel, src, javascript::language())
        }
        ".tsx" => super::javascript::extract_js_file(rel, src, tsx::language()),
        ".ts" | ".mts" | ".cts" => {
            super::javascript::extract_js_file(rel, src, typescript::language())
        }
        ".tf" | ".tfvars" | ".hcl" => super::terraform::extract_terraform(rel, src),
        ".json" => super::json::extract_json(rel, src),
        ".py" => super::python::extract_python(rel, src),
        ".rs" => super::rust::extract_rust(rel, src),
        ".h" if is_cpp_header(src) => super::cpp::extract_cpp(rel, src),
        ".h" | ".c" => super::c::extract_c(rel, src),
        ".cpp" | ".cc" | ".cxx" | ".hpp" | ".hh" | ".hxx" => super::cpp::extract_cpp(rel, src),
        ".java" => super::java::extract_java(rel, src),
        ".cs" => super::csharp::extract_csharp(rel, src),
        ".rb" | ".rake" => super::ruby::extract_ruby(rel, src),
        ".php" | ".phtml" => super::php::extract_php(rel, src),
        ".sh" | ".bash" => super::bash::extract_bash(rel, src),
        ".scala" | ".sc" => super::scala::extract_scala(rel, src),
        ".jl" => super::julia::extract_julia(rel, src),
        ".v" | ".sv" | ".svh" | ".vh" => super::verilog::extract_verilog(rel, src),
        ".kt" | ".kts" => super::kotlin::extract_kotlin(rel, src),
        ".lua" => super::lua::extract_lua(rel, src),
        ".zig" => super::zig::extract_zig(rel, src),
        ".md" | ".mdx" | ".markdown" | ".skill" => {
            Some(super::markdown::extract_markdown(rel, src))
        }
        ".vue" | ".svelte" => super::component::extract_component(rel, src, false),
        ".astro" => super::component::extract_component(rel, src, true),
        _ => Some(Result::default()),
    }
}

/// Go `cppHeaderMarkers`: tokens no plain C header contains.
const CPP_HEADER_MARKERS: [&[u8]; 7] = [
    b"class ",
    b"namespace ",
    b"template",
    b"::",
    b"public:",
    b"private:",
    b"protected:",
];

/// Go `isCppHeader`: a `.h` whose first 256 KiB hold a C++ token.
pub(crate) fn is_cpp_header(src: &[u8]) -> bool {
    let head = &src[..src.len().min(256 * 1024)];
    CPP_HEADER_MARKERS
        .iter()
        .any(|m| head.windows(m.len()).any(|w| w == *m))
}

/// Go `toUTF8`: valid UTF-8 is kept; UTF-16 with a byte-order mark is decoded (an unpaired
/// surrogate becomes U+FFFD, an odd last byte is dropped); anything else is read as latin-1.
pub(crate) fn to_utf8(src: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use std::borrow::Cow;
    if std::str::from_utf8(src).is_ok() {
        return Cow::Borrowed(src);
    }
    if src.len() >= 2 && matches!((src[0], src[1]), (0xFF, 0xFE) | (0xFE, 0xFF)) {
        let le = src[0] == 0xFF;
        let units = src[2..].as_chunks::<2>().0.iter().map(|&[a, b]| {
            if le {
                u16::from_le_bytes([a, b])
            } else {
                u16::from_be_bytes([a, b])
            }
        });
        let s: String = char::decode_utf16(units)
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        return Cow::Owned(s.into_bytes());
    }
    Cow::Owned(
        src.iter()
            .map(|&b| char::from(b))
            .collect::<String>()
            .into_bytes(),
    )
}

/// Go `toUTF8` (`extract.go:625`): valid UTF-8 is kept (borrowed); UTF-16 with a BOM is decoded
/// (an unpaired surrogate becomes U+FFFD, an odd last byte is dropped); anything else is latin-1.
pub(crate) fn to_utf8(src: &[u8]) -> Cow<'_, [u8]> {
    todo!("graphify-rs-ihc.16")
}

/// Go `isCppHeader` (`extract.go:316`): a `.h` whose first 256 KiB hold a C++ token.
pub(crate) fn is_cpp_header(src: &[u8]) -> bool {
    todo!("graphify-rs-ihc.16")
}

/// Go `Imp.Dyn` values (`extract.go:553-556`): a file's dynamic import, a function's deferred
/// import.
pub(crate) const DYN_IMPORT: u8 = 1;
pub(crate) const DYN_DEFERRED: u8 = 2;
