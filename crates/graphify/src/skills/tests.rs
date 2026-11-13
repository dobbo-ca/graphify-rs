use super::GRAPHIFY;

#[test]
fn skill_is_embedded() {
    assert!(GRAPHIFY.starts_with("---\nname: graphify\n"));
    assert_eq!(GRAPHIFY.lines().count(), 56);
    assert!(GRAPHIFY.ends_with('\n'));
}
