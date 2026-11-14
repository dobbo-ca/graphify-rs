//! graphify v0.9.107 `tests/test_json_schema_config.py` (14 tests), the 13 whose assertion holds
//! for graphify-go v0.12.0. Python's `extract_json` returns `skipped` for a file it does not walk;
//! Go's `extractJSON` returns an empty `Result` (no nodes, no edges), which is `is_skipped` here.
//! The file is not walked when `isConfigJSON` (`json.go:176`) is false: a basename in
//! `configJSONNames`, a suffix, or a root key of `configJSONKeys`, unless the root has `$schema`
//! with `$defs`, `definitions` or `$id`.
//!
//! Not ported (1):
//! - `test_root_level_ref_alone_is_skipped`: upstream removed `$ref` from the root-key probe; Go's
//!   `configJSONKeys` still holds `$ref` (and `$schema`), so `{"$ref": ...}` is walked and gives
//!   nodes. The assertion is false for Go.
//!
//! The 13 ported cases are ignored on ihc.16: the JSON extractor (`json.go`) and the parse it
//! makes through `ts::parse` (ihc.15).

use crate::extract::Result;
use crate::extract::tests::util::extract_bytes;

/// A JSON Schema as a contract file actually looks like: `$id` + `$defs`.
const SCHEMA_WITH_DEFS: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.com/events-v1.json",
  "title": "Event catalogue",
  "$defs": {
    "dataClassification": {
      "description": "Handling class for the event payload.",
      "type": "string",
      "enum": ["Public", "Internal", "CUI"]
    },
    "event": {
      "description": "One domain event.",
      "type": "object",
      "required": ["id", "kind"],
      "properties": {
        "id": {"description": "Stable id.", "type": "string"},
        "kind": {"description": "Event kind.", "type": "string"},
        "seq": {"description": "Sequence.", "type": "integer", "minimum": 0},
        "tags": {"description": "Tags.", "type": "array",
                 "items": {"type": "string"}},
        "shape": {"description": "One shape.", "oneOf": [{"type": "string"}]}
      },
      "additionalProperties": false
    }
  },
  "$ref": "#/$defs/event"
}
"##;

const SCHEMA_WITH_LEGACY_DEFS: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "point": {"description": "A point.", "type": "object"}
  },
  "$ref": "#/definitions/point"
}
"##;

/// Real configs that carry `$schema` themselves. These point at their OWN tool's schema, not at
/// the JSON Schema meta-schema.
const BIOME: &str = r#"{
  "$schema": "https://biomejs.dev/schemas/1.7.0/schema.json",
  "files": {"include": ["src"]},
  "linter": {"enabled": true}
}
"#;

const RENOVATE: &str = r#"{
  "$schema": "https://docs.renovatebot.com/renovate-schema.json",
  "extends": ["config:base"]
}
"#;

/// Arbitrary filename -> only the root-key probe can catch these.
const PROBE_EXTENDS: &str = r#"{"extends": "./base.json", "rules": {"eqeqeq": "error"}}"#;
const PROBE_COMPILER: &str = r#"{"compilerOptions": {"strict": true, "target": "es2022"}}"#;
const SUFFIX_TSCONFIG: &str = r#"{"compilerOptions": {"strict": true}}"#;

fn extract_json(rel: &str, body: &str) -> Result {
    extract_bytes(rel, body.as_bytes())
}

fn labels(r: &Result) -> Vec<&str> {
    r.nodes.iter().map(|n| n.label.as_str()).collect()
}

fn is_skipped(r: &Result) -> bool {
    r.nodes.is_empty() && r.edges.is_empty()
}

// --- the schema side: these must stop being walked -------------------------

#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_schema_with_defs_is_skipped_not_walked() {
    let r = extract_json("events-v1.json", SCHEMA_WITH_DEFS);
    assert!(is_skipped(&r), "{r:?}");
}

