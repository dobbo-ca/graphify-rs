//! Port of `manifest.go`: the package-manifest pass (`pyproject.toml`, `Cargo.toml`, `go.mod`,
//! `pom.xml`). Owner: the extractors-and-CLI stream; filled by graphify-rs-ihc.17.

use std::path::Path;

use super::extract::Result;

/// Go `extract.IntrospectManifests`: one `package` node per manifest (keyed by package name), a
/// stub `package` node per external dependency, and `depends_on` edges. `files` are the manifests
/// under `root` as `detect::collect_manifests` lists them (Go calls it here; the caller passes
/// the list, so this pass has no error). A malformed or nameless manifest is skipped.
// graphify-rs-ihc.17: stubs until the code PR
#[allow(dead_code, unused_variables)]
pub fn introspect_manifests(root: &Path, files: &[String]) -> Result {
    todo!("graphify-rs-ihc.17")
}

#[cfg(test)]
#[path = "tests/manifest.rs"]
mod manifest_tests;

#[cfg(test)]
#[path = "tests/upstream_manifest_ingest.rs"]
mod upstream_manifest_ingest_tests;
