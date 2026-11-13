//! KindMask registration at startup (plan hazard "KindMask registration order"). A gositter tree
//! carries mask bits only for kinds named before it was parsed, and bits go in first-registration
//! order, so every kind graphify uses is named once here, before the worker pool starts and in
//! every test that parses, in Go's init order. No `kind_mask` call anywhere else.
//!
//! Each language keeps its names and masks in its own file: `<lang>::register_kinds()` names
//! them in Go's order (package-level vars by file name, then declaration order, then `init()`)
//! and stores the masks in that file's own `OnceLock`. Owner of this file: the
//! extractors-and-CLI stream; a language's function belongs to that language's owner.

use std::sync::Once;

/// Names every kind of every ported language, once per process.
pub fn register_kinds() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        super::c::register_kinds();
        super::golang::register_kinds();
        super::java::register_kinds();
        super::verilog::register_kinds();
        super::zig::register_kinds();
    });
}
