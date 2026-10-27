//! Build identity shared by the Talvik client and daemon binaries.

use serde_json::{json, Value};

/// Immutable identity embedded in a Talvik binary at build time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildIdentity {
    /// Product version.
    pub version: &'static str,
    /// Source revision, or `unknown` when unavailable.
    pub git_sha: &'static str,
    /// Whether tracked source changes were present during the build.
    pub dirty: bool,
    /// Rust target triple.
    pub target: &'static str,
    /// Cargo build profile.
    pub profile: &'static str,
}

/// Return this binary's embedded build identity.
#[must_use]
pub fn current() -> BuildIdentity {
    BuildIdentity {
        version: env!("CARGO_PKG_VERSION"),
        git_sha: option_env!("TALVIK_GIT_SHA").unwrap_or("unknown"),
        dirty: option_env!("TALVIK_GIT_DIRTY") == Some("true"),
        target: option_env!("TALVIK_BUILD_TARGET").unwrap_or("unknown"),
        profile: option_env!("TALVIK_BUILD_PROFILE").unwrap_or("unknown"),
    }
}

/// Serialize build identity for machine-readable clients.
#[must_use]
pub fn json(component: &str, native_backend: Option<&str>) -> Value {
    let identity = current();
    json!({
        "object": "talvik.build",
        "component": component,
        "version": identity.version,
        "git_sha": identity.git_sha,
        "dirty": identity.dirty,
        "target": identity.target,
        "profile": identity.profile,
        "native_backend": native_backend,
    })
}

/// Render a stable human-readable identity line.
#[must_use]
pub fn display(component: &str) -> String {
    let identity = current();
    format!(
        "{component} {} (git={}, dirty={}, target={}, profile={})",
        identity.version, identity.git_sha, identity.dirty, identity.target, identity.profile
    )
}
