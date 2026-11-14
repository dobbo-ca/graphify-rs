//! Port of `report.go`: renders GRAPH_REPORT.md.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::analyze::{god_nodes, import_cycles, surprising};
use crate::cluster::scores;
use crate::detect::go_lossy;
use crate::model::{Confidence, Edge, Graph};

/// Go `minCommunitySize` (report.go:17).
const MIN_COMMUNITY_SIZE: usize = 3;

/// Go `Generate` (report.go:20): builds the GRAPH_REPORT.md body. A commit is shortened to 8
/// bytes; `""` omits the freshness section. Go runs the analyses on goroutines; they only read
/// the graph, so here they run in turn.
pub fn generate(
    g: &Graph,
    c: &BTreeMap<i64, Vec<String>>,
    root: &str,
    built_at_commit: &str,
) -> String {
    let scores = scores(g, c);
    let gods = god_nodes(g, 10, 0);
    let surprises = surprising(g, c, 5);
    // Real labels per community, read three times below.
    let labels: BTreeMap<i64, Vec<String>> = c
        .iter()
        .map(|(cid, nodes)| (*cid, real_labels(g, nodes)))
        .collect();
    let confidence = confidence_breakdown(g);
    let ambiguous: Vec<&Edge> = g
        .edges()
        .iter()
        .filter(|e| e.confidence == Confidence::AMBIGUOUS)
        .collect();
    let isolated = isolated_nodes(g);
    let cycles = import_cycles(g, 5, 20);

    let mut b = String::new();

    writeln!(b, "# Graph Report - {root}").unwrap();
    writeln!(b).unwrap();
    writeln!(b, "## Summary").unwrap();
    writeln!(
        b,
        "- {} nodes · {} edges · {} communities",
        g.num_nodes(),
        g.num_edges(),
        c.len()
    )
    .unwrap();
    writeln!(b, "- {confidence}").unwrap();
    if g.over_budget > 0 {
        writeln!(
            b,
            "- {} file(s) skipped: parser made no progress (GRAPHIFY_PARSE_BUDGET)",
            g.over_budget
        )
        .unwrap();
    }
    if !built_at_commit.is_empty() {
        writeln!(b).unwrap();
        writeln!(b, "## Graph Freshness").unwrap();
        writeln!(b, "- Built from commit: `{}`", short(built_at_commit)).unwrap();
        writeln!(
            b,
            "- Run `git rev-parse HEAD` and compare to check if the graph is stale."
        )
        .unwrap();
        writeln!(b, "- Run `graphify build .` after code changes to rebuild.").unwrap();
    }

    writeln!(b).unwrap();
    writeln!(b, "## God Nodes (most connected - your core abstractions)").unwrap();
    if gods.is_empty() {
        writeln!(b, "- None detected.").unwrap();
    }
    for (i, n) in gods.iter().enumerate() {
        writeln!(b, "{}. `{}` - {} edges", i + 1, n.label, n.degree).unwrap();
    }

    writeln!(b).unwrap();
    writeln!(
        b,
        "## Surprising Connections (you probably didn't know these)"
    )
    .unwrap();
    if surprises.is_empty() {
        writeln!(
            b,
            "- None detected - all connections are within the same source files."
        )
        .unwrap();
    }
    for s in &surprises {
        let mut note = String::new();
        if !s.note.is_empty() {
            note = format!("  _{}_", s.note);
        }
        writeln!(
            b,
            "- `{}` --{}--> `{}`  [{}]",
            s.source, s.relation, s.target, s.confidence
        )
        .unwrap();
        writeln!(b, "  {} → {}{}", s.source_files[0], s.source_files[1], note).unwrap();
    }

    writeln!(b).unwrap();
    writeln!(b, "## Import Cycles").unwrap();
    if cycles.is_empty() {
        writeln!(b, "- None detected.").unwrap();
    }
    for cy in &cycles {
        let mut path = cy.files.clone();
        path.push(cy.files[0].clone());
        writeln!(
            b,
            "- {}-file cycle: `{}`",
            cy.files.len(),
            path.join(" -> ")
        )
        .unwrap();
    }

    writeln!(b).unwrap();
    writeln!(b, "## Communities ({} total)", c.len()).unwrap();
    for cid in sorted_keys(c) {
        let mut display: &[String] = &labels[&cid];
        if display.len() < MIN_COMMUNITY_SIZE {
            continue;
        }
        let (mut suffix, total) = (String::new(), display.len());
        if display.len() > 8 {
            suffix = format!(" (+{} more)", display.len() - 8);
            display = &display[..8];
        }
        writeln!(b).unwrap();
        writeln!(b, "### Community {cid}").unwrap();
        writeln!(
            b,
            "Cohesion: {:.2}",
            scores.get(&cid).copied().unwrap_or(0.0)
        )
        .unwrap();
        writeln!(b, "Nodes ({}): {}{}", total, display.join(", "), suffix).unwrap();
    }

    if !ambiguous.is_empty() {
        writeln!(b).unwrap();
        writeln!(b, "## Ambiguous Edges - Review These").unwrap();
        for e in &ambiguous {
            writeln!(
                b,
                "- `{}` --{}--> `{}`  [AMBIGUOUS]",
                node_label(g, &e.source),
                e.relation,
                node_label(g, &e.target)
            )
            .unwrap();
            writeln!(b, "  {} · relation: {}", e.source_file, e.relation).unwrap();
        }
    }

    let mut thin = 0;
    for real in labels.values() {
        let n = real.len();
        if n > 0 && n < MIN_COMMUNITY_SIZE {
            thin += 1;
        }
    }
    let amb_pct = ambiguous.len() * 100 / g.num_edges().max(1);
    if isolated.len() + thin > 0 || amb_pct > 20 {
        writeln!(b).unwrap();
        writeln!(b, "## Knowledge Gaps").unwrap();
        if !isolated.is_empty() {
            let mut labels: Vec<String> = isolated
                .iter()
                .map(|id| format!("`{}`", node_label(g, id)))
                .collect();
            let mut suffix = String::new();
            if labels.len() > 5 {
                suffix = format!(" (+{} more)", labels.len() - 5);
                labels.truncate(5);
            }
            writeln!(
                b,
                "- **{} isolated node(s):** {}{}",
                isolated.len(),
                labels.join(", "),
                suffix
            )
            .unwrap();
            writeln!(
                b,
                "  These have <=1 connection - possible missing edges or undocumented components."
            )
            .unwrap();
        }
        if thin > 0 {
            writeln!(
                b,
                "- **{thin} thin communities (<{MIN_COMMUNITY_SIZE} nodes) omitted from report** - run `graphify query` to explore isolated nodes."
            )
            .unwrap();
        }
        if amb_pct > 20 {
            writeln!(
                b,
                "- **High ambiguity: {amb_pct}% of edges are AMBIGUOUS.** Review the Ambiguous Edges section above."
            )
            .unwrap();
        }
    }
    b
}

