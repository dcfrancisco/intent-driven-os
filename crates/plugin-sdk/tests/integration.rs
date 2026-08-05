#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert!(!oid_plugin_sdk::boundary_name().is_empty());
}
