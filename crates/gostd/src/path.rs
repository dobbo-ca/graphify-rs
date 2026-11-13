//! Go 1.27.4 `path` (lexical, unix): the same code as `path/filepath` on unix. Call sites:
//! graphify-go's ~60 `path` uses (`Ext`, `Dir`, `Base`, `Join`, `Clean`).

pub use crate::filepath::{base, clean, dir, ext, is_abs, join};