/// `g.Nodes[id].Label`; Go would panic on a missing node (nil `*Node`), as `unwrap` does.
fn node_label<'a>(g: &'a Graph, id: &str) -> &'a str {
    &g.node(id).unwrap().label
}

/// Go `isolatedNodes` (report.go:169): the IDs of real entities (non-file, non-concept) with at
/// most one connection - candidates for missing edges or undocumented components.
fn isolated_nodes(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    for id in g.node_ids() {
        let n = g.node(id).unwrap();
        if g.degree(id) > 1 {
            continue;
        }
        if n.source_file.is_empty() || n.label == base(&n.source_file) {
            // concept node or file hub
            continue;
        }
        if !base(&n.source_file).contains('.') {
            // concept node (no real source file)
            continue;
        }
        out.push(id.clone());
    }
    out
}

/// Go `confidenceBreakdown` (report.go:187).
fn confidence_breakdown(g: &Graph) -> String {
    let mut counts: BTreeMap<Confidence, usize> = BTreeMap::new();
    let mut total = 0;
    for e in g.edges() {
        *counts.entry(e.confidence).or_insert(0) += 1;
        total += 1;
    }
    if total == 0 {
        total = 1;
    }
    let pct = |k: Confidence| counts.get(&k).copied().unwrap_or(0) * 100 / total;
    format!(
        "Extraction: {}% EXTRACTED · {}% INFERRED · {}% AMBIGUOUS",
        pct(Confidence::EXTRACTED),
        pct(Confidence::INFERRED),
        pct(Confidence::AMBIGUOUS)
    )
}

/// Go `realLabels` (report.go:204): the labels of nodes that are real entities (not file hubs
/// or method stubs), for display in a community listing.
fn real_labels(g: &Graph, nodes: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for id in nodes {
        let n = g.node(id).unwrap();
        if !n.source_file.is_empty() && n.label == base(&n.source_file) {
            continue; // file-hub node
        }
        out.push(n.label.clone());
    }
    out
}

/// Go `sortedKeys` (report.go:216). The map is already ordered by key.
fn sorted_keys(m: &BTreeMap<i64, Vec<String>>) -> Vec<i64> {
    m.keys().copied().collect()
}

/// Go `base` (report.go:225).
fn base(p: &str) -> &str {
    match p.rfind('/') {
        Some(i) => &p[i + 1..],
        None => p,
    }
}

/// Go `short` (report.go:232): the first 8 bytes; Go can cut inside a rune, which prints as
/// U+FFFD (plan R3).
fn short(s: &str) -> String {
    if s.len() > 8 {
        go_lossy(&s.as_bytes()[..8])
    } else {
        s.to_string()
    }
}
