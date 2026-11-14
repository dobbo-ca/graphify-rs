//! graphify-go `internal/extract/component_test.go`.

use crate::extract::{file_from_bytes, resolve};

// A .vue SFC's <script> block must yield its function defs and import edges even
// though the surrounding template/style is not valid JS. The mask preserves line
// numbers, so the function node keeps its true source location, and a template-
// layer dynamic import is recovered by the regex fallback.
#[test]
#[ignore = "graphify-rs-ihc.26"]
fn test_extract_vue_component() {
    crate::extract::kinds::register_kinds();
    let src = r#"<template>
  <component :is="() => import('./LazyWidget.vue')" />
  <div>{{ greet() }}</div>
</template>
<script setup lang="ts">
import { helper } from './helper'
import { ref } from 'vue'

function greet(): string {
  return helper('hi')
}
</script>
<style scoped>
.foo { color: red; }
</style>
"#;
    let rel = "components/Greeter.vue";
    let res = file_from_bytes(rel, src.as_bytes());

    // Function node from the <script> block, at its true (masked-preserving) line.
    let mut greet = false;
    for n in &res.nodes {
        if n.label == "greet()" {
            greet = true;
            assert_eq!(n.source_location, "L9", "greet() source location");
        }
    }
    assert!(
        greet,
        "missing greet() function node; nodes={:?}",
        res.nodes
    );

    // Static import from the <script> block plus the template-layer dynamic import.
    for want in ["./helper", "vue", "./LazyWidget.vue"] {
        assert!(
            res.imps.iter().any(|im| im.spec == want),
            "missing import {want:?}; imps={:?}",
            res.imps
        );
    }

    // Resolve turns the bare 'vue' import into an external concept + imports edge.
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let ext = resolve(&[res], &[rel.to_string()], workers);
    let import_edge = ext
        .edges
        .iter()
        .any(|e| e.relation.as_str() == "imports" && e.source_file.as_str() == rel);
    assert!(import_edge, "no import edge produced for {rel}");
}
