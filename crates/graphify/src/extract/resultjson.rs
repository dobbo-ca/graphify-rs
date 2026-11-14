//! Port of `resultjson.go`: the cache form of a `Result`, appended by hand in Go's
//! `json.Marshal` field order. An empty list is written `null` (Go: a nil slice); byte parity of
//! the cache is a non-goal (spec section 1), and decoding reads `null` and `[]` alike.

use gostd::json::{Error as JsonError, Marshal, marshal};

use super::extract::Result;
use crate::model::{Edge, Node, append_json_float, append_json_string};

impl Result {
    /// Go `Result.AppendJSON` (`resultjson.go:15`).
    pub fn append_json(&self, b: &mut Vec<u8>) -> std::result::Result<(), JsonError> {
        b.extend_from_slice(br#"{"Nodes":"#);
        arr(b, &self.nodes, append_node)?;
        b.extend_from_slice(br#","Edges":"#);
        arr(b, &self.edges, append_edge)?;
        b.extend_from_slice(br#","Defs":"#);
        arr(b, &self.defs, |b, d| {
            fields(
                b,
                &[
                    (r#"{"ID":"#, &d.id),
                    (r#","Name":"#, &d.name),
                    (r#","File":"#, &d.file),
                ],
            );
            fields(b, &[(r#","Owner":"#, &d.owner)]);
            boolean(b, r#","Abstract":"#, d.is_abstract);
            boolean(b, r#","Value":"#, d.value);
            boolean(b, r#","Export":"#, d.export);
            b.push(b'}');
            Ok(())
        })?;
        b.extend_from_slice(br#","Calls":"#);
        arr(b, &self.calls, |b, c| {
            b.extend_from_slice(
                format!(r#"{{"CallerID":{},"Callee":{}"#, c.caller_id, c.callee).as_bytes(),
            );
            b.extend_from_slice(br#","Loc":"#);
            c.loc.append_json(b);
            b.extend_from_slice(format!(r#","Recv":{}}}"#, c.recv).as_bytes());
            Ok(())
        })?;
        b.extend_from_slice(br#","CallText":"#);
        strings(b, &self.call_text);
        fields(b, &[(r#","File":"#, &self.file)]);
        b.extend_from_slice(br#","CallMods":"#);
        strings(b, &self.call_mods);
        b.extend_from_slice(br#","TypeRefs":"#);
        arr(b, &self.type_refs, |b, t| {
            fields(
                b,
                &[
                    (r#"{"FromID":"#, &t.from_id),
                    (r#","Name":"#, &t.name),
                    (r#","Relation":"#, &t.relation),
                    (r#","File":"#, &t.file),
                ],
            );
            b.extend_from_slice(br#","Loc":"#);
            t.loc.append_json(b);
            if t.stub {
                b.extend_from_slice(br#","Stub":true"#);
            }
            if t.deco {
                b.extend_from_slice(br#","Deco":true"#);
            }
            if !t.qual.is_empty() {
                fields(b, &[(r#","Qual":"#, &t.qual)]);
            }
            if t.arg {
                b.extend_from_slice(br#","Arg":true"#);
            }
            b.push(b'}');
            Ok(())
        })?;
        b.extend_from_slice(br#","Imps":"#);
        arr(b, &self.imps, |b, i| {
            fields(
                b,
                &[
                    (r#"{"FileID":"#, &i.file_id),
                    (r#","File":"#, &i.file),
                    (r#","Spec":"#, &i.spec),
                ],
            );
            b.extend_from_slice(br#","Loc":"#);
            i.loc.append_json(b);
            boolean(b, r#","TypeOnly":"#, i.type_only);
            boolean(b, r#","Rel":"#, i.rel);
            if i.export {
                b.extend_from_slice(br#","Export":true"#);
            }
            if i.dynamic != 0 {
                b.extend_from_slice(format!(r#","Dyn":{}"#, i.dynamic).as_bytes());
            }
            b.extend_from_slice(br#","Names":"#);
            strings(b, &i.names);
            b.push(b'}');
            Ok(())
        })?;
        b.extend_from_slice(br#","ModRefs":"#);
        any(b, self.mod_refs.is_empty(), &self.mod_refs)?;
        b.extend_from_slice(br#","NullLabels":"#);
        any(b, self.null_labels.is_empty(), &self.null_labels)?;
        b.extend_from_slice(br#","ModInvokes":"#);
        any(b, self.mod_invokes.is_empty(), &self.mod_invokes)?;
        b.extend_from_slice(br#","ImportAliases":"#);
        arr(b, &self.import_aliases, |b, a| {
            fields(
                b,
                &[
                    (r#"{"Local":"#, &a.local),
                    (r#","Imported":"#, &a.imported),
                    (r#","ModuleStem":"#, &a.module_stem),
                    (r#","Spec":"#, &a.spec),
                ],
            );
            b.extend_from_slice(br#","Loc":"#);
            a.loc.append_json(b);
            if a.in_func {
                b.extend_from_slice(br#","InFunc":true"#);
            }
            b.push(b'}');
            Ok(())
        })?;
        b.extend_from_slice(br#","MDRefs":"#);
        arr(b, &self.md_refs, |b, m| {
            fields(
                b,
                &[
                    (r#"{"FromID":"#, &m.from_id),
                    (r#","Target":"#, &m.target),
                    (r#","File":"#, &m.file),
                ],
            );
            b.extend_from_slice(br#","Loc":"#);
            m.loc.append_json(b);
            b.push(b'}');
            Ok(())
        })?;
        b.extend_from_slice(br#","GoMods":"#);
        any(b, self.go_mods.is_empty(), &self.go_mods)?;
        if !self.default.is_empty() {
            fields(b, &[(r#","Default":"#, &self.default)]);
        }
        for (key, v) in [
            (r#","Exports":"#, &self.exports),
            (r#","TypeAliases":"#, &self.type_aliases),
            (r#","JavaRets":"#, &self.java_rets),
            (r#","CsTypes":"#, &self.cs_types),
        ] {
            if !v.is_empty() {
                b.extend_from_slice(key.as_bytes());
                strings(b, v);
            }
        }
        if self.over_budget {
            b.extend_from_slice(br#","OverBudget":true"#);
        }
        b.push(b'}');
        Ok(())
    }
}

type Elem<'a, T> = &'a dyn Fn(&mut Vec<u8>, &T) -> std::result::Result<(), JsonError>;

/// Go `appendJSONArr`: `null` for an empty list (Go: a nil slice), else `[...]`.
fn arr<T>(
    b: &mut Vec<u8>,
    s: &[T],
    elem: impl Fn(&mut Vec<u8>, &T) -> std::result::Result<(), JsonError>,
) -> std::result::Result<(), JsonError> {
    let elem: Elem<'_, T> = &elem;
    if s.is_empty() {
        b.extend_from_slice(b"null");
        return Ok(());
    }
    b.push(b'[');
    for (i, x) in s.iter().enumerate() {
        if i > 0 {
            b.push(b',');
        }
        elem(b, x)?;
    }
    b.push(b']');
    Ok(())
}

/// An array of strings.
fn strings(b: &mut Vec<u8>, s: &[String]) {
    let _ = arr(b, s, |b, x| {
        append_json_string(b, x);
        Ok(())
    });
}

/// Go `appendJSONAny`: `null`, or `json.Marshal(v)`.
fn any<T: Marshal + ?Sized>(
    b: &mut Vec<u8>,
    is_nil: bool,
    v: &T,
) -> std::result::Result<(), JsonError> {
    if is_nil {
        b.extend_from_slice(b"null");
        return Ok(());
    }
    b.extend_from_slice(&marshal(v)?);
    Ok(())
}

/// Go `appendJSONFields`: each key (with its punctuation) and its value as a JSON string.
fn fields(b: &mut Vec<u8>, kv: &[(&str, &String)]) {
    for (k, v) in kv {
        b.extend_from_slice(k.as_bytes());
        append_json_string(b, v);
    }
}

/// `key` then `true`/`false` (Go `appendJSONBool` with the separator moved into the key).
fn boolean(b: &mut Vec<u8>, key: &str, v: bool) {
    b.extend_from_slice(key.as_bytes());
    b.extend_from_slice(if v { b"true" } else { b"false" });
}

/// Go `appendJSONNode`.
fn append_node(b: &mut Vec<u8>, n: &Node) -> std::result::Result<(), JsonError> {
    fields(
        b,
        &[
            (r#"{"id":"#, &n.id),
            (r#","label":"#, &n.label),
            (r#","file_type":"#, &n.file_type),
            (r#","source_file":"#, &n.source_file),
        ],
    );
    if !n.source_location.is_empty() {
        fields(b, &[(r#","source_location":"#, &n.source_location)]);
    }
    if !n.computed_name.is_empty() {
        fields(b, &[(r#","computed_name":"#, &n.computed_name)]);
    }
    if !n.attributes.is_empty() {
        b.extend_from_slice(br#","attributes":"#);
        b.extend_from_slice(&marshal(&n.attributes)?);
    }
    b.push(b'}');
    Ok(())
}

/// Go `appendJSONEdge`.
fn append_edge(b: &mut Vec<u8>, e: &Edge) -> std::result::Result<(), JsonError> {
    let (rel, conf) = (
        e.relation.as_str().to_string(),
        e.confidence.as_str().to_string(),
    );
    fields(
        b,
        &[
            (r#"{"source":"#, &e.source),
            (r#","target":"#, &e.target),
            (r#","relation":"#, &rel),
            (r#","confidence":"#, &conf),
        ],
    );
    if e.type_only() {
        b.extend_from_slice(br#","type_only":true"#);
    }
    if e.deferred() {
        b.extend_from_slice(br#","deferred":true"#);
    }
    if e.source_file.0 != 0 {
        fields(
            b,
            &[(r#","source_file":"#, &e.source_file.as_str().to_string())],
        );
    }
    let l = e.loc();
    if l.0 != 0 {
        b.extend_from_slice(br#","source_location":"#);
        l.append_json(b);
    }
    if e.weight != 0.0 {
        b.extend_from_slice(br#","weight":"#);
        append_json_float(b, e.weight)?;
    }
    if e.confidence_score != 0.0 {
        b.extend_from_slice(br#","confidence_score":"#);
        append_json_float(b, e.confidence_score)?;
    }
    b.push(b'}');
    Ok(())
}
