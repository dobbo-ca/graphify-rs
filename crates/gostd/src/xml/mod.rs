//! Go 1.27.4 `encoding/xml`, the subset graphify-go uses: `xml.Unmarshal` of
//! pom.xml into the `xmlElem` shape (`internal/extract/manifest.go:315`) and
//! `xml.MarshalIndent` of the GraphML tree (`internal/export/formats.go:95-110`).
#![allow(unused_variables)]
#![allow(clippy::ptr_arg)]

mod marshal;
mod read;

pub use marshal::{HEADER, Node, escape_attr, escape_text, marshal_indent};
pub use read::{
    Attr, Element, Name, SyntaxError, Token, input_pos_after_tokens, is_in_character_range,
    proc_inst, raw_token_spans, raw_tokens, tokens, tokens_partial, tokens_with_entity,
    unmarshal_element,
};
