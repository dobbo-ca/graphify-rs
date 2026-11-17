//! Port of `cmd/graphify/semantic.go`: the opt-in `--semantic` stage of `build`. Filled by
//! graphify-rs-ihc.20.

/// Go `semanticOpts`: the enrichment-stage settings derived from the build flags.
// graphify-rs-ihc.20: stubs until the code PR
#[allow(dead_code, unused_variables)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SemanticOpts {
    pub enabled: bool,
    pub backend: String,
}
