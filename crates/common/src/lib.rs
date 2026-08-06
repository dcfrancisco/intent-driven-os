//! Shared contracts and infrastructure primitives for Open Intelligence Desktop.
//!
//! This crate is intentionally dependency-light. Cross-cutting models should live here
//! only when they are genuinely shared by multiple bounded contexts.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;

macro_rules! identifier {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Construct an identifier from a non-empty value.
            ///
            /// # Errors
            ///
            /// Returns [`OidError::InvalidInput`] when the value is empty.
            pub fn new(value: impl Into<String>) -> Result<Self, OidError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(OidError::InvalidInput(stringify!($name).to_owned()));
                }
                Ok(Self(value))
            }

            /// Return the identifier as text.
            #[must_use]
            pub fn as_str(&self) -> &str { &self.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier!(/// Identifier for an intent request.
    IntentId);
identifier!(/// Identifier for an executable operation.
    OperationId);
identifier!(/// Identifier for a registered skill.
    SkillId);
identifier!(/// Identifier for a plugin.
    PluginId);
identifier!(/// Identifier for an evidence record.
    EvidenceId);

/// Errors shared across OID bounded contexts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OidError {
    /// A required input was empty or malformed.
    InvalidInput(String),
    /// A state transition is not allowed.
    InvalidTransition {
        /// State being exited.
        from: String,
        /// State being entered.
        to: String,
    },
    /// An operation was denied by policy.
    Unauthorized(String),
    /// An explicit approval is required before proceeding.
    ApprovalRequired(String),
    /// A requested item does not exist.
    NotFound(String),
    /// An operation failed during execution.
    Execution(String),
    /// Post-operation verification failed.
    Verification(String),
    /// Evidence could not be recorded.
    Evidence(String),
}

impl fmt::Display for OidError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(formatter, "invalid input: {value}"),
            Self::InvalidTransition { from, to } => {
                write!(formatter, "invalid transition: {from} -> {to}")
            }
            Self::Unauthorized(reason) => write!(formatter, "unauthorized: {reason}"),
            Self::ApprovalRequired(reason) => write!(formatter, "approval required: {reason}"),
            Self::NotFound(value) => write!(formatter, "not found: {value}"),
            Self::Execution(reason) => write!(formatter, "execution failed: {reason}"),
            Self::Verification(reason) => write!(formatter, "verification failed: {reason}"),
            Self::Evidence(reason) => write!(formatter, "evidence failure: {reason}"),
        }
    }
}

impl std::error::Error for OidError {}

/// A platform-neutral event emitted by a bounded context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FoundationEvent {
    /// An intent changed lifecycle state.
    IntentStateChanged {
        /// Intent whose state changed.
        intent_id: IntentId,
        /// New state label.
        state: String,
    },
    /// An operation requires an approval decision.
    ApprovalRequested {
        /// Operation awaiting approval.
        operation_id: OperationId,
    },
    /// An operation produced a result.
    OperationCompleted {
        /// Operation that completed.
        operation_id: OperationId,
    },
    /// An evidence record was appended.
    EvidenceRecorded {
        /// Evidence record that was written.
        evidence_id: EvidenceId,
    },
}

/// Returns the name of this crate's current foundation layer.
///
/// ```
/// assert_eq!(oid_common::crate_name(), "common");
/// ```
#[must_use]
pub const fn crate_name() -> &'static str {
    "common"
}
