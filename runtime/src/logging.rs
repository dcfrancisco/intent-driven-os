//! Logging initialization boundary.

/// Placeholder logger handle for the Phase 1 startup lifecycle.
#[derive(Clone, Debug, Default)]
pub struct Logger;

/// Initialize logging without configuring a concrete logging backend yet.
#[must_use]
pub fn initialize() -> Logger {
    Logger
}
