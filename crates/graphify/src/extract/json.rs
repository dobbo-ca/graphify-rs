//! Port of `json.go`: config and manifest `.json` files become config-key nodes plus the
//! cross-file edges they show (`extends` chains, `$ref` schema refs, dependency imports). Data
//! JSON (fixtures, datasets, GeoJSON, API dumps) is skipped: recognition is by file name or a
//! top-level key probe; a malformed, oversized or non-config file gives an empty result.

use std::collections::HashSet;

use gositter::Node;
use gositter_grammars::json::language;

use super::builder::{line, node_text, parse_root};
use super::extract::Result;
use crate::idutil::{make_id, normalize_id};
use crate::model::{Confidence, Edge, Loc, file_of, rel as model_rel};
use crate::security::sanitize_label;

/// Go `jsonMaxBytes`: large fixture dumps and GeoJSON blobs are skipped.
const JSON_MAX_BYTES: usize = 1 << 20;
/// Go `jsonMaxPairs`, `jsonMaxDepth`: bounds of the walk.
const JSON_MAX_PAIRS: usize = 500;
const JSON_MAX_DEPTH: usize = 6;

/// Go `configJSONNames`: config and manifest basenames, matched lowercased.
const CONFIG_JSON_NAMES: [&str; 22] = [
    "package.json",
    "tsconfig.json",
    "jsconfig.json",
    "composer.json",
    "deno.json",
    "deno.jsonc",
    "bower.json",
    "manifest.json",
    "app.json",
    "now.json",
    "vercel.json",
    "angular.json",
    "nest-cli.json",
    "biome.json",
    "biome.jsonc",
    "renovate.json",
    ".babelrc",
    ".babelrc.json",
    ".eslintrc.json",
    ".prettierrc.json",
    ".prettierrc",
    "babel.config.json",
];

/// Go `configJSONKeys`: top-level keys that prove an object is a config or manifest.
const CONFIG_JSON_KEYS: [&str; 10] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
    "bundleDependencies",
    "bundledDependencies",
    "extends",
    "$ref",
    "$schema",
    "compilerOptions",
];

/// Go `depKeys`: blocks whose string values become imports.
const DEP_KEYS: [&str; 6] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
    "bundleDependencies",
    "bundledDependencies",
];

/// Go `degenerateName` (`javascript.go:499`): a name that normalises to an empty id.
fn degenerate_name(name: &str) -> bool {
    normalize_id(name).is_empty()
}

/// The per-file output of Go `extractJSON`'s closures.
struct Out {
    rel: String,
    res: Result,
    seen_node: HashSet<String>,
    seen_edge: HashSet<(String, String, String)>,
    pair_count: usize,
}

impl Out {
    /// Go `addNode` (the closure): a node once per id, its label sanitised.
    fn add_node(&mut self, id: &str, label: &str, loc: Loc, file_type: &str) {
        if id.is_empty() || !self.seen_node.insert(id.to_string()) {
            return;
        }
        self.res.nodes.push(crate::model::Node {
            id: id.to_string(),
            label: sanitize_label(label),
            file_type: file_type.to_string(),
            source_file: self.rel.clone(),
            source_location: loc.to_string(),
            ..crate::model::Node::default()
        });
    }

    /// Go `addEdge` (the closure): an EXTRACTED edge once per (source, target, relation); none
    /// with an empty end or a self loop.
    fn add_edge(&mut self, src: &str, tgt: &str, relation: &str, loc: Loc) {
        if src.is_empty() || tgt.is_empty() || src == tgt {
            return;
        }
        if !self
            .seen_edge
            .insert((src.to_string(), tgt.to_string(), relation.to_string()))
        {
            return;
        }
        self.res.edges.push(Edge {
            source: src.to_string(),
            target: tgt.to_string(),
            relation: model_rel(relation),
            confidence: Confidence::EXTRACTED,
            source_file: file_of(&self.rel),
            source_location: loc,
            ..Edge::default()
        });
    }

