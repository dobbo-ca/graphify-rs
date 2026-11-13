//! Port of `okf.go`: the Open Knowledge Format bundle.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gostd::filepath;
use gostd::path;
use gostd::sort;
use gostd::strconv::quote;
use gostd::strings::trim_space;

use super::export::{Error, JsonGraph, JsonNode, mkdir_all, write_file};
use super::formats::read_graph_json;
use crate::idutil::make_id;
use crate::security::sanitize_label;

// --- OKF (Open Knowledge Format) bundle ---
//
// `okf_from_json` serializes the graph at a graph.json as an Open Knowledge Format bundle under
// an output directory: one markdown concept document per node (YAML frontmatter + a "# Relations"
// section of bundle-relative links), plus an index.md at each directory level. The bundle is
// human- and agent-friendly, SDK-free, and byte-stable across rebuilds (nodes, edges, and
// directory entries are sorted), so it diffs cleanly. Only graph metadata is emitted, never
// source file bodies.
//
// Concept paths mirror the source tree: a node from src/a.go lands at /src/a.go/<id>.md, and
// cross-links use the absolute, bundle-relative form (leading "/", ".md" suffix) recommended by
// the OKF spec section 5.1.

/// Go `concept` (`okf.go:31`): one node placed at its bundle-relative path (no leading slash, no
/// .md suffix in id; "link" is the absolute form used inside relation lists).
#[derive(Clone)]
struct Concept<'a> {
    node: &'a JsonNode,
    /// Bundle-relative directory, e.g. "src/a.go" ("" = root).
    dir: String,
    /// File name without extension, e.g. "src_a_go_add".
    file: String,
}

impl Concept<'_> {
    /// Go `concept.link` (`okf.go:37`).
    fn link(&self) -> String {
        format!("/{}.md", path::join(&[&self.dir, &self.file]))
    }
}

/// Go `OKFFromJSON` (`okf.go:42`).
pub fn okf_from_json(json_path: &Path, out_dir: &Path) -> Result<(), Error> {
    let g = read_graph_json(json_path)?;
    let abs_out = filepath::abs(out_dir)?;
    mkdir_all(&abs_out)?;

    // Place every node at a bundle path derived from its source file, then index by ID so
    // relation links can resolve endpoints.
    let nodes = g.nodes.as_deref().unwrap_or_default();
    let mut concepts: Vec<Concept> = Vec::with_capacity(nodes.len());
    let mut by_id: HashMap<&str, Concept> = HashMap::with_capacity(nodes.len());
    for n in nodes {
        let c = Concept {
            node: n,
            dir: concept_dir(&n.source_file),
            file: make_id(&[&n.id]),
        };
        concepts.push(c.clone());
        by_id.insert(&n.id, c);
    }
    sort::slice(&mut concepts, |a, b| a.file < b.file);

    // Group edges by node so each concept lists its own relations (both directions), mirroring
    // `graphify explain`.
    let rels = relations_by_node(&g, &by_id);

    let mut dirs = BTreeSet::from([String::new()]);
    for c in &concepts {
        let body = concept_doc(
            c,
            rels.get(c.node.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        let full = safe_join(&abs_out, &c.dir, &format!("{}.md", c.file))?;
        mkdir_all(full.parent().unwrap_or(Path::new(".")))?;
        write_file(&full, body.as_bytes())?;
        let mut d = c.dir.as_str();
        loop {
            dirs.insert(d.to_string());
            if d.is_empty() {
                break;
            }
            d = parent_dir(d);
        }
    }

    write_indexes(&abs_out, &dirs, &concepts)
}

/// Go `relation` (`okf.go:97`): one edge as seen from a node: its direction marker, relation
/// name, and the absolute bundle link to the other endpoint.
struct Relation {
    rel: String,
    dir: &'static str,
    label: String,
    link: String,
}

/// Go `relationsByNode` (`okf.go:101`).
fn relations_by_node<'a>(
    g: &'a JsonGraph,
    by_id: &HashMap<&str, Concept>,
) -> BTreeMap<&'a str, Vec<Relation>> {
    let mut out: BTreeMap<&str, Vec<Relation>> = BTreeMap::new();
    let mut add = |from: &'a str, rel: &str, dir: &'static str, other: &str| {
        let Some(c) = by_id.get(other) else {
            return;
        };
        out.entry(from).or_default().push(Relation {
            rel: rel.to_string(),
            dir,
            label: sanitize_label(&c.node.label),
            link: c.link(),
        });
    };
    for e in g.links.as_deref().unwrap_or_default() {
        add(&e.source, &e.relation, "->", &e.target);
        add(&e.target, &e.relation, "<-", &e.source);
    }
    for r in out.values_mut() {
        sort::slice(r, |a, b| {
            if a.rel != b.rel {
                return a.rel < b.rel;
            }
            if a.dir != b.dir {
                return a.dir < b.dir;
            }
            a.link < b.link
        });
    }
    out
}

