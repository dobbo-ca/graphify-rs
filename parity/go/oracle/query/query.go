// Package query holds the graph.json types of graphify-go internal/query/query.go:22-97 for the
// json oracle. They live in a package named query so that Go's error texts name them as
// graphify-go does (`query.Graph`, `[]query.Node`).
package query

// Graph is graph.json as query.Load and Validate read it (Unmarshal at query.go:113 and
// validate.go:27, error "parsing %s: %w"). The unexported fields (byID, adj, edge) are left out:
// encoding/json ignores them.
type Graph struct {
	Nodes []Node     `json:"nodes"`
	Links []Link     `json:"links"`
	Attrs GraphAttrs `json:"graph"`

	BuiltAtCommit string `json:"built_at_commit,omitempty"`
	Path          string `json:"-"` // file Load read, for git lookups
}

// GraphAttrs is query.go:38-41.
type GraphAttrs struct {
	UnclassifiedFiles int            `json:"unclassified_files"`
	UnclassifiedExts  map[string]int `json:"unclassified_extensions"`
}

// Node is query.go:77-87.
type Node struct {
	ID             string            `json:"id"`
	Label          string            `json:"label"`
	FileType       string            `json:"file_type"`
	SourceFile     string            `json:"source_file"`
	SourceLocation string            `json:"source_location"`
	Community      *int              `json:"community"`
	NormLabel      string            `json:"norm_label"`
	ComputedName   string            `json:"computed_name"`
	Attributes     map[string]string `json:"attributes"`
}

// Link is query.go:90-97.
type Link struct {
	Source         string `json:"source"`
	Target         string `json:"target"`
	Relation       string `json:"relation"`
	Confidence     string `json:"confidence"`
	SourceFile     string `json:"source_file"`
	SourceLocation string `json:"source_location"`
}
