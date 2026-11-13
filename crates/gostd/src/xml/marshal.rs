//! Writer side: `xml.MarshalIndent` of a GraphML-shaped tree and escaping.

/// Go `xml.Header`.
pub const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

/// One element: string `,attr` fields, `,chardata`, child elements, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub chardata: String,
    pub children: Vec<Node>,
}

/// Go `xml.MarshalIndent(n, prefix, indent)`.
pub fn marshal_indent(n: &Node, prefix: &str, indent: &str) -> Vec<u8> {
    todo!("graphify-rs-aa6.12")
}

/// Go `xml.EscapeString` (appends to `dst`).
pub fn escape_attr(dst: &mut Vec<u8>, s: &[u8]) {
    todo!("graphify-rs-aa6.12")
}

/// Go `escapeText(w, s, false)` (appends to `dst`; newline kept raw).
pub fn escape_text(dst: &mut Vec<u8>, s: &[u8]) {
    todo!("graphify-rs-aa6.12")
}
