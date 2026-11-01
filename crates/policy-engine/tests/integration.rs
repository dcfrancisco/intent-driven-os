#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert_ne!(oid_policy_engine::boundary_name(), "");
}
