//! Typed Linux operation and skill contracts for Open Intelligence Desktop.
//!
//! Concrete operations are deliberately absent from this foundation milestone.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{
    ActionType, ApprovalRequirement, ApprovedOperationPlan, ExecutionResult, OidError, OperationId,
    OperationPlan, OperationStep, RiskLevel, RollbackPlan, RollbackResult, RollbackStep, SkillId,
    VerificationCheck, VerificationPlan, VerificationResult,
};
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
pub struct SkillRequest {
    /// Stable operation identity.
    pub id: OperationId,
    /// Serialized, skill-specific arguments.
    pub arguments: String,
}

/// Backward-compatible name for a skill request.
pub type OperationRequest = SkillRequest;

/// Backward-compatible name for an execution result.
pub type OperationResult = ExecutionResult;

/// Typed boundary for a Linux operation.
pub trait Skill: Send + Sync {
    /// Return the capability metadata.
    fn descriptor(&self) -> &SkillDescriptor;

    /// Produce a validated operation plan without changing system state.
    ///
    /// # Errors
    ///
    /// Returns an error when the request cannot be planned safely.
    fn plan(&self, request: &SkillRequest) -> Result<OperationPlan, OidError>;

    /// Execute only an approved operation plan.
    ///
    /// # Errors
    ///
    /// Returns an operation error when the approved plan cannot be completed.
    fn execute(&self, approved_plan: &ApprovedOperationPlan) -> Result<ExecutionResult, OidError>;

    /// Verify the execution against the plan's declared checks.
    ///
    /// # Errors
    ///
    /// Returns an error when verification cannot be completed.
    fn verify(&self, execution: &ExecutionResult) -> Result<VerificationResult, OidError>;

    /// Roll back an execution when the plan declares rollback support.
    ///
    /// # Errors
    ///
    /// Returns an error when rollback is unavailable or fails.
    fn rollback(&self, execution: &ExecutionResult) -> Result<RollbackResult, OidError>;

