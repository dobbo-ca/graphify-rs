//! bedrock-body (graphify-rs-1kw): bedrock's request body for a note path and body, to compare with
//! anthropic-sdk-go v1.51.0's MessageNewParams.MarshalJSON in parity/go/oracle/bedrock_body.go.

use super::Op;

/// The input is the note path, a NUL byte, then the note body.
pub fn run(_name: &str, input: &[u8]) -> Vec<Op> {
    let (file, content) = match input.iter().position(|&b| b == 0) {
        Some(i) => (&input[..i], &input[i + 1..]),
        None => (input, &[][..]),
    };
    let file = std::str::from_utf8(file).expect("the Go generator writes UTF-8 note paths");
    vec![(
        "body",
        Ok(bedrock::request_body(file, content).into_bytes()),
    )]
}