    /// Go `walkObject` (depth-bounded at 6, so the recursion stays shallow).
    fn walk_object(
        &mut self,
        obj: Node<'_>,
        src: &[u8],
        parent_id: &str,
        parent_key: &str,
        depth: usize,
    ) {
        if depth > JSON_MAX_DEPTH {
            return;
        }
        let stem = self.rel.clone();
        let file_id = make_id(&[&stem]);
        for i in 0..obj.child_count() {
            let Some(child) = obj.child(i) else { continue };
            if child.kind() != "pair" {
                continue;
            }
            if self.pair_count >= JSON_MAX_PAIRS {
                return;
            }
            self.pair_count += 1;
            let key = json_key_text(child, src);
            // A key with no identifier signal (empty, a JSONC `//` comment key) is noise (#1899).
            if degenerate_name(&key) {
                continue;
            }
            let loc = line(child);
            let mut key_parts: Vec<&str> = vec![&stem];
            if !parent_key.is_empty() {
                key_parts.push(parent_key);
            }
            key_parts.push(&key);
            let key_id = make_id(&key_parts);
            if key_id.is_empty() {
                continue;
            }
            self.add_node(&key_id, &key, loc, "code");
            self.add_edge(parent_id, &key_id, "contains", loc);
            let Some(val) = child.child_by_field_name("value") else {
                continue;
            };
            match val.kind() {
                "object" => self.walk_object(val, src, &key_id, &key, depth + 1),
                "array" => {
                    // `extends` arrays (tsconfig, eslint): each string element is a ref, its id
                    // namespaced with "ref" so it cannot collide with a code or file node.
                    for j in 0..val.child_count() {
                        let Some(item) = val.child(j) else { continue };
                        if item.kind() != "string" {
                            continue;
                        }
                        let r = json_string_content(item, src);
                        if r.is_empty() {
                            continue;
                        }
                        let ref_id = make_id(&["ref", &r]);
                        if !ref_id.is_empty() {
                            self.add_node(&ref_id, &r, loc, "concept");
                            self.add_edge(&key_id, &ref_id, "extends", loc);
                        }
                    }
                }
                "string" => {
                    let v = json_string_content(val, src);
                    if v.is_empty() {
                        continue;
                    }
                    if key == "extends" {
                        let ref_id = make_id(&["ref", &v]);
                        if !ref_id.is_empty() {
                            self.add_node(&ref_id, &v, loc, "concept");
                            self.add_edge(&file_id, &ref_id, "extends", loc);
                        }
                    } else if key == "$ref" {
                        let ref_id = make_id(&["ref", &v]);
                        if !ref_id.is_empty() {
                            self.add_edge(parent_id, &ref_id, "references", loc);
                        }
                    } else if DEP_KEYS.contains(&parent_key) {
                        let dep_id = make_id(&[&key]);
                        if !dep_id.is_empty() {
                            self.add_node(&dep_id, &key, loc, "concept");
                            self.add_edge(&key_id, &dep_id, "imports", loc);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Go `extractJSON`: `None` when the parse budget stops the parse.
pub(crate) fn extract_json(rel: &str, src: &[u8]) -> Option<Result> {
    if src.len() > JSON_MAX_BYTES {
        return Some(Result::default());
    }
    parse_root(src, language(), |root| {
        let mut doc = Some(root);
        if root.kind() == "document" && root.child_count() > 0 {
            doc = root.child(0);
        }
        // A top-level array or scalar is data JSON, never a config.
        let Some(doc) = doc.filter(|d| d.kind() == "object") else {
            return Result::default();
        };
        if !is_config_json(rel, doc, src) {
            return Result::default();
        }
        let file_id = make_id(&[rel]);
        let mut out = Out {
            rel: rel.to_string(),
            res: Result::default(),
            seen_node: HashSet::from([file_id.clone()]),
            seen_edge: HashSet::new(),
            pair_count: 0,
        };
        out.res.nodes.push(crate::model::Node {
            id: file_id.clone(),
            label: gostd::filepath::base(rel),
            file_type: "code".to_string(),
            source_file: rel.to_string(),
            source_location: "L1".to_string(),
            ..crate::model::Node::default()
        });
        out.walk_object(doc, src, &file_id, "", 0);
        out.res
    })
}

/// Go `isConfigJSON`: a config basename (or suffix), or a root object with a config key; a
/// JSON Schema document (`$schema` with `$defs`, `definitions` or `$id`) is not one.
fn is_config_json(rel: &str, obj: Node<'_>, src: &[u8]) -> bool {
    let name = gostd::strings::to_lower(&gostd::filepath::base(rel));
    if CONFIG_JSON_NAMES.contains(&name.as_str()) {
        return true;
    }
    if [
        ".eslintrc.json",
        ".prettierrc.json",
        ".babelrc.json",
        "tsconfig.json",
        "jsconfig.json",
    ]
    .iter()
    .any(|s| name.ends_with(s))
    {
        return true;
    }
    let mut root_keys = HashSet::new();
    for i in 0..obj.child_count() {
        if let Some(child) = obj.child(i).filter(|c| c.kind() == "pair") {
            root_keys.insert(json_key_text(child, src));
        }
    }
    let has = |k: &str| root_keys.contains(k);
    if has("$schema") && (has("$defs") || has("definitions") || has("$id")) {
        return false;
    }
    CONFIG_JSON_KEYS.iter().any(|k| has(k))
}

/// Go `jsonKeyText`: the string content of a pair's key, or `""`.
fn json_key_text(pair: Node<'_>, src: &[u8]) -> String {
    match pair.child_by_field_name("key") {
        None => String::new(),
        Some(k) if k.kind() == "string" => json_string_content(k, src),
        Some(k) => node_text(k, src).into_owned(),
    }
}

/// Go `jsonStringContent`: the `string_content` child's text, else the text with surrounding
/// quotes trimmed.
fn json_string_content(s: Node<'_>, src: &[u8]) -> String {
    for i in 0..s.child_count() {
        if let Some(c) = s.child(i).filter(|c| c.kind() == "string_content") {
            return node_text(c, src).into_owned();
        }
    }
    node_text(s, src).trim_matches(['"', '\'']).to_string()
}
