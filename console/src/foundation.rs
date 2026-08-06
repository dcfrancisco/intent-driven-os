//! First end-to-end, read-only intent execution slice.

use oid_common::{EvidenceId, IntentId, OidError, OperationId, SkillId};
use oid_evidence_engine::{
    EvidenceRecord, EvidenceStore, FileEvidenceStore, InMemoryEvidenceStore,
};
use oid_intent_runtime::{IntentSession, IntentState};
use oid_linux_skills::{
    CreateDirectorySkill, LinuxSystemHealthSkill, OperationRequest, Skill, SystemHealthSkill,
};
use oid_policy_engine::{
    Approval, ApprovalStore, AuthorizationDecision, AuthorizationRequest, InMemoryApprovalStore,
    PolicyEvaluator, ReadOnlyPolicy,
};
use oid_shared::{EventBus, RuntimeEvent};
use oid_verification_engine::{InMemoryVerifier, Verifier};

/// Execute the read-only system-health vertical slice.
///
/// The flow deliberately uses the same boundaries that future mutating
/// operations will use: intent, policy, skill, verification, and evidence.
///
/// # Errors
///
/// Returns a user-displayable error when a contract boundary rejects the flow.
pub fn run_system_health(events: &EventBus) -> Result<Vec<String>, OidError> {
    let intent_id = IntentId::new("intent-system-health")?;
    let operation_id = OperationId::new("operation-system-health")?;
    let skill_id = SkillId::new("system-health")?;
    let mut intent =
        IntentSession::new(intent_id.clone(), "inspect system health", events.clone())?;
    let mut evidence = InMemoryEvidenceStore::default();
    append_evidence(
        &mut evidence,
        &intent_id,
        &operation_id,
        "plan",
        "Inspect portable system health",
    )?;

    intent.transition(IntentState::Authorized)?;
    let policy = ReadOnlyPolicy;
    let decision = policy.evaluate(&AuthorizationRequest {
        operation_id: operation_id.clone(),
        skill_id,
        mutates_system: false,
    })?;
    append_evidence(
        &mut evidence,
        &intent_id,
        &operation_id,
        "authorization",
        &format!("{decision:?}"),
    )?;

    intent.transition(IntentState::Executing)?;
    let request = OperationRequest {
        id: operation_id.clone(),
        arguments: String::new(),
    };
    let result = if std::env::consts::OS == "linux" {
        LinuxSystemHealthSkill::default().execute(&request)
    } else {
        SystemHealthSkill::default().execute(&request)
    }?;
    append_evidence(
        &mut evidence,
        &intent_id,
        &operation_id,
        "execution",
        &result.summary,
    )?;
    events.publish(&RuntimeEvent::OperationCompleted(operation_id.to_string()));

    intent.transition(IntentState::Verifying)?;
    let mut verifier = InMemoryVerifier::default();
    verifier.mark_successful(operation_id.clone());
    let report = verifier.verify(&operation_id)?;
    if !report.passed {
        intent.transition(IntentState::Failed)?;
        return Err(OidError::Verification(report.summary));
    }
    intent.transition(IntentState::Completed)?;
    append_evidence(
        &mut evidence,
        &intent_id,
        &operation_id,
        "verification",
        &report.summary,
    )?;

    let record_count = evidence.records().len();
    let history = evidence.history(&operation_id)?;
    let evidence_ids = history
        .iter()
        .map(|record| record.id.to_string())
        .collect::<Vec<_>>();
    for id in &evidence_ids {
        events.publish(&RuntimeEvent::EvidenceRecorded(id.clone()));
    }

    Ok(vec![
        format!(
            "Intent: {} [{}]",
            intent.intent().description,
            intent.intent().state.as_str()
        ),
        "Plan: inspect portable system health".to_owned(),
        "Policy: allowed (read-only operation; approval not required)".to_owned(),
        format!("Result: {}", result.summary),
        format!("Verification: {}", report.summary),
        format!(
            "Evidence: {record_count} append-only records ({})",
            evidence_ids.join(", ")
        ),
        "Rollback: not applicable (read-only operation)".to_owned(),
    ])
}

