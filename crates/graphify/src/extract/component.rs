//! Port of `component.go`.
//!
//! Single-file components (.vue/.svelte/.astro) wrap a JS/TS script region in markup a JS
//! grammar cannot parse. Feeding the whole file to the parser yields a top-level ERROR node,
//! so defs and imports are silently dropped. Instead we mask every non-script region with
//! blanks (newlines preserved so line numbers stay accurate) and run the existing JS/TS
//! extractor over the masked source, then a regex pass recovers dynamic `import('…')` calls
//! the AST does not edge (including template-layer imports the mask blanked out). Mirrors
//! upstream extract_vue/extract_svelte/extract_astro.

use std::sync::LazyLock;

use gositter::Language;
use gostd::regexp::Regexp;
use gostd::strings::to_lower;

use super::builder::go_text;
use super::extract::{DYN_IMPORT, Imp, Result};
use super::javascript::extract_js;
use crate::idutil::make_id;
use crate::model::line as model_line;

/// Go `componentScriptRE`: matches a <script> block, capturing its open tag, body, and close
/// tag. The open-tag matcher skips over quoted attribute values so a `>` inside one (e.g.
/// `<script setup lang="ts" generic="T extends A<B>">`) does not prematurely end the tag.
static COMPONENT_SCRIPT_RE: LazyLock<Regexp> = LazyLock::new(|| {
    Regexp::must_compile(r#"(?i)(<script\b(?:"[^"]*"|'[^']*'|[^>"'])*>)([\s\S]*?)(</script\s*>)"#)
});

/// Go `componentScriptLangRE`: reads the declared `lang` from a script open tag.
static COMPONENT_SCRIPT_LANG_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r#"(?i)\blang\s*=\s*['"]?([A-Za-z]+)['"]?"#));

/// Go `astroFrontmatterRE`: matches the leading `---\n…\n---` TypeScript frontmatter fence of
/// an .astro file, capturing the code between the fences.
static ASTRO_FRONTMATTER_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r"\A\s*---\s*\r?\n([\s\S]*?)\r?\n---\s*(?:\r?\n|\z)"));

/// Go `dynamicImportRE`: matches a dynamic `import('…')` specifier.
static DYNAMIC_IMPORT_RE: LazyLock<Regexp> =
    LazyLock::new(|| Regexp::must_compile(r#"import\(\s*['"]([^'"]+)['"]\s*\)"#));

/// Go `extractComponent`: extracts a single-file component. `is_astro` also keeps the leading
/// `---…---` frontmatter fence as a script region. `None` when the parse budget stops the parse
/// (see `builder`).
pub(crate) fn extract_component(rel: &str, src: &[u8], is_astro: bool) -> Option<Result> {
    let (masked, lang) = mask_component_script(src, is_astro);
    let mut res = extract_js(rel, &masked, component_lang_ptr(&lang))?;

    // Regex fallback for dynamic imports the AST never reaches, whether they live
    // in the script body or the template layer the mask blanked out.
    let file_id = make_id(&[rel]);
    for m in DYNAMIC_IMPORT_RE.find_all_submatch_index(src, -1) {
        let (Some((at, _)), Some((s0, s1))) = (m[0], m[1]) else {
            continue;
        };
        let spec = go_text(&src[s0..s1]).into_owned();
        // upstream drops an empty package name
        if spec.starts_with('/') {
            continue;
        }
        if spec.is_empty() {
            continue;
        }
        res.imps.push(Imp {
            file_id: file_id.clone(),
            file: rel.to_string(),
            spec,
            loc: model_line(line_of(src, at) as i64),
            dynamic: DYN_IMPORT,
            ..Imp::default()
        });
    }
    Some(res)
}

/// Go `maskComponentScript`: blanks everything outside script regions, replacing each
/// non-newline byte with a space so the JS/TS grammar sees only the script while preserved \r/\n
/// keep line numbers accurate. It returns the masked source and the first script block's
/// declared lang (lowercased, "" when unset). For .astro the leading frontmatter fence is also
/// kept.
fn mask_component_script(src: &[u8], is_astro: bool) -> (Vec<u8>, String) {
    let mut masked: Vec<u8> = src
        .iter()
        .map(|&c| if c == b'\n' || c == b'\r' { c } else { b' ' })
        .collect();
    if is_astro
        && let Some(m) = ASTRO_FRONTMATTER_RE.find_submatch_index(src)
        && let Some((a, b)) = m[1]
    {
        masked[a..b].copy_from_slice(&src[a..b]);
    }
    let mut lang = String::new();
    for m in COMPONENT_SCRIPT_RE.find_all_submatch_index(src, -1) {
        let (Some((o0, o1)), Some((b0, b1))) = (m[1], m[2]) else {
            continue;
        };
        // script body, verbatim
        masked[b0..b1].copy_from_slice(&src[b0..b1]);
        if lang.is_empty()
            && let Some(lm) = COMPONENT_SCRIPT_LANG_RE.find_submatch(&src[o0..o1])
            && let Some(l) = lm[1]
        {
            lang = to_lower(&go_text(l));
        }
    }
    (masked, lang)
}

/// Go `componentLangPtr`: picks the grammar for a script block's declared lang: TS for `ts`,
/// TSX for `tsx`, else JS (js/jsx/unset). TS is a superset of JS, so JS is a safe default for a
/// script that declares no lang.
fn component_lang_ptr(lang: &str) -> &'static Language {
    match lang {
        "ts" => gositter_grammars::typescript::language(),
        "tsx" => gositter_grammars::tsx::language(),
        _ => gositter_grammars::javascript::language(),
    }
}

/// Go `lineOf`: the 1-based line number of byte offset `off` in `src`.
fn line_of(src: &[u8], off: usize) -> usize {
    1 + src[..off.min(src.len())]
        .iter()
        .filter(|&&c| c == b'\n')
        .count()
}

#[cfg(test)]
#[path = "tests/component.rs"]
mod component_tests;
