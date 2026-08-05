//! Public package boundary for the runtime-owned llama.cpp adapter.
//!
//! The implementation lives in `oid-runtime` so the console and other clients
//! never depend on a concrete backend. This crate provides the adapter package
//! identity for future plugin loading and re-exports the stable runtime type.

#![warn(missing_docs)]

pub use oid_runtime::LlamaCppAdapter;
