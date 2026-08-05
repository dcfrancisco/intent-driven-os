#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert!(!oid_model_runner::boundary_name().is_empty());
}