    /// Return a standard unsupported rollback result for read-only skills.
    ///
    /// # Errors
    ///
    /// Always returns an error because no rollback implementation is available.
    fn no_rollback(execution: &ExecutionResult) -> Result<RollbackResult, OidError>
    where
        Self: Sized,
    {
        let _ = execution;
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

    fn plan(&self, request: &SkillRequest) -> Result<OperationPlan, OidError> {
        health_plan(
            request.id.clone(),
            self.descriptor.id.clone(),
            "system health",
        )
    }

    fn execute(&self, approved_plan: &ApprovedOperationPlan) -> Result<ExecutionResult, OidError> {
        if approved_plan.plan.skill != self.descriptor.id {
            return Err(OidError::Unauthorized(
                "plan belongs to another skill".to_owned(),
            ));
        }
        let platform = std::env::consts::OS;
        let uptime = std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|value| value.split_whitespace().next().map(str::to_owned))
            .unwrap_or_else(|| "unavailable".to_owned());
        let load = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|value| value.split_whitespace().next().map(str::to_owned))
            .unwrap_or_else(|| "unavailable".to_owned());
        Ok(ExecutionResult {
            operation_id: approved_plan.plan.id.clone(),
            summary: format!("platform={platform}; uptime_seconds={uptime}; load_1m={load}"),
            changed: false,
        })
    }

    fn verify(&self, execution: &ExecutionResult) -> Result<VerificationResult, OidError> {
        Ok(VerificationResult {
            operation_id: execution.operation_id.clone(),
            passed: true,
            summary: "health snapshot collected".to_owned(),
        })
    }

    fn rollback(&self, execution: &ExecutionResult) -> Result<RollbackResult, OidError> {
        Self::no_rollback(execution)
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

    fn plan(&self, request: &SkillRequest) -> Result<OperationPlan, OidError> {
        health_plan(
            request.id.clone(),
            self.descriptor.id.clone(),
            "Linux /proc health",
        )
    }

    fn execute(&self, approved_plan: &ApprovedOperationPlan) -> Result<ExecutionResult, OidError> {
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
        Ok(ExecutionResult {
            operation_id: approved_plan.plan.id.clone(),
            summary: format!("platform=linux; uptime_seconds={uptime}; load_1m={load}; {memory}"),
            changed: false,
        })
    }

    fn verify(&self, execution: &ExecutionResult) -> Result<VerificationResult, OidError> {
        Ok(VerificationResult {
            operation_id: execution.operation_id.clone(),
            passed: true,
            summary: "Linux /proc checks passed".to_owned(),
        })
    }

    fn rollback(&self, execution: &ExecutionResult) -> Result<RollbackResult, OidError> {
        Self::no_rollback(execution)
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

    fn plan(&self, request: &SkillRequest) -> Result<OperationPlan, OidError> {
        let path = validate_directory_path(&request.arguments)?;
        Ok(OperationPlan {
            id: request.id.clone(),
            intent: None,
            skill: self.descriptor.id.clone(),
            summary: "Create directory".to_owned(),
            rationale: format!(
                "Create the explicitly requested directory {}",
                path.display()
            ),
            risk: RiskLevel::Low,
            approval: ApprovalRequirement::User,
            steps: vec![OperationStep {
                order: 1,
                description: format!("Create {}", path.display()),
                action: ActionType::CreateDirectory {
                    path: path.display().to_string(),
                },
                affects_system_state: true,
            }],
            verification: VerificationPlan {
                checks: vec![VerificationCheck {
                    description: "Directory exists".to_owned(),
                    kind: "directory-exists".to_owned(),
                }],
            },
            rollback: Some(RollbackPlan {
                supported: true,
                description: "Delete directory if empty".to_owned(),
                steps: vec![RollbackStep {
                    order: 1,
                    description: format!("Delete {} if empty", path.display()),
                    action: ActionType::RemoveDirectory {
                        path: path.display().to_string(),
                    },
                }],
            }),
        })
    }

    fn execute(&self, approved_plan: &ApprovedOperationPlan) -> Result<ExecutionResult, OidError> {
        let path = plan_directory_path(&approved_plan.plan)?;
        std::fs::create_dir(&path)
            .map_err(|error| OidError::Execution(format!("create {}: {error}", path.display())))?;
        *self
            .created
            .lock()
            .map_err(|_| OidError::Execution("directory skill lock poisoned".to_owned()))? =
            Some(path.clone());
        Ok(ExecutionResult {
            operation_id: approved_plan.plan.id.clone(),
            summary: format!("created directory {}", path.display()),
            changed: true,
        })
    }

    fn verify(&self, execution: &ExecutionResult) -> Result<VerificationResult, OidError> {
        let path = self
            .created
            .lock()
            .map_err(|_| OidError::Execution("directory skill lock poisoned".to_owned()))?
            .clone()
            .ok_or_else(|| OidError::NotFound("created directory".to_owned()))?;
        Ok(VerificationResult {
            operation_id: execution.operation_id.clone(),
            passed: path.is_dir(),
            summary: if path.is_dir() {
                "directory exists".to_owned()
            } else {
                "directory does not exist".to_owned()
            },
        })
    }

    fn rollback(&self, execution: &ExecutionResult) -> Result<RollbackResult, OidError> {
        let _ = execution;
        let created = self
            .created
            .lock()
            .map_err(|_| OidError::Execution("directory skill lock poisoned".to_owned()))?;
        let Some(path) = created.clone() else {
            return Err(OidError::NotFound("created directory".to_owned()));
        };
        drop(created);
        if !path.is_dir() {
            return Err(OidError::NotFound("created directory".to_owned()));
        }
        std::fs::remove_dir(&path).map_err(|error| {
            OidError::Execution(format!("rollback {}: {error}", path.display()))
        })?;
        Ok(RollbackResult {
            operation_id: execution.operation_id.clone(),
            summary: format!("removed directory {}", path.display()),
            changed: true,
        })
    }
}

fn health_plan(
    id: OperationId,
    skill: SkillId,
    description: &str,
) -> Result<OperationPlan, OidError> {
    let plan = OperationPlan {
        id,
        intent: None,
        skill,
        summary: "Inspect system health".to_owned(),
        rationale: description.to_owned(),
        risk: RiskLevel::None,
        approval: ApprovalRequirement::None,
        steps: vec![OperationStep {
            order: 1,
            description: description.to_owned(),
            action: ActionType::InspectSystemHealth,
            affects_system_state: false,
        }],
        verification: VerificationPlan {
            checks: vec![VerificationCheck {
                description: "Health snapshot is readable".to_owned(),
                kind: "snapshot-readable".to_owned(),
            }],
        },
        rollback: None,
    };
    plan.validate()?;
    Ok(plan)
}

fn plan_directory_path(plan: &OperationPlan) -> Result<PathBuf, OidError> {
    plan.steps
        .iter()
        .find_map(|step| match &step.action {
            ActionType::CreateDirectory { path } => Some(parse_directory_path(path)),
            _ => None,
        })
        .ok_or_else(|| OidError::InvalidInput("directory plan has no create step".to_owned()))?
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
    use oid_common::{ApprovedOperationPlan, OperationId};

    #[test]
    fn system_health_is_read_only_and_has_no_rollback() {
        let skill = SystemHealthSkill::default();
        let request = OperationRequest {
            id: OperationId::new("operation-1").expect("valid id"),
            arguments: String::new(),
        };
        let plan = skill.plan(&request).expect("health plan succeeds");
        let approved = ApprovedOperationPlan::new(plan, "test").expect("plan approved");
        let result = skill
            .execute(&approved)
            .expect("health inspection succeeds");
        assert!(!result.changed);
        assert!(skill.rollback(&result).is_err());
    }

    #[test]
    fn directory_skill_rejects_unsafe_paths() {
        let skill = CreateDirectorySkill::new();
        let request = OperationRequest {
            id: OperationId::new("operation-2").expect("valid id"),
            arguments: "../outside".to_owned(),
        };
        assert!(skill.plan(&request).is_err());
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
        let plan = skill.plan(&request).expect("directory plan works");
        let approved = ApprovedOperationPlan::new(plan, "test").expect("plan approved");
        let result = skill.execute(&approved).expect("directory creation works");
        assert!(result.changed);
        skill.rollback(&result).expect("empty directory rolls back");
        assert!(!std::path::Path::new(&path).exists());
    }
}
