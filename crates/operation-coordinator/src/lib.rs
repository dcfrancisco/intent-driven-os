//! Durable operation orchestration for Open Intelligence Desktop.
//!
//! This crate composes planning, policy, approval, skill execution, verification,
//! and evidence without owning the individual bounded-context rules.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{
    ActionType, ApprovalRequirement, ApprovedOperationPlan, ExecutionResult, OidError, OperationId,
    OperationPlan, OperationStep, RiskLevel, VerificationCheck, VerificationPlan,
};
use oid_evidence_engine::{EvidenceRecord, EvidenceStore};
use oid_linux_skills::{Skill, SkillDescriptor, SkillRequest};
use oid_plugin_sdk::{CapabilityCommand, CapabilityDescriptor, CapabilityRegistry};
use oid_policy_engine::{Approval, ApprovalLifecycle, PolicyEvaluator, ReadOnlyPolicy};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Durable lifecycle state for one coordinated operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationStatus {
    /// Plan was created and is inspectable.
    Planned,
    /// Approval is required before execution.
    AwaitingApproval,
    /// Approval was recorded.
    Approved,
    /// Skill execution has started.
    Executing,
    /// Skill execution completed.
    Executed,
    /// Verification completed successfully.
    Verified,
    /// A lifecycle stage failed.
    Failed,
    /// Execution was explicitly rolled back.
    RolledBack,
    /// Rollback was attempted but failed.
    RollbackFailed,
}

impl OperationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::AwaitingApproval => "awaiting-approval",
            Self::Approved => "approved",
            Self::Executing => "executing",
            Self::Executed => "executed",
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::RolledBack => "rolled-back",
            Self::RollbackFailed => "rollback-failed",
        }
    }

    fn parse(value: &str) -> Result<Self, OidError> {
        match value {
            "planned" => Ok(Self::Planned),
            "awaiting-approval" => Ok(Self::AwaitingApproval),
            "approved" => Ok(Self::Approved),
            "executing" => Ok(Self::Executing),
            "executed" => Ok(Self::Executed),
            "verified" => Ok(Self::Verified),
            "failed" => Ok(Self::Failed),
            "rolled-back" => Ok(Self::RolledBack),
            "rollback-failed" => Ok(Self::RollbackFailed),
            _ => Err(OidError::Execution(format!(
                "unknown operation status: {value}"
            ))),
        }
    }
}

/// Durable operation journal record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRecord {
    /// Operation identity.
    pub operation_id: OperationId,
    /// Latest lifecycle state.
    pub status: OperationStatus,
    /// Human-readable state detail.
    pub detail: String,
}

/// Append-only operation state journal.
#[derive(Clone, Debug)]
pub struct OperationJournal {
    path: PathBuf,
}

impl OperationJournal {
    /// Create a journal at a path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Return the journal path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append a state transition without rewriting history.
    ///
    /// # Errors
    ///
    /// Returns an error when the journal cannot be opened or appended.
    pub fn append(&self, record: &OperationRecord) -> Result<(), OidError> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| {
                OidError::Evidence(format!("open {}: {error}", self.path.display()))
            })?;
        writeln!(
            file,
            "{}\t{}\t{}",
            encode(record.operation_id.as_str()),
            record.status.as_str(),
            encode(&record.detail)
        )
        .map_err(|error| OidError::Evidence(format!("append {}: {error}", self.path.display())))
    }

    /// Return the latest record for each operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the journal is unreadable or malformed.
    pub fn latest(&self) -> Result<Vec<OperationRecord>, OidError> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(OidError::Evidence(format!(
                    "read {}: {error}",
                    self.path.display()
                )))
            }
        };
        let mut records = BTreeMap::new();
        for line in BufReader::new(file).lines() {
            let fields: Vec<_> = line
                .map_err(|error| OidError::Evidence(error.to_string()))?
                .split('\t')
                .map(decode)
                .collect::<Result<_, _>>()?;
            if fields.len() != 3 {
                return Err(OidError::Evidence(
                    "malformed operation journal record".to_owned(),
                ));
            }
            let operation_id = OperationId::new(fields[0].clone())
                .map_err(|error| OidError::Evidence(error.to_string()))?;
            records.insert(
                operation_id.clone(),
                OperationRecord {
                    operation_id,
                    status: OperationStatus::parse(&fields[1])?,
                    detail: fields[2].clone(),
                },
            );
        }
        Ok(records.into_values().collect())
    }

    /// Return operations that may need recovery after interruption.
    ///
    /// # Errors
    ///
    /// Returns an error when the journal is unreadable or malformed.
    pub fn recoverable(&self) -> Result<Vec<OperationRecord>, OidError> {
        Ok(self
            .latest()?
            .into_iter()
            .filter(|record| {
                !matches!(
                    record.status,
                    OperationStatus::Verified
                        | OperationStatus::Failed
                        | OperationStatus::RolledBack
                        | OperationStatus::RollbackFailed
                )
            })
            .collect())
    }
}