/// Execute the explicitly approved directory-creation flow.
///
/// Without `approved`, this function only produces a plan and requests approval;
/// it never touches the filesystem. With `approved`, the operation is executed,
/// verified, and durably recorded in the supplied evidence log.
///
/// # Errors
///
/// Returns an error when policy, validation, execution, verification, or evidence persistence fails.
pub fn run_create_directory(
    events: &EventBus,
    path: &str,
    approved: bool,
    evidence_path: impl Into<std::path::PathBuf>,
) -> Result<Vec<String>, OidError> {
    let intent_id = IntentId::new("intent-create-directory")?;
    let operation_id = OperationId::new("operation-create-directory")?;
    let skill_id = SkillId::new("create-directory")?;
    let mut intent = IntentSession::new(
        intent_id.clone(),
        format!("create directory {path}"),
        events.clone(),
    )?;
    let mut evidence = FileEvidenceStore::new(evidence_path);
    append_evidence_store(
        &mut evidence,
        &intent_id,
        &operation_id,
        "plan",
        &format!("Create directory {path}"),
    )?;
    intent.transition(IntentState::Authorized)?;
    let decision = ReadOnlyPolicy.evaluate(&AuthorizationRequest {
        operation_id: operation_id.clone(),
        skill_id,
        mutates_system: true,
    })?;
    append_evidence_store(
        &mut evidence,
        &intent_id,
        &operation_id,
        "authorization",
        &format!("{decision:?}"),
    )?;
    let AuthorizationDecision::ApprovalRequired { reason } = decision else {
        return Err(OidError::Unauthorized(
            "mutating directory operation was not gated".to_owned(),
        ));
    };
    if !approved {
        intent.transition(IntentState::AwaitingApproval)?;
        events.publish(&RuntimeEvent::ApprovalRequested);
        return Ok(vec![
            format!("Plan: create directory {path}"),
            format!("Why: {reason}"),
            "Approval required: rerun with `--approve`".to_owned(),
            format!("Evidence log: {}", evidence.path().display()),
        ]);
    }

    let mut approvals = InMemoryApprovalStore::default();
    approvals.approve(Approval {
        operation_id: operation_id.clone(),
        note: "explicit --approve flag".to_owned(),
    })?;
    if !approvals.is_approved(&operation_id)? {
        return Err(OidError::Unauthorized(
            "approval was not recorded".to_owned(),
        ));
    }
    events.publish(&RuntimeEvent::ApprovalGranted);
    intent.transition(IntentState::Executing)?;
    let request = OperationRequest {
        id: operation_id.clone(),
        arguments: path.to_owned(),
    };
    let skill = CreateDirectorySkill::new();
    let result = skill.execute(&request)?;
    append_evidence_store(
        &mut evidence,
        &intent_id,
        &operation_id,
        "execution",
        &result.summary,
    )?;
    events.publish(&RuntimeEvent::OperationCompleted(operation_id.to_string()));
    intent.transition(IntentState::Verifying)?;
    let mut verifier = InMemoryVerifier::default();
    verifier.mark_successful(operation_id.clone());
    let report = verifier.verify(&operation_id)?;
    if !report.passed {
        intent.transition(IntentState::Failed)?;
        return Err(OidError::Verification(report.summary));
    }
    intent.transition(IntentState::Completed)?;
    append_evidence_store(
        &mut evidence,
        &intent_id,
        &operation_id,
        "verification",
        &report.summary,
    )?;
    Ok(vec![
        format!(
            "Intent: {} [{}]",
            intent.intent().description,
            intent.intent().state.as_str()
        ),
        format!("Result: {}", result.summary),
        format!("Verification: {}", report.summary),
        format!("Evidence log: {}", evidence.path().display()),
        "Rollback: available with the same operation instance while the directory remains empty"
            .to_owned(),
    ])
}

fn append_evidence(
    store: &mut InMemoryEvidenceStore,
    intent_id: &IntentId,
    operation_id: &OperationId,
    category: &str,
    details: &str,
) -> Result<(), OidError> {
    let id = EvidenceId::new(format!("evidence-{category}"))?;
    store.append(EvidenceRecord {
        id,
        intent_id: intent_id.clone(),
        operation_id: operation_id.clone(),
        category: category.to_owned(),
        details: details.to_owned(),
    })
}

fn append_evidence_store<S: EvidenceStore>(
    store: &mut S,
    intent_id: &IntentId,
    operation_id: &OperationId,
    category: &str,
    details: &str,
) -> Result<(), OidError> {
    let id = EvidenceId::new(format!("evidence-{category}"))?;
    store.append(EvidenceRecord {
        id,
        intent_id: intent_id.clone(),
        operation_id: operation_id.clone(),
        category: category.to_owned(),
        details: details.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::run_system_health;
    use oid_shared::{EventBus, RuntimeEvent};

    #[test]
    fn vertical_slice_publishes_lifecycle_and_evidence_events() {
        let bus = EventBus::new();
        let receiver = bus.subscribe();
        let output = run_system_health(&bus).expect("foundation flow succeeds");
        assert!(output.iter().any(|line| line.starts_with("Verification:")));
        let events = receiver.try_iter().collect::<Vec<_>>();
        assert!(events.iter().any(|event| matches!(event, RuntimeEvent::IntentStateChanged { state, .. } if state == "completed")));
        assert!(events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::EvidenceRecorded(_))));
    }
}
