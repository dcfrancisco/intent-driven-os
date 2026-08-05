//! Shared error contracts.

use std::fmt;

/// Errors that can cross the initial runtime boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    /// Configuration failed validation.
    InvalidConfiguration(String),
    /// A lifecycle transition is not valid.
    InvalidLifecycleTransition {
        /// State being exited.
        from: String,
        /// State being entered.
        to: String,
    },
    /// A requested capability is not implemented in Phase 1.
    NotImplemented(&'static str),
    /// A backend identifier is already registered.
    BackendAlreadyRegistered(String),
    /// A backend identifier is unknown.
    BackendNotFound(String),
    /// A backend is registered but disabled.
    BackendNotEnabled(String),
    /// Backend manager state could not be accessed.
    BackendManagerUnavailable,
    /// A model identifier is already registered.
    ModelAlreadyRegistered(String),
    /// A model identifier is unknown.
    ModelNotFound(String),
    /// Persistent model registry storage failed.
    Persistence(String),
    /// The native llama.cpp library is not available.
    NativeBackendUnavailable(String),
    /// A native backend operation failed.
    NativeBackend(String),
    /// A model file is invalid for the requested operation.
    InvalidModel(String),
    /// A model lifecycle operation failed.
    ModelLifecycle(String),
    /// Tokenization could not be completed.
    Tokenization(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid configuration: {message}")
            }
            Self::InvalidLifecycleTransition { from, to } => {
                write!(formatter, "invalid lifecycle transition: {from} -> {to}")
            }
            Self::NotImplemented(feature) => {
                write!(formatter, "not implemented in Phase 1: {feature}")
            }
            Self::BackendAlreadyRegistered(id) => {
                write!(formatter, "backend already registered: {id}")
            }
            Self::BackendNotFound(id) => write!(formatter, "backend not found: {id}"),
            Self::BackendNotEnabled(id) => write!(formatter, "backend not enabled: {id}"),
            Self::BackendManagerUnavailable => formatter.write_str("backend manager unavailable"),
            Self::ModelAlreadyRegistered(id) => write!(formatter, "model already registered: {id}"),
            Self::ModelNotFound(id) => write!(formatter, "model not found: {id}"),
            Self::Persistence(message) => {
                write!(formatter, "model registry persistence error: {message}")
            }
            Self::NativeBackendUnavailable(message) => {
                write!(formatter, "native backend unavailable: {message}")
            }
            Self::NativeBackend(message) => write!(formatter, "native backend error: {message}"),
            Self::InvalidModel(message) => write!(formatter, "invalid model: {message}"),
            Self::ModelLifecycle(message) => write!(formatter, "model lifecycle error: {message}"),
            Self::Tokenization(message) => write!(formatter, "tokenization error: {message}"),
        }
    }
}

impl std::error::Error for RuntimeError {}
