//! Canonical operation planning and execution models.

use crate::{IntentId, OidError, OperationId, SkillId};

/// Minimal intent context embedded in a plan to avoid a dependency cycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentContext {
    /// Intent identity.
    pub id: IntentId,
    /// User-visible intent description.
    pub description: String,
}

/// Relative risk of an operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RiskLevel {
    /// No system state is changed.
    None,
    /// A reversible, low-impact change.
    Low,
    /// A material but bounded change.
    Medium,
    /// A high-impact or broad change.
    High,
    /// A potentially catastrophic change.
    Critical,
}

impl RiskLevel {
    /// Return the stable display label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }
}

/// Approval authority required before execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalRequirement {
    /// The operation may execute without interactive approval.
    None,
    /// The user must explicitly approve.
    User,
    /// An administrator must approve.
    Administrator,
    /// A policy engine must issue an approval decision.
    Policy,
}

impl ApprovalRequirement {
    /// Return the stable display label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::User => "User",
            Self::Administrator => "Administrator",
            Self::Policy => "Policy",
        }
    }
}

/// Operation action represented by a plan step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionType {
    /// Inspect system health without mutation.
    InspectSystemHealth,
    /// Create one directory at a validated path.
    CreateDirectory {
        /// Directory target.
        path: String,
    },
    /// Remove an empty directory created by a governed operation.
    RemoveDirectory {
        /// Directory target.
        path: String,
    },
    /// Execute a native executable through the governed native boundary.
    NativeCommand {
        /// Executable name.
        command: String,
        /// Arguments passed to the executable.
        args: Vec<String>,
    },
    /// A provider-defined action.
    Capability {
        /// Provider-defined action name.
        name: String,
    },
}

impl ActionType {
    /// Return a stable action label.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::InspectSystemHealth => "inspect-system-health".to_owned(),
            Self::CreateDirectory { path } => format!("create-directory:{path}"),
            Self::RemoveDirectory { path } => format!("remove-directory:{path}"),
            Self::NativeCommand { command, .. } => format!("native-command:{command}"),
            Self::Capability { name } => format!("capability:{name}"),
        }
    }
}

/// One ordered action in an operation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationStep {
    /// Zero-based or one-based caller-defined order, stable within a plan.
    pub order: u32,
    /// User-visible explanation of the action.
    pub description: String,
    /// Typed action to be performed.
    pub action: ActionType,
    /// Whether the step can alter system state.
    pub affects_system_state: bool,
}

/// A check that must be evaluated after execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationCheck {
    /// User-visible check description.
    pub description: String,
    /// Stable check kind.
    pub kind: String,
}

/// Checks that define successful execution before it begins.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct VerificationPlan {
    /// Ordered postcondition checks.
    pub checks: Vec<VerificationCheck>,
}

/// One rollback action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackStep {
    /// Ordered rollback step number.
    pub order: u32,
    /// User-visible explanation.
    pub description: String,
    /// Typed inverse action.
    pub action: ActionType,
}

/// Rollback capability declared by a plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackPlan {
    /// Whether rollback is supported for this plan.
    pub supported: bool,
    /// User-visible rollback behavior.
    pub description: String,
    /// Ordered inverse steps.
    pub steps: Vec<RollbackStep>,
}

/// Canonical, inspectable operation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationPlan {
    /// Stable operation identity.
    pub id: OperationId,
    /// Optional intent context snapshot.
    pub intent: Option<IntentContext>,
    /// Skill or capability that owns execution.
    pub skill: SkillId,
    /// Short summary shown in the terminal.
    pub summary: String,
    /// Rationale shown before approval.
    pub rationale: String,
    /// Declared operation risk.
    pub risk: RiskLevel,
    /// Required approval authority.
    pub approval: ApprovalRequirement,
    /// Ordered execution steps.
    pub steps: Vec<OperationStep>,
    /// Postconditions defined before execution.
    pub verification: VerificationPlan,
    /// Optional rollback declaration.
    pub rollback: Option<RollbackPlan>,
}

impl OperationPlan {
    /// Validate plan invariants before presenting or executing a plan.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan has no steps, no verification checks, or
    /// declares inconsistent risk and mutation metadata.
    pub fn validate(&self) -> Result<(), OidError> {
        if self.steps.is_empty() {
            return Err(OidError::InvalidInput(
                "operation plan requires a step".to_owned(),
            ));
        }
        if self.verification.checks.is_empty() {
            return Err(OidError::InvalidInput(
                "operation plan requires verification checks".to_owned(),
            ));
        }
        let mutates = self.steps.iter().any(|step| step.affects_system_state);
        if mutates && self.risk == RiskLevel::None {
            return Err(OidError::InvalidInput(
                "mutating plan cannot have None risk".to_owned(),
            ));
        }
        if mutates && self.approval == ApprovalRequirement::None {
            return Err(OidError::InvalidInput(
                "mutating plan requires approval".to_owned(),
            ));
        }
        Ok(())
    }

