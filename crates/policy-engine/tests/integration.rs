#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert!(!oid_policy_engine::boundary_name().is_empty());
}
