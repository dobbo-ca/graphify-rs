//! Port of graphify-go `internal/langfamily/langfamily_rake_test.go`.

use crate::langfamily::{cross, of};

/// .rake is plain Ruby: it must join the ruby interop family so a call in a .rake file can bind
/// to a .rb definition, but not to a different-family def (#1784).
#[test]
fn test_rake_is_ruby_family() {
    assert_eq!(of("tasks/build.rake"), "ruby", "of(.rake)");
    assert!(
        !cross("a.rake", "b.rb"),
        ".rake and .rb must be the same (ruby) family — a cross-resolve must not be blocked"
    );
    assert!(
        cross("a.rake", "b.go"),
        ".rake and .go must be different families — a name collision must be blocked"
    );
}
