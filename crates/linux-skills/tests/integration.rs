#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert!(!oid_linux_skills::boundary_name().is_empty());
}
