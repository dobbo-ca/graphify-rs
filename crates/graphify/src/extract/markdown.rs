//! Port of `markdown.go`: the regexp-based Markdown extractor. Owner: the extractors-and-CLI
//! stream; filled by graphify-rs-ihc.17.

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use gostd::regexp::Regexp;
use gostd::strings::trim_space;

use super::builder::{go_text, itoa};
use super::extract::{MdRef, Result};
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Loc, Node, Relation, file_of, line};

/// Go `mdLink`: an inline link `[text](target)`; the target is `<...>` or runs to the first
/// space or `)`.
static MD_LINK: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"\[[^\]]*\]\((?:<([^>]+)>|([^)\s]+))"));

/// Go `mdRefDef`: a reference definition `[label]: target` at the start of a line.
static MD_REF_DEF: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?"));

/// Go `mdWikilink`: a `[[target]]` wikilink, without its `#section` or `|alias`.
static MD_WIKILINK: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"\[\[([^\]|#\\]+)\\?(?:[#|][^\]]*)?\]\]"));

/// Go `mdHeading`: an ATX heading line.
static MD_HEADING: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile(r"^(#{1,6})\s+(.+)"));

/// Go `mdCodeSpan`: an inline `code` span.
static MD_CODE_SPAN: LazyLock<Regexp> = LazyLock::new(|| Regexp::must_compile("`([^`\n]+)`"));

/// Go `extractMarkdown` (no parse, so no parse budget).
pub(crate) fn extract_markdown(rel: &str, src: &[u8]) -> Result {
    let rel_f = file_of(rel);
    let ext = gostd::filepath::ext(rel);
    let concept_id = make_id(&[rel.strip_suffix(ext).unwrap_or(rel)]);

    let s = go_text(src);
    let s: &str = &s;
    let (fm, body) = split_frontmatter(s);
    let file_type = match fm.get("type").map(String::as_str) {
        None | Some("") => "document".to_string(),
        Some(t) => t.to_string(),
    };
    let label = match fm.get("title").map(String::as_str) {
        None | Some("") => gostd::filepath::base(rel),
        Some(t) => t.to_string(),
    };

    let mut res = Result::default();
    res.nodes.push(Node {
        id: concept_id.clone(),
        label,
        file_type,
        source_file: rel.to_string(),
        source_location: "L1".to_string(),
        computed_name: computed_meta(&fm),
        ..Node::default()
    });

    // body is an exact suffix of s, so the count of newlines dropped with the frontmatter is the
    // offset that keeps heading/reference lines file-absolute.
    let offset = s[..s.len() - body.len()].matches('\n').count();

    let mut seen: HashSet<String> = HashSet::from([concept_id.clone()]);
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut in_code = false;

    for (i, line_text) in body.split('\n').enumerate() {
        let n = (offset + i + 1) as i64;
        let loc = line(n);
        if trim_space(line_text).starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }

        // References anywhere in the doc (scanned on heading lines too). Inline `code` spans are
        // candidate references to a bare symbol name.
        let refs = &mut res.md_refs;
        for m in MD_LINK.find_all_string_submatch(line_text, -1) {
            let target = format!("{}{}", m[1].unwrap_or(""), m[2].unwrap_or(""));
            add_ref(refs, &concept_id, rel, &target, loc);
        }
        for m in MD_WIKILINK.find_all_string_submatch(line_text, -1) {
            add_ref(refs, &concept_id, rel, m[1].unwrap_or(""), loc);
        }
        if let Some(m) = MD_REF_DEF.find_string_submatch(line_text) {
            add_ref(refs, &concept_id, rel, m[1].unwrap_or(""), loc);
        }
        for m in MD_CODE_SPAN.find_all_string_submatch(line_text, -1) {
            let sym = trim_space(m[1].unwrap_or(""));
            if !sym.is_empty() && !sym.contains([' ', '\t']) {
                add_ref(refs, &concept_id, rel, sym, loc);
            }
        }

        let Some(m) = MD_HEADING.find_string_submatch(line_text) else {
            continue;
        };
        let level = m[1].unwrap_or("").len();
        let title = trim_space(m[2].unwrap_or(""));
        let mut hid = make_id(&[&concept_id, title]);
        if seen.contains(&hid) {
            // disambiguate a repeated heading title by its line
            hid = make_id(&[&concept_id, title, &itoa(n)]);
        }
        if seen.insert(hid.clone()) {
            res.nodes.push(Node {
                id: hid.clone(),
                label: title.to_string(),
                file_type: "heading".to_string(),
                source_file: rel.to_string(),
                source_location: loc.to_string(),
                ..Node::default()
            });
        }
        // Pop headings at the same or a deeper level, then hang this heading off its nearest
        // shallower ancestor (or the concept node for a top heading).
        while stack.last().is_some_and(|(l, _)| *l >= level) {
            stack.pop();
        }
        let parent = stack.last().map_or(&concept_id, |(_, id)| id).clone();
        res.edges.push(Edge {
            source: parent,
            target: hid.clone(),
            relation: Relation::CONTAINS,
            confidence: Confidence::EXTRACTED,
            source_file: rel_f,
            source_location: loc,
            ..Edge::default()
        });
        stack.push((level, hid));
    }
    res
}