    /// Serialize the plan into deterministic JSON without external dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan violates its invariants.
    pub fn to_json(&self) -> Result<String, OidError> {
        self.validate()?;
        let steps = self
            .steps
            .iter()
            .map(|step| {
                format!(
                    "{{\"order\":{},\"description\":{},\"action\":{},\"affects_system_state\":{}}}",
                    step.order,
                    json_string(&step.description),
                    json_string(&step.action.label()),
                    step.affects_system_state
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let checks = self
            .verification
            .checks
            .iter()
            .map(|check| {
                format!(
                    "{{\"description\":{},\"kind\":{}}}",
                    json_string(&check.description),
                    json_string(&check.kind)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let intent = self.intent.as_ref().map_or_else(
            || "null".to_owned(),
            |intent| {
                format!(
                    "{{\"id\":{},\"description\":{}}}",
                    json_string(intent.id.as_str()),
                    json_string(&intent.description)
                )
            },
        );
        let rollback = self.rollback.as_ref().map_or_else(
            || "null".to_owned(),
            |rollback| {
                let steps = rollback
                    .steps
                    .iter()
                    .map(|step| {
                        format!(
                            "{{\"order\":{},\"description\":{},\"action\":{}}}",
                            step.order,
                            json_string(&step.description),
                            json_string(&step.action.label())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"supported\":{},\"description\":{},\"steps\":[{}]}}",
                    rollback.supported,
                    json_string(&rollback.description),
                    steps
                )
            },
        );
        Ok(format!(
            "{{\"id\":{},\"intent\":{},\"skill\":{},\"summary\":{},\"rationale\":{},\"risk\":{},\"approval\":{},\"steps\":[{}],\"verification\":[{}],\"rollback\":{}}}",
            json_string(self.id.as_str()), intent, json_string(self.skill.as_str()), json_string(&self.summary),
            json_string(&self.rationale), json_string(self.risk.as_str()), json_string(self.approval.as_str()),
            steps, checks, rollback
        ))
    }
}

/// A plan that has passed approval and is allowed to execute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedOperationPlan {
    /// The validated plan.
    pub plan: OperationPlan,
    /// Human-readable authority that approved execution.
    pub approved_by: String,
}

impl ApprovedOperationPlan {
    /// Construct an approved plan after validating it.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan is invalid or the approver is empty.
    pub fn new(plan: OperationPlan, approved_by: impl Into<String>) -> Result<Self, OidError> {
        plan.validate()?;
        let approved_by = approved_by.into();
        if approved_by.trim().is_empty() {
            return Err(OidError::InvalidInput("approval authority".to_owned()));
        }
        Ok(Self { plan, approved_by })
    }
}

/// Result produced by an approved execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionResult {
    /// Operation that was executed.
    pub operation_id: OperationId,
    /// Human-readable result.
    pub summary: String,
    /// Whether system state changed.
    pub changed: bool,
}

/// Result produced by verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationResult {
    /// Operation that was verified.
    pub operation_id: OperationId,
    /// Whether all plan checks passed.
    pub passed: bool,
    /// Human-readable result.
    pub summary: String,
}

/// Result produced by rollback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackResult {
    /// Operation whose effects were rolled back.
    pub operation_id: OperationId,
    /// Whether rollback changed the system.
    pub changed: bool,
    /// Human-readable result.
    pub summary: String,
}

fn json_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> OperationPlan {
        OperationPlan {
            id: OperationId::new("operation-1").expect("valid id"),
            intent: Some(IntentContext {
                id: IntentId::new("intent-1").expect("valid id"),
                description: "create a directory".to_owned(),
            }),
            skill: SkillId::new("create-directory").expect("valid id"),
            summary: "Create directory".to_owned(),
            rationale: "User requested it".to_owned(),
            risk: RiskLevel::Low,
            approval: ApprovalRequirement::User,
            steps: vec![OperationStep {
                order: 1,
                description: "Create directory".to_owned(),
                action: ActionType::CreateDirectory {
                    path: "/tmp/example".to_owned(),
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
                description: "Delete if empty".to_owned(),
                steps: vec![RollbackStep {
                    order: 1,
                    description: "Delete directory".to_owned(),
                    action: ActionType::RemoveDirectory {
                        path: "/tmp/example".to_owned(),
                    },
                }],
            }),
        }
    }

    #[test]
    fn plan_serialization_contains_intent_and_rollback_steps() {
        let serialized = plan().to_json().expect("valid plan serializes");
        assert!(serialized.contains("\"intent\""));
        assert!(serialized.contains("remove-directory:/tmp/example"));
        assert!(serialized.contains("\"supported\":true"));
    }

    #[test]
    fn mutating_plan_requires_approval() {
        let mut plan = plan();
        plan.approval = ApprovalRequirement::None;
        assert!(plan.validate().is_err());
    }
}
