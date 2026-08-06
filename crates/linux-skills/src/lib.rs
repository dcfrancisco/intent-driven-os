//! Typed Linux operation and skill contracts for Open Intelligence Desktop.
//!
//! Concrete operations are deliberately absent from this foundation milestone.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, OperationId, SkillId};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Metadata describing a capability exposed by a skill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillDescriptor {
    /// Stable skill identity.
    pub id: SkillId,
    /// Human-readable summary.
    pub description: String,
    /// Whether the skill can alter system state.
    pub mutates_system: bool,
}

/// Input passed to a skill adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRequest {
    /// Stable operation identity.
    pub id: OperationId,
    /// Serialized, skill-specific arguments.
    pub arguments: String,
}

/// Result returned by a skill adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationResult {
    /// Human-readable summary of the observed change.
    pub summary: String,
    /// Whether the adapter reports that a change occurred.
    pub changed: bool,
}

/// Typed boundary for a Linux operation.
pub trait Skill: Send + Sync {
    /// Return the capability metadata.
    fn descriptor(&self) -> &SkillDescriptor;

    /// Execute the operation after policy and approval have succeeded.
    ///
    /// # Errors
    ///
    /// Returns an operation error when the adapter cannot complete the request.
    fn execute(&self, request: &OperationRequest) -> Result<OperationResult, OidError>;

    /// Attempt a rollback when the operation provides one.
    ///
    /// # Errors
    ///
    /// Returns an error when rollback is unavailable or fails.
    fn rollback(&self, _request: &OperationRequest) -> Result<OperationResult, OidError> {
        Err(OidError::NotFound("rollback implementation".to_owned()))
    }
}

/// Read-only system health inspection skill.
#[derive(Clone, Debug)]
pub struct SystemHealthSkill {
    descriptor: SkillDescriptor,
}

impl Default for SystemHealthSkill {
    fn default() -> Self {
        Self {
            descriptor: SkillDescriptor {
                id: SkillId::new("system-health").expect("static skill id is valid"),
                description: "Inspect portable system health information".to_owned(),
                mutates_system: false,
            },
        }
    }
}

impl Skill for SystemHealthSkill {
    fn descriptor(&self) -> &SkillDescriptor {
        &self.descriptor
    }

    fn execute(&self, _request: &OperationRequest) -> Result<OperationResult, OidError> {
        let platform = std::env::consts::OS;
        let uptime = std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|value| value.split_whitespace().next().map(str::to_owned))
            .unwrap_or_else(|| "unavailable".to_owned());
        let load = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|value| value.split_whitespace().next().map(str::to_owned))
            .unwrap_or_else(|| "unavailable".to_owned());
        Ok(OperationResult {
            summary: format!("platform={platform}; uptime_seconds={uptime}; load_1m={load}"),
            changed: false,
        })
    }
}

/// Linux-specific read-only health adapter backed by `/proc`.
#[derive(Clone, Debug)]
pub struct LinuxSystemHealthSkill {
    descriptor: SkillDescriptor,
}

impl Default for LinuxSystemHealthSkill {
    fn default() -> Self {
        Self {
            descriptor: SkillDescriptor {
                id: SkillId::new("linux-system-health").expect("static skill id is valid"),
                description: "Inspect Linux /proc health information".to_owned(),
                mutates_system: false,
            },
        }
    }
}

impl Skill for LinuxSystemHealthSkill {
    fn descriptor(&self) -> &SkillDescriptor {
        &self.descriptor
    }

    fn execute(&self, _request: &OperationRequest) -> Result<OperationResult, OidError> {
        if std::env::consts::OS != "linux" {
            return Err(OidError::Execution(
                "linux-system-health requires a Linux host".to_owned(),
            ));
        }
        let uptime = read_proc_value("/proc/uptime")?;
        let load = read_proc_value("/proc/loadavg")?;
        let memory = std::fs::read_to_string("/proc/meminfo")
            .map_err(|error| OidError::Execution(format!("read /proc/meminfo: {error}")))?
            .lines()
            .find(|line| line.starts_with("MemAvailable:"))
            .unwrap_or("MemAvailable: unavailable")
            .to_owned();
        Ok(OperationResult {
            summary: format!("platform=linux; uptime_seconds={uptime}; load_1m={load}; {memory}"),
            changed: false,
        })
    }
}

fn read_proc_value(path: &str) -> Result<String, OidError> {
    std::fs::read_to_string(path)
        .map_err(|error| OidError::Execution(format!("read {path}: {error}")))?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| OidError::Execution(format!("empty {path}")))
}

/// Explicitly approved operation that creates one new directory.
#[derive(Debug)]
pub struct CreateDirectorySkill {
    descriptor: SkillDescriptor,
    created: Mutex<Option<PathBuf>>,
}

