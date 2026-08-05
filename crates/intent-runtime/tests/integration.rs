#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert!(!oid_intent_runtime::boundary_name().is_empty());
}