/// Go `conceptDoc` (`okf.go:134`): renders one node as an OKF concept document: YAML frontmatter
/// (with the required `type` field) followed by a "# Source" line and a grouped "# Relations"
/// section.
fn concept_doc(c: &Concept, rels: &[Relation]) -> String {
    let n = c.node;
    let kind = if n.file_type.is_empty() {
        "concept"
    } else {
        n.file_type.as_str()
    };
    let mut desc = format!("degree {}", rels.len());
    if let Some(comm) = n.community {
        desc = format!("community {comm}, {desc}");
    }
    let mut resource = n.source_file.clone();
    if !n.source_location.is_empty() {
        resource.push(':');
        resource.push_str(&n.source_location);
    }

    let mut b = String::new();
    b.push_str("---\n");
    let _ = writeln!(b, "type: {}", yaml_string(kind));
    let _ = writeln!(b, "title: {}", yaml_string(&sanitize_label(&n.label)));
    let _ = writeln!(b, "description: {}", yaml_string(&desc));
    if !resource.is_empty() {
        let _ = writeln!(b, "resource: {}", yaml_string(&sanitize_label(&resource)));
    }
    b.push_str("tags:\n");
    if let Some(comm) = n.community {
        let _ = writeln!(b, "  - {}", yaml_string(&format!("community:{comm}")));
    }
    b.push_str("---\n\n");

    let _ = write!(b, "# {}\n\n", sanitize_label(&n.label));
    if !resource.is_empty() {
        let _ = write!(b, "# Source\n\n{}\n\n", sanitize_label(&resource));
    }

    b.push_str("# Relations\n\n");
    if rels.is_empty() {
        b.push_str("_none_\n");
        return b;
    }
    let mut last_rel = "";
    for r in rels {
        if r.rel != last_rel {
            if !last_rel.is_empty() {
                b.push('\n');
            }
            let _ = write!(b, "## {}\n\n", r.rel);
            last_rel = &r.rel;
        }
        let _ = writeln!(b, "- {} [{}]({})", r.dir, r.label, r.link);
    }
    b
}

/// Go `writeIndexes` (`okf.go:190`): writes an index.md at every directory level for progressive
/// disclosure (OKF section 6): each lists the child concepts and subdirectories at that level
/// with their descriptions.
fn write_indexes(
    abs_out: &Path,
    dirs: &BTreeSet<String>,
    concepts: &[Concept],
) -> Result<(), Error> {
    let mut child_concepts: HashMap<&str, Vec<&Concept>> = HashMap::new();
    for c in concepts {
        child_concepts.entry(&c.dir).or_default().push(c);
    }
    let mut child_dirs: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in dirs {
        if d.is_empty() {
            continue;
        }
        child_dirs.entry(parent_dir(d)).or_default().push(d);
    }

    for d in dirs {
        let mut b = String::new();
        let title = if d.is_empty() { "/" } else { d.as_str() };
        let _ = write!(b, "# {title}\n\n");

        let mut subs = child_dirs.remove(d.as_str()).unwrap_or_default();
        subs.sort();
        for sub in subs {
            let _ = writeln!(b, "- [{}](/{sub}/index.md)", path::base(sub));
        }

        let mut cs: Vec<&Concept> = child_concepts.get(d.as_str()).cloned().unwrap_or_default();
        sort::slice(&mut cs, |a, b| a.file < b.file);
        for c in cs {
            let _ = writeln!(
                b,
                "- [{}]({}) - {}",
                sanitize_label(&c.node.label),
                c.link(),
                concept_description(c.node)
            );
        }

        let full = safe_join(abs_out, d, "index.md")?;
        mkdir_all(full.parent().unwrap_or(Path::new(".")))?;
        write_file(&full, b.as_bytes())?;
    }
    Ok(())
}

/// Go `conceptDescription` (`okf.go:239`).
fn concept_description(n: &JsonNode) -> String {
    match n.community {
        Some(c) => format!("community {c}"),
        None => n.file_type.clone(),
    }
}

/// Go `conceptDir` (`okf.go:248`): maps a source file to a bundle-relative directory by
/// sanitising each path segment with idutil. An empty source file places the node at the root.
pub(super) fn concept_dir(src: &str) -> String {
    let src = trim_space(src);
    if src.is_empty() {
        return String::new();
    }
    let kept: Vec<String> = src
        .split('/')
        .map(|p| make_id(&[p]))
        .filter(|id| !id.is_empty())
        .collect();
    kept.join("/")
}

/// Go `parentDir` (`okf.go:263`).
fn parent_dir(d: &str) -> &str {
    d.rfind('/').map_or("", |i| &d[..i])
}

/// Go `safeJoin` (`okf.go:274`): joins bundle-relative parts under `base` and verifies the
/// result stays inside `base` (defence against path traversal), mirroring the containment check
/// in internal/security.
pub(super) fn safe_join(base: &Path, dir: &str, name: &str) -> Result<PathBuf, Error> {
    let base = base.to_string_lossy();
    let base: &str = &base;
    let full = filepath::join(&[base, dir, name]);
    match filepath::rel(base, &full) {
        Ok(rel) if rel != ".." && !rel.starts_with("../") => Ok(PathBuf::from(full)),
        _ => Err(Error::Other(format!(
            "OKF path {} escapes bundle root {base}",
            quote(filepath::join(&[dir, name]).as_bytes())
        ))),
    }
}

/// Go `yamlString` (`okf.go:285`): quotes a frontmatter scalar so colons, leading markers, and
/// other YAML-significant characters can't break the document.
fn yaml_string(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for c in s.chars() {
        if matches!(c, '\\' | '"') {
            q.push('\\');
        }
        q.push(c);
    }
    q.push('"');
    q
}