/// draft-07 schemas spell the same thing `definitions`.
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_schema_with_legacy_definitions_is_skipped() {
    let r = extract_json("legacy.json", SCHEMA_WITH_LEGACY_DEFS);
    assert!(is_skipped(&r), "{r:?}");
}

/// The reporter's file lived at contracts/events-v1.json, i.e. the filename branch could never
/// have matched it -- only the probe could.
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_schema_with_defs_in_subdirectory_is_skipped() {
    let r = extract_json("contracts/events-v1.json", SCHEMA_WITH_DEFS);
    assert!(is_skipped(&r), "{r:?}");
}

/// The actual symptom: bare schema keywords became graph nodes.
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_schema_produces_no_keyword_nodes() {
    let r = extract_json("events-v1.json", SCHEMA_WITH_DEFS);
    let got = labels(&r);
    for keyword in [
        "description",
        "type",
        "properties",
        "required",
        "additionalProperties",
        "oneOf",
        "items",
        "enum",
    ] {
        assert!(!got.contains(&keyword), "{keyword:?} in {got:?}");
    }
}

// --- the config side: the fix must not over-correct ------------------------

/// biome.json points `$schema` at biome's schema, not at the meta-schema. It is filename-matched,
/// so it never depended on the probe.
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_biome_json_with_its_own_schema_key_is_still_extracted() {
    let r = extract_json("biome.json", BIOME);
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"linter"), "{:?}", labels(&r));
}

#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_renovate_json_with_schema_and_extends_is_still_extracted() {
    let r = extract_json("renovate.json", RENOVATE);
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"extends"), "{:?}", labels(&r));
}

#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_package_json_is_still_extracted() {
    let r = extract_json(
        "package.json",
        r#"{"name": "x", "devDependencies": {"jest": "^29"}}"#,
    );
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"devDependencies"), "{:?}", labels(&r));
}

/// `extends` stays in the probe.
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_probe_still_recognises_extends_by_arbitrary_filename() {
    let r = extract_json("custom.config.json", PROBE_EXTENDS);
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"extends"), "{:?}", labels(&r));
}

#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_probe_still_recognises_compileroptions_by_arbitrary_filename() {
    let r = extract_json("weird.json", PROBE_COMPILER);
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"compilerOptions"), "{:?}", labels(&r));
}

#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_compound_suffix_match_is_unaffected() {
    let r = extract_json("api.tsconfig.json", SUFFIX_TSCONFIG);
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"compilerOptions"), "{:?}", labels(&r));
}

#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_dev_dependencies_probe_still_works() {
    let r = extract_json("deps.json", PROBE_COMPILER);
    assert!(!is_skipped(&r), "{r:?}");
}

// --- documented boundary, pinned so a future change is deliberate ----------

/// A hand-written schema carrying only `$schema` + `properties` is still walked: it has none of
/// `$defs`, `$id`, `definitions`, and `$schema` is in Go's `configJSONKeys`.
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_minimal_schema_without_defs_or_id_is_still_walked() {
    let body = r#"{"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object", "properties": {"a": {"type": "string"}}}"#;
    let r = extract_json("minimal.schema.json", body);
    assert!(!is_skipped(&r), "{r:?}");
    assert!(labels(&r).contains(&"properties"), "{:?}", labels(&r));
}

// --- determinism -----------------------------------------------------------

/// Extraction must not depend on ordering or ambient state (CONTRIBUTING #3).
#[test]
#[ignore = "graphify-rs-ihc.16"]
fn test_classification_is_deterministic_across_repeated_calls() {
    let first: Vec<String> = labels(&extract_json("events-v1.json", SCHEMA_WITH_DEFS))
        .into_iter()
        .map(String::from)
        .collect();
    for _ in 0..5 {
        let again: Vec<String> = labels(&extract_json("events-v1.json", SCHEMA_WITH_DEFS))
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(again, first);
    }
}
