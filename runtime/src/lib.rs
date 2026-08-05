//! Intelligent Runtime foundation.
//!
//! Phase 1 defines the control-plane boundaries and startup lifecycle. It does
//! not implement inference, backend integration, Linux operations, or hardware
//! probing.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod api;
pub mod backends;
pub mod config;
pub mod core;
pub mod hardware;
pub mod lifecycle;
pub mod logging;
pub mod mock;
pub mod models;
pub mod native_llama;
pub mod security;

pub use api::{RuntimeApi, RuntimeService, RuntimeSnapshot};
pub use backends::{
    Backend, BackendDescriptor, BackendHealth, BackendManager, BackendSummary, LoadedModel,
    MockBackend,
};
pub use core::Runtime;
pub use hardware::{HardwareService, HardwareSnapshot};
pub use mock::MockRuntime;
pub use models::{ModelMetadata, ModelRegistry, ModelStatus};
pub use native_llama::LlamaCppAdapter;
pub use oid_shared::{LifecycleState, RuntimeConfig, RuntimeError, RuntimeEvent, RuntimeStatus};
