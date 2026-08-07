//! Authorization and approval contracts for Open Intelligence Desktop.
//!
//! Policy decisions must remain explicit and independently testable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{OidError, OperationId, SkillId};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Result of evaluating whether an operation may proceed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorizationDecision {
    /// Proceed without interactive approval.
    Allowed,
    /// Pause until the user explicitly approves.
    ApprovalRequired {
        /// Explanation shown to the user.
        reason: String,
    },
    /// Refuse the operation.
    Denied {
        /// Explanation for the denial.
        reason: String,
    },
}

/// Context supplied to a policy evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationRequest {
    /// Operation being evaluated.
    pub operation_id: OperationId,
    /// Skill requested by the operation.
    pub skill_id: SkillId,
    /// Whether the operation mutates system state.
    pub mutates_system: bool,
}

/// A user approval that can be attached to one operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approval {
    /// Operation approved by the user.
    pub operation_id: OperationId,
    /// Human-readable approval note.
    pub note: String,
}

/// Durable state of an approval lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalStatus {
    /// A plan was shown and is awaiting a decision.
    Pending,
    /// The user explicitly approved the plan.
    Approved,
    /// The approval was consumed by execution.
    Consumed,
    /// The approval was rejected or invalidated.
    Rejected,
}

impl ApprovalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Consumed => "consumed",
            Self::Rejected => "rejected",
        }
    }

    fn parse(value: &str) -> Result<Self, OidError> {
        match value {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "consumed" => Ok(Self::Consumed),
            "rejected" => Ok(Self::Rejected),
            _ => Err(OidError::Execution(format!(
                "unknown approval status: {value}"
            ))),
        }
    }
}

/// A journal entry describing the latest known approval state for an operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRecord {
    /// Operation whose approval changed.
    pub operation_id: OperationId,
    /// Current lifecycle state represented by this record.
    pub status: ApprovalStatus,
    /// User or system note associated with the state change.
    pub note: String,
}

/// Policy evaluation boundary.
pub trait PolicyEvaluator: Send + Sync {
    /// Evaluate authorization without executing the operation.
    ///
    /// # Errors
    ///
    /// Returns an error when policy evaluation cannot be completed.
    fn evaluate(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, OidError>;
}

/// Approval persistence/validation boundary.
pub trait ApprovalStore: Send + Sync {
    /// Record explicit user approval.
    ///
    /// # Errors
    ///
    /// Returns an error when approval cannot be recorded.
    fn approve(&mut self, approval: Approval) -> Result<(), OidError>;

    /// Check whether an operation has approval.
    ///
    /// # Errors
    ///
    /// Returns an error when approval state cannot be read.
    fn is_approved(&self, operation_id: &OperationId) -> Result<bool, OidError>;
}

/// Extended approval lifecycle boundary used for recovery-aware execution.
pub trait ApprovalLifecycle: ApprovalStore {
    /// Persist a pending approval request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request cannot be persisted.
    fn request(&mut self, operation_id: OperationId) -> Result<(), OidError>;

    /// Consume an approval when execution begins.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation is not approved or state cannot be persisted.
    fn consume(&mut self, operation_id: &OperationId) -> Result<(), OidError>;

    /// Return operations that were pending or approved when the process stopped.
    ///
    /// # Errors
    ///
    /// Returns an error when the approval journal cannot be read.
    fn recoverable(&self) -> Result<Vec<ApprovalRecord>, OidError>;
}

/// Deterministic policy that allows read-only skills and requires approval for mutations.
#[derive(Clone, Debug, Default)]
pub struct ReadOnlyPolicy;

impl PolicyEvaluator for ReadOnlyPolicy {
    fn evaluate(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, OidError> {
        Ok(if request.mutates_system {
            AuthorizationDecision::ApprovalRequired {
                reason: format!("skill {} can change system state", request.skill_id),
            }
        } else {
            AuthorizationDecision::Allowed
        })
    }
}

/// In-memory approval store for deterministic tests and the reference client.
#[derive(Clone, Debug, Default)]
pub struct InMemoryApprovalStore {
    approvals: std::collections::BTreeSet<OperationId>,
}

impl ApprovalStore for InMemoryApprovalStore {
    fn approve(&mut self, approval: Approval) -> Result<(), OidError> {
        self.approvals.insert(approval.operation_id);
        Ok(())
    }

    fn is_approved(&self, operation_id: &OperationId) -> Result<bool, OidError> {
        Ok(self.approvals.contains(operation_id))
    }
}

impl ApprovalLifecycle for InMemoryApprovalStore {
    fn request(&mut self, operation_id: OperationId) -> Result<(), OidError> {
        self.approvals.remove(&operation_id);
        Ok(())
    }

    fn consume(&mut self, operation_id: &OperationId) -> Result<(), OidError> {
        if self.approvals.remove(operation_id) {
            Ok(())
        } else {
            Err(OidError::ApprovalRequired(format!(
                "operation {operation_id} has no approval"
            )))
        }
    }