impl CreateDirectorySkill {
    /// Construct the directory skill with its fixed capability metadata.
    ///
    /// # Panics
    ///
    /// This cannot panic unless the compile-time capability identifier is changed to an invalid value.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: SkillDescriptor {
                id: SkillId::new("create-directory").expect("static skill id is valid"),
                description: "Create one explicitly approved empty directory".to_owned(),
                mutates_system: true,
            },
            created: Mutex::new(None),
        }
    }
}

impl Default for CreateDirectorySkill {
    fn default() -> Self {
        Self::new()
    }
}

impl Skill for CreateDirectorySkill {
    fn descriptor(&self) -> &SkillDescriptor {
        &self.descriptor
    }

    fn execute(&self, request: &OperationRequest) -> Result<OperationResult, OidError> {
        let path = validate_directory_path(&request.arguments)?;
        std::fs::create_dir(&path)
            .map_err(|error| OidError::Execution(format!("create {}: {error}", path.display())))?;
        *self
            .created
            .lock()
            .map_err(|_| OidError::Execution("directory skill lock poisoned".to_owned()))? =
            Some(path.clone());
        Ok(OperationResult {
            summary: format!("created directory {}", path.display()),
            changed: true,
        })
    }

    fn rollback(&self, request: &OperationRequest) -> Result<OperationResult, OidError> {
        let path = parse_directory_path(&request.arguments)?;
        if !path.is_dir() {
            return Err(OidError::NotFound("created directory".to_owned()));
        }
        let created = self
            .created
            .lock()
            .map_err(|_| OidError::Execution("directory skill lock poisoned".to_owned()))?;
        if created.as_deref() != Some(path.as_path()) {
            return Err(OidError::NotFound(
                "directory created by this operation".to_owned(),
            ));
        }
        drop(created);
        std::fs::remove_dir(&path).map_err(|error| {
            OidError::Execution(format!("rollback {}: {error}", path.display()))
        })?;
        Ok(OperationResult {
            summary: format!("removed directory {}", path.display()),
            changed: true,
        })
    }
}

fn validate_directory_path(raw: &str) -> Result<PathBuf, OidError> {
    let path = parse_directory_path(raw)?;
    if path.exists() {
        return Err(OidError::InvalidInput(
            "directory path already exists".to_owned(),
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(OidError::InvalidInput(
                "directory parent must already exist".to_owned(),
            ));
        }
    }
    Ok(path)
}

fn parse_directory_path(raw: &str) -> Result<PathBuf, OidError> {
    let path = Path::new(raw.trim());
    if raw.trim().is_empty() || path == Path::new(".") || path == Path::new("/") {
        return Err(OidError::InvalidInput(
            "directory path must name a new directory".to_owned(),
        ));
    }
    if path
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(OidError::InvalidInput(
            "directory path must not contain '..'".to_owned(),
        ));
    }
    Ok(path.to_path_buf())
}

/// Identifies the Linux operations boundary.
///
/// ```
/// assert_eq!(oid_linux_skills::boundary_name(), "linux-skills");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "linux-skills"
}

#[cfg(test)]
mod tests {
    use super::{CreateDirectorySkill, OperationRequest, Skill, SystemHealthSkill};
    use oid_common::OperationId;

    #[test]
    fn system_health_is_read_only_and_has_no_rollback() {
        let skill = SystemHealthSkill::default();
        let result = skill
            .execute(&OperationRequest {
                id: OperationId::new("operation-1").expect("valid id"),
                arguments: String::new(),
            })
            .expect("health inspection succeeds");
        assert!(!result.changed);
        assert!(skill
            .rollback(&OperationRequest {
                id: OperationId::new("operation-1").expect("valid id"),
                arguments: String::new(),
            })
            .is_err());
    }

    #[test]
    fn directory_skill_rejects_unsafe_paths() {
        let skill = CreateDirectorySkill::new();
        let request = OperationRequest {
            id: OperationId::new("operation-2").expect("valid id"),
            arguments: "../outside".to_owned(),
        };
        assert!(skill.execute(&request).is_err());
    }

    #[test]
    fn directory_skill_can_rollback_its_empty_directory() {
        let path = std::env::temp_dir().join(format!("oid-directory-test-{}", std::process::id()));
        let path = path.to_string_lossy().into_owned();
        let skill = CreateDirectorySkill::default();
        let request = OperationRequest {
            id: OperationId::new("operation-3").expect("valid id"),
            arguments: path.clone(),
        };
        let result = skill.execute(&request).expect("directory creation works");
        assert!(result.changed);
        skill
            .rollback(&request)
            .expect("empty directory rolls back");
        assert!(!std::path::Path::new(&path).exists());
    }
}
