#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert_ne!(oid_verification_engine::boundary_name(), "");
}