    fn recoverable(&self) -> Result<Vec<ApprovalRecord>, OidError> {
        Ok(self
            .approvals
            .iter()
            .cloned()
            .map(|operation_id| ApprovalRecord {
                operation_id,
                status: ApprovalStatus::Approved,
                note: "in-memory approval".to_owned(),
            })
            .collect())
    }
}

/// Durable append-only approval journal for crash recovery.
#[derive(Clone, Debug)]
pub struct FileApprovalStore {
    path: PathBuf,
}

impl FileApprovalStore {
    /// Create a journal at the supplied path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Return the configured journal path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn append_record(&self, record: &ApprovalRecord) -> Result<(), OidError> {
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
            encode(&record.note)
        )
        .map_err(|error| OidError::Evidence(format!("append {}: {error}", self.path.display())))
    }

    fn records(&self) -> Result<Vec<ApprovalRecord>, OidError> {
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
        BufReader::new(file)
            .lines()
            .map(|line| {
                let line = line.map_err(|error| OidError::Evidence(error.to_string()))?;
                let fields: Vec<_> = line.split('\t').map(decode).collect::<Result<_, _>>()?;
                if fields.len() != 3 {
                    return Err(OidError::Evidence("malformed approval record".to_owned()));
                }
                Ok(ApprovalRecord {
                    operation_id: OperationId::new(fields[0].clone())
                        .map_err(|error| OidError::Evidence(error.to_string()))?,
                    status: ApprovalStatus::parse(&fields[1])?,
                    note: fields[2].clone(),
                })
            })
            .collect()
    }

    fn latest(&self) -> Result<BTreeMap<OperationId, ApprovalRecord>, OidError> {
        Ok(self
            .records()?
            .into_iter()
            .map(|record| (record.operation_id.clone(), record))
            .collect())
    }
}

impl ApprovalStore for FileApprovalStore {
    fn approve(&mut self, approval: Approval) -> Result<(), OidError> {
        self.append_record(&ApprovalRecord {
            operation_id: approval.operation_id,
            status: ApprovalStatus::Approved,
            note: approval.note,
        })
    }

    fn is_approved(&self, operation_id: &OperationId) -> Result<bool, OidError> {
        Ok(self
            .latest()?
            .get(operation_id)
            .is_some_and(|record| record.status == ApprovalStatus::Approved))
    }
}

impl ApprovalLifecycle for FileApprovalStore {
    fn request(&mut self, operation_id: OperationId) -> Result<(), OidError> {
        self.append_record(&ApprovalRecord {
            operation_id,
            status: ApprovalStatus::Pending,
            note: "awaiting user approval".to_owned(),
        })
    }

    fn consume(&mut self, operation_id: &OperationId) -> Result<(), OidError> {
        if !self.is_approved(operation_id)? {
            return Err(OidError::ApprovalRequired(format!(
                "operation {operation_id} has no approval"
            )));
        }
        self.append_record(&ApprovalRecord {
            operation_id: operation_id.clone(),
            status: ApprovalStatus::Consumed,
            note: "approval consumed by execution".to_owned(),
        })
    }

    fn recoverable(&self) -> Result<Vec<ApprovalRecord>, OidError> {
        Ok(self
            .latest()?
            .into_values()
            .filter(|record| {
                matches!(
                    record.status,
                    ApprovalStatus::Pending | ApprovalStatus::Approved
                )
            })
            .collect())
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

/// Identifies the policy boundary.
///
/// ```
/// assert_eq!(oid_policy_engine::boundary_name(), "policy-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "policy-engine"
}

#[cfg(test)]
mod tests {
    use super::{
        Approval, ApprovalLifecycle, ApprovalStore, AuthorizationDecision, AuthorizationRequest,
        FileApprovalStore, PolicyEvaluator, ReadOnlyPolicy,
    };
    use oid_common::{OperationId, SkillId};

    fn request(mutates_system: bool) -> AuthorizationRequest {
        AuthorizationRequest {
            operation_id: OperationId::new("operation-1").expect("valid id"),
            skill_id: SkillId::new("skill-1").expect("valid id"),
            mutates_system,
        }
    }

    #[test]
    fn read_only_operations_are_allowed() {
        assert_eq!(
            ReadOnlyPolicy
                .evaluate(&request(false))
                .expect("policy works"),
            AuthorizationDecision::Allowed
        );
    }

    #[test]
    fn mutating_operations_require_approval() {
        assert!(matches!(
            ReadOnlyPolicy
                .evaluate(&request(true))
                .expect("policy works"),
            AuthorizationDecision::ApprovalRequired { .. }
        ));
    }

    #[test]
    fn durable_approval_journal_recovers_pending_and_approved_operations() {
        let path = std::env::temp_dir().join(format!("oid-approval-{}", std::process::id()));
        let operation_id = OperationId::new("operation-durable").expect("operation id");
        let mut store = FileApprovalStore::new(&path);
        store.request(operation_id.clone()).expect("request");
        assert_eq!(store.recoverable().expect("recover").len(), 1);
        store
            .approve(Approval {
                operation_id: operation_id.clone(),
                note: "approved in test".to_owned(),
            })
            .expect("approve");
        assert!(store.is_approved(&operation_id).expect("approval lookup"));
        store.consume(&operation_id).expect("consume");
        assert!(store.recoverable().expect("recover").is_empty());
        std::fs::remove_file(path).expect("cleanup");
    }
}
