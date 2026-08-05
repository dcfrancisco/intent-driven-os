//! Shared contracts used by the Intelligent Runtime and AI Console.
//!
//! This crate contains dependency-light value types and events only. It does not
//! contain inference, operating-system operations, or presentation logic.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod errors;
pub mod events;
pub mod types;

pub use config::RuntimeConfig;
pub use errors::RuntimeError;
pub use events::{EventBus, EventReceiver, RuntimeEvent};
pub use types::{LifecycleState, RuntimeStatus};
