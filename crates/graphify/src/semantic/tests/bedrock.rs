//! Port of `bedrock_test.go` (4 tests). The Bedrock backend turns the model's structured tool
//! output into graph fragments. `parse_concepts` is the pure core of that translation and is
//! tested here without any network call.

use crate::semantic::{CONCEPT_TYPE, Note};

use crate::semantic::bedrock::parse_concepts;

#[test]
fn test_parse_concepts_builds_nodes_and_edges() {
    let raw = r#"{
		"concepts": [
			{"name": "AppConfig", "relation": "conceptually_related_to", "score": 0.9},
			{"name": "deployment bake time", "relation": "cites", "score": 0.7}
		]
	}"#;
    let note = Note {
        id: "notes_appconfig_bake".to_string(),
        file: "learnings/appconfig-bake.md".to_string(),
        ..Note::default()
    };
    let (nodes, edges) =
        parse_concepts(&note, raw.as_bytes()).unwrap_or_else(|e| panic!("parseConcepts: {e}"));
    assert_eq!(nodes.len(), 2, "want 2 concept nodes, got {}", nodes.len());
    assert_eq!(edges.len(), 2, "want 2 edges, got {}", edges.len());
    // Concept ids are stable (idutil), edges originate from the note, and the
    // score rides ConfidenceScore.
    assert_eq!(
        nodes[0].file_type, CONCEPT_TYPE,
        "node 0 file_type = {:?}, want {:?}",
        nodes[0].file_type, CONCEPT_TYPE
    );
    for e in &edges {
        assert_eq!(
            e.source, note.id,
            "edge source = {:?}, want note id {:?}",
            e.source, note.id
        );
        assert!(e.confidence_score != 0.0, "edge {e:?} lost its score");
    }
    // An edge must point at the concept node id that parseConcepts minted.
    let want = &nodes[0].id;
    let found = edges.iter().any(|e| &e.target == want);
    assert!(
        found,
        "no edge targets the AppConfig concept node id {want:?}"
    );
}

#[test]
fn test_parse_concepts_drops_unknown_relations() {
    let raw = r#"{"concepts": [{"name": "X", "relation": "is_a", "score": 1.0}]}"#;
    let note = Note {
        id: "n".to_string(),
        ..Note::default()
    };
    let (_, edges) =
        parse_concepts(&note, raw.as_bytes()).unwrap_or_else(|e| panic!("parseConcepts: {e}"));
    assert!(
        edges.is_empty(),
        "unknown relation should yield no edge, got {}",
        edges.len()
    );
}

#[test]
fn test_parse_concepts_tolerates_garbage() {
    let note = Note {
        id: "n".to_string(),
        ..Note::default()
    };
    assert!(
        parse_concepts(&note, b"not json").is_err(),
        "malformed tool output should error so the note is skipped"
    );
}

#[test]
fn test_parse_concepts_marks_low_score_ambiguous() {
    let raw =
        r#"{"concepts": [{"name": "Maybe", "relation": "semantically_similar_to", "score": 0.2}]}"#;
    let note = Note {
        id: "n".to_string(),
        ..Note::default()
    };
    let (_, edges) =
        parse_concepts(&note, raw.as_bytes()).unwrap_or_else(|e| panic!("parseConcepts: {e}"));
    assert!(
        edges.len() == 1 && edges[0].confidence.to_string() == "AMBIGUOUS",
        "low-score edge should be AMBIGUOUS, got {edges:?}"
    );
}