/// Coordinates a complete governed operation lifecycle.
pub struct OperationCoordinator<A, E>
where
    A: ApprovalLifecycle,
    E: EvidenceStore,
{
    policy: ReadOnlyPolicy,
    approvals: A,
    evidence: E,
    journal: OperationJournal,
    skills: BTreeMap<String, Box<dyn Skill>>,
    dynamic: BTreeMap<String, String>,
    registry: CapabilityRegistry,
    completed: BTreeMap<OperationId, (String, ExecutionResult)>,
}

impl<A, E> fmt::Debug for OperationCoordinator<A, E>
where
    A: ApprovalLifecycle + fmt::Debug,
    E: EvidenceStore + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationCoordinator")
            .field("policy", &self.policy)
            .field("approvals", &self.approvals)
            .field("evidence", &self.evidence)
            .field("journal", &self.journal)
            .field("skills", &self.skills.keys().collect::<Vec<_>>())
            .field("dynamic", &self.dynamic)
            .field("registry", &self.registry)
            .field("completed", &self.completed.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl<A, E> OperationCoordinator<A, E>
where
    A: ApprovalLifecycle,
    E: EvidenceStore,
{
    /// Construct a coordinator with durable operation state and injected stores.
    #[must_use]
    pub fn new(approvals: A, evidence: E, journal: OperationJournal) -> Self {
        Self {
            policy: ReadOnlyPolicy,
            approvals,
            evidence,
            journal,
            skills: BTreeMap::new(),
            dynamic: BTreeMap::new(),
            registry: CapabilityRegistry::new(),
            completed: BTreeMap::new(),
        }
    }

    /// Register a skill implementation by its stable skill identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when another skill already owns the identifier.
    pub fn register_skill(&mut self, skill: Box<dyn Skill>) -> Result<(), OidError> {
        let id = skill.descriptor().id.to_string();
        if self.skills.insert(id.clone(), skill).is_some() {
            return Err(OidError::InvalidInput(format!(
                "skill already registered: {id}"
            )));
        }
        Ok(())
    }

    /// Register a dynamic command and its governed skill implementation.
    ///
    /// # Errors
    ///
    /// Returns an error when capability metadata is invalid or registration collides.
    pub fn register_dynamic(
        &mut self,
        command: CapabilityCommand,
        plugin_id: oid_common::PluginId,
        skill: Box<dyn Skill>,
    ) -> Result<(), OidError> {
        let skill_id = skill.descriptor().id.clone();
        self.registry.register(CapabilityDescriptor {
            id: skill_id.clone(),
            plugin_id,
            commands: vec![command.clone()],
        })?;
        self.register_skill(skill)?;
        self.dynamic.insert(command.name, skill_id.to_string());
        Ok(())
    }

    /// Return the registry used to discover dynamic commands.
    #[must_use]
    pub const fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Produce a plan for a registered skill without changing state.
    ///
    /// # Errors
    ///
    /// Returns an error when the skill cannot safely plan the request.
    pub fn preview(
        &self,
        skill_id: &str,
        request: &SkillRequest,
    ) -> Result<OperationPlan, OidError> {
        self.skills
            .get(skill_id)
            .ok_or_else(|| OidError::NotFound(format!("skill: {skill_id}")))?
            .plan(request)
    }

    /// Plan, authorize, execute, verify, and record one registered skill.
    ///
    /// # Errors
    ///
    /// Returns an error when planning, authorization, approval, execution,
    /// verification, or evidence persistence fails.
    pub fn run(
        &mut self,
        skill_id: &str,
        request: &SkillRequest,
        approve: bool,
    ) -> Result<ExecutionResult, OidError> {
        let (plan, mutates_system) = {
            let skill = self
                .skills
                .get(skill_id)
                .ok_or_else(|| OidError::NotFound(format!("skill: {skill_id}")))?;
            (skill.plan(request)?, skill.descriptor().mutates_system)
        };
        self.transition(&plan.id, OperationStatus::Planned, &plan.summary)?;
        self.record(&plan, "plan", &plan.to_json()?)?;
        let decision = self
            .policy
            .evaluate(&oid_policy_engine::AuthorizationRequest {
                operation_id: plan.id.clone(),
                skill_id: plan.skill.clone(),
                mutates_system,
            })?;
        let policy_requires_approval = matches!(
            decision,
            oid_policy_engine::AuthorizationDecision::ApprovalRequired { .. }
        );
        let approval_required =
            plan.approval != ApprovalRequirement::None || policy_requires_approval;
        if approval_required {
            let already_approved = self.approvals.is_approved(&plan.id)?;
            if !already_approved {
                self.approvals.request(plan.id.clone())?;
            }
            self.transition(
                &plan.id,
                OperationStatus::AwaitingApproval,
                "approval required",
            )?;
            if !approve && !already_approved {
                return Err(OidError::ApprovalRequired(plan.id.to_string()));
            }
            if approve && !already_approved {
                self.approvals.approve(Approval {
                    operation_id: plan.id.clone(),
                    note: "approved by coordinator".to_owned(),
                })?;
            }
        }
        let approved = ApprovedOperationPlan::new(plan.clone(), "coordinator")?;
        if approval_required {
            self.approvals.consume(&plan.id)?;
        }
        self.transition(&plan.id, OperationStatus::Approved, "approval complete")?;
        self.record(&plan, "approval", "user approval recorded")?;
        self.transition(
            &plan.id,
            OperationStatus::Executing,
            "skill execution started",
        )?;
        let result = match self
            .skills
            .get(skill_id)
            .ok_or_else(|| OidError::NotFound(format!("skill: {skill_id}")))?
            .execute(&approved)
        {
            Ok(result) => result,
            Err(error) => {
                self.transition(&plan.id, OperationStatus::Failed, &error.to_string())?;
                return Err(error);
            }
        };
        self.transition(&plan.id, OperationStatus::Executed, &result.summary)?;
        self.record(&plan, "execution", &result.summary)?;
        let verification = self
            .skills
            .get(skill_id)
            .ok_or_else(|| OidError::NotFound(format!("skill: {skill_id}")))?
            .verify(&result)?;
        self.record(&plan, "verification", &verification.summary)?;
        if !verification.passed {
            self.transition(&plan.id, OperationStatus::Failed, &verification.summary)?;
            return Err(OidError::Verification(verification.summary));
        }
        self.transition(&plan.id, OperationStatus::Verified, &verification.summary)?;
        self.completed
            .insert(plan.id.clone(), (skill_id.to_owned(), result.clone()));
        Ok(result)
    }

    /// Persist explicit approval for a recoverable operation.
    ///
    /// # Errors
    ///
    /// Returns an error when approval cannot be persisted.
    pub fn approve(&mut self, operation_id: &OperationId) -> Result<(), OidError> {
        self.approvals.approve(Approval {
            operation_id: operation_id.clone(),
            note: "approved by console".to_owned(),
        })?;
        self.transition(
            operation_id,
            OperationStatus::Approved,
            "approved by console",
        )
    }

    /// Check whether an operation has a durable approval.
    ///
    /// # Errors
    ///
    /// Returns an error when the approval store cannot be read.
    pub fn is_approved(&self, operation_id: &OperationId) -> Result<bool, OidError> {
        self.approvals.is_approved(operation_id)
    }

    /// Roll back a completed operation using its owning skill.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation is unknown or rollback fails.
    pub fn rollback(
        &mut self,
        operation_id: &OperationId,
    ) -> Result<oid_common::RollbackResult, OidError> {
        let (skill_id, execution) =
            self.completed.get(operation_id).cloned().ok_or_else(|| {
                OidError::NotFound(format!("completed operation: {operation_id}"))
            })?;
        let result = self
            .skills
            .get(&skill_id)
            .ok_or_else(|| OidError::NotFound(format!("skill: {skill_id}")))?
            .rollback(&execution);
        match result {
            Ok(result) => {
                self.transition(operation_id, OperationStatus::RolledBack, &result.summary)?;
                self.record_operation_event(operation_id, "rollback", &result.summary)?;
                Ok(result)
            }
            Err(error) => {
                self.transition(
                    operation_id,
                    OperationStatus::RollbackFailed,
                    &error.to_string(),
                )?;
                Err(error)
            }
        }
    }

    /// Roll back an operation reconstructed after a process restart.
    ///
    /// The caller must provide the skill and execution record recovered from
    /// durable evidence; no in-memory execution state is trusted.
    ///
    /// # Errors
    ///
    /// Returns an error when the skill is unavailable, rollback fails, or the
    /// lifecycle/evidence journal cannot be updated.
    pub fn rollback_recovered(
        &mut self,
        operation_id: &OperationId,
        skill_id: &str,
        execution: &ExecutionResult,
    ) -> Result<oid_common::RollbackResult, OidError> {
        let result = self
            .skills
            .get(skill_id)
            .ok_or_else(|| OidError::NotFound(format!("skill: {skill_id}")))?
            .rollback(execution);
        match result {
            Ok(result) => {
                self.transition(operation_id, OperationStatus::RolledBack, &result.summary)?;
                self.record_operation_event(operation_id, "rollback", &result.summary)?;
                Ok(result)
            }
            Err(error) => {
                self.transition(
                    operation_id,
                    OperationStatus::RollbackFailed,
                    &error.to_string(),
                )?;
                Err(error)
            }
        }
    }

    /// Run a dynamic command through its registered skill.
    ///
    /// # Errors
    ///
    /// Returns an error when the command is unavailable or its lifecycle fails.
    pub fn run_dynamic(
        &mut self,
        command: &str,
        request: &SkillRequest,
        approve: bool,
    ) -> Result<ExecutionResult, OidError> {
        let skill_id = self
            .dynamic
            .get(command)
            .cloned()
            .ok_or_else(|| OidError::NotFound(format!("dynamic command: {command}")))?;
        self.run(&skill_id, request, approve)
    }

    /// Return recoverable operation records after a restart.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation journal is unreadable.
    pub fn recoverable(&self) -> Result<Vec<OperationRecord>, OidError> {
        self.journal.recoverable()
    }

    /// Return the latest durable state for one operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation journal is unreadable.
    pub fn latest(&self, operation_id: &OperationId) -> Result<Option<OperationRecord>, OidError> {
        Ok(self
            .journal
            .latest()?
            .into_iter()
            .find(|record| &record.operation_id == operation_id))
    }

    /// Return the append-only evidence history for one operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the evidence store is unreadable.
    pub fn evidence_history(
        &self,
        operation_id: &OperationId,
    ) -> Result<Vec<EvidenceRecord>, OidError> {
        self.evidence.history(operation_id)
    }

    fn transition(
        &self,
        id: &OperationId,
        status: OperationStatus,
        detail: &str,
    ) -> Result<(), OidError> {
        self.journal.append(&OperationRecord {
            operation_id: id.clone(),
            status,
            detail: detail.to_owned(),
        })
    }

    fn record(
        &mut self,
        plan: &OperationPlan,
        category: &str,
        details: &str,
    ) -> Result<(), OidError> {
        self.evidence.append(EvidenceRecord {
            id: oid_common::EvidenceId::new(format!("{}-{category}", plan.id))
                .map_err(|error| OidError::Evidence(error.to_string()))?,
            intent_id: plan.intent.as_ref().map_or_else(
                || {
                    oid_common::IntentId::new(format!("operation-{}", plan.id))
                        .expect("generated intent id")
                },
                |intent| intent.id.clone(),
            ),
            operation_id: plan.id.clone(),
            category: category.to_owned(),
            details: details.to_owned(),
        })
    }

    fn record_operation_event(
        &mut self,
        operation_id: &OperationId,
        category: &str,
        details: &str,
    ) -> Result<(), OidError> {
        self.evidence.append(EvidenceRecord {
            id: oid_common::EvidenceId::new(format!("{operation_id}-{category}"))
                .map_err(|error| OidError::Evidence(error.to_string()))?,
            intent_id: oid_common::IntentId::new(format!("operation-{operation_id}"))
                .map_err(|error| OidError::Evidence(error.to_string()))?,
            operation_id: operation_id.clone(),
            category: category.to_owned(),
            details: details.to_owned(),
        })
    }
}

/// Native command adapter that executes only the command declared at construction.
#[derive(Debug)]
pub struct NativeCommandSkill {
    descriptor: SkillDescriptor,
    command: String,
    args: Vec<String>,
}

impl NativeCommandSkill {
    /// Construct a governed native command adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when the command is empty or is not a PATH command.
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Result<Self, OidError> {
        let command = command.into();
        if command.trim().is_empty() || command.contains('/') {
            return Err(OidError::InvalidInput(
                "native command must be a PATH command".to_owned(),
            ));
        }
        Ok(Self {
            descriptor: SkillDescriptor {
                id: oid_common::SkillId::new(format!("native-{command}"))
                    .map_err(|error| OidError::InvalidInput(error.to_string()))?,
                description: format!("Execute native command {command}"),
                mutates_system: true,
            },
            command,
            args,
        })
    }
}

impl Skill for NativeCommandSkill {
    fn descriptor(&self) -> &SkillDescriptor {
        &self.descriptor
    }
    fn plan(&self, request: &SkillRequest) -> Result<OperationPlan, OidError> {
        let plan = OperationPlan {
            id: request.id.clone(),
            intent: None,
            skill: self.descriptor.id.clone(),
            summary: format!("Execute {}", self.command),
            rationale: "Native command execution requires explicit approval".to_owned(),
            risk: RiskLevel::Medium,
            approval: ApprovalRequirement::User,
            steps: vec![OperationStep {
                order: 1,
                description: format!("Run {}", self.command),
                action: ActionType::NativeCommand {
                    command: self.command.clone(),
                    args: self.args.clone(),
                },
                affects_system_state: true,
            }],
            verification: VerificationPlan {
                checks: vec![VerificationCheck {
                    description: "Native command exits successfully".to_owned(),
                    kind: "exit-success".to_owned(),
                }],
            },
            rollback: None,
        };
        plan.validate()?;
        Ok(plan)
    }
    fn execute(&self, approved: &ApprovedOperationPlan) -> Result<ExecutionResult, OidError> {
        let output = std::process::Command::new(&self.command)
            .args(&self.args)
            .output()
            .map_err(|error| OidError::Execution(error.to_string()))?;
        if !output.status.success() {
            return Err(OidError::Execution(format!(
                "{} exited with {}",
                self.command, output.status
            )));
        }
        Ok(ExecutionResult {
            operation_id: approved.plan.id.clone(),
            summary: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            changed: true,
        })
    }
    fn verify(
        &self,
        execution: &ExecutionResult,
    ) -> Result<oid_common::VerificationResult, OidError> {
        Ok(oid_common::VerificationResult {
            operation_id: execution.operation_id.clone(),
            passed: true,
            summary: "native command exited successfully".to_owned(),
        })
    }
    fn rollback(
        &self,
        execution: &ExecutionResult,
    ) -> Result<oid_common::RollbackResult, OidError> {
        Self::no_rollback(execution)
    }
}

fn encode(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}
fn decode(value: &str) -> Result<String, OidError> {
    let mut output = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(match character {
                't' => '\t',
                'n' => '\n',
                '\\' => '\\',
                other => return Err(OidError::Evidence(format!("unknown escape: \\{other}"))),
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        return Err(OidError::Evidence("trailing escape".to_owned()));
    }
    Ok(output)
}

/// Identifies the coordinator boundary.
#[must_use]
pub const fn boundary_name() -> &'static str {
    "operation-coordinator"
}

#[cfg(test)]
mod tests {
    use super::{OperationCoordinator, OperationJournal};
    use oid_common::OperationId;
    use oid_evidence_engine::InMemoryEvidenceStore;
    use oid_linux_skills::{SkillRequest, SystemHealthSkill};
    use oid_policy_engine::InMemoryApprovalStore;

    #[test]
    fn coordinates_read_only_execution_and_evidence() {
        let path = std::env::temp_dir().join(format!("oid-operations-{}", std::process::id()));
        let mut coordinator = OperationCoordinator::new(
            InMemoryApprovalStore::default(),
            InMemoryEvidenceStore::default(),
            OperationJournal::new(&path),
        );
        coordinator
            .register_skill(Box::new(SystemHealthSkill::default()))
            .expect("register");
        let request = SkillRequest {
            id: OperationId::new("operation-coordinator").expect("id"),
            arguments: String::new(),
        };
        let result = coordinator
            .run("system-health", &request, false)
            .expect("run");
        assert!(!result.changed);
        assert!(coordinator.recoverable().expect("recovery").is_empty());
        std::fs::remove_file(path).expect("cleanup");
    }
}