/// Go `addRef`: records `target`, trimmed, as an MDRef of the concept node unless it is empty.
fn add_ref(refs: &mut Vec<MdRef>, from_id: &str, rel: &str, target: &str, loc: Loc) {
    let target = trim_space(target);
    if !target.is_empty() {
        refs.push(MdRef {
            from_id: from_id.to_string(),
            target: target.to_string(),
            file: rel.to_string(),
            loc,
        });
    }
}

/// Go `splitFrontmatter`: the flat keys of a leading `---` YAML block and the body after it (an
/// exact suffix of `src`). With no block, or an unterminated one, no keys and all of `src`.
pub(crate) fn split_frontmatter(src: &str) -> (BTreeMap<String, String>, &str) {
    let mut fm = BTreeMap::new();
    let mut lines = src.split('\n');
    let first = lines.next().unwrap_or("");
    if first.trim_end_matches('\r') != "---" {
        return (fm, src);
    }
    // `end` is the offset of the '\n' after the current line (Go joins lines[i+1:] with "\n",
    // which is the suffix of src after that newline).
    let mut end = first.len();
    for l in lines {
        let start = end + 1;
        end = start + l.len();
        let l = l.trim_end_matches('\r');
        if l == "---" {
            return (fm, src.get(end + 1..).unwrap_or(""));
        }
        let Some((key, val)) = l.split_once(':') else {
            continue;
        };
        fm.insert(
            trim_space(key).to_string(),
            unquote_fm(trim_space(val)).to_string(),
        );
    }
    // Unterminated frontmatter: treat the whole source as body.
    (BTreeMap::new(), src)
}

/// Go `computedMeta`: the description and the tags (without `[]`), space-joined.
fn computed_meta(fm: &BTreeMap<String, String>) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(2);
    if let Some(d) = fm.get("description").filter(|d| !d.is_empty()) {
        parts.push(d);
    }
    if let Some(t) = fm.get("tags").filter(|t| !t.is_empty()) {
        parts.push(t.trim_matches(['[', ']']));
    }
    parts.join(" ")
}

/// Go `unquoteFM`: strips matching surrounding single or double quotes.
fn unquote_fm(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && b[0] == b[b.len() - 1] && (b[0] == b'"' || b[0] == b'\'') {
        return &s[1..s.len() - 1];
    }
    s
}

#[cfg(test)]
#[path = "tests/markdown.rs"]
mod markdown_tests;

#[cfg(test)]
#[path = "tests/upstream_markdown_code_spans.rs"]
mod upstream_markdown_code_spans_tests;
