//! Port of `oldnodes.go`: the nodes of the graph.json on disk, for the shrink guard.

use std::io::Read;

/// Go `oldNode` (`oldnodes.go:13`): an alias of an unnamed struct, so a decode error prints the
/// struct type, not a name.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct OldNode {
    pub(super) id: String,
    pub(super) source_file: String,
}

gostd::json_fields!(decode OldNode, "struct { ID string \"json:\\\"id\\\"\"; SourceFile string \"json:\\\"source_file\\\"\" }", {
    id => "id",
    source_file => "source_file",
});

/// Go `scanOldNodes` (`oldnodes.go:47`): `None` is Go's `ok == false`.
pub(super) fn scan_old_nodes(r: impl Read) -> Option<Vec<OldNode>> {
    todo!("graphify-rs-ihc.12")
}
