//! First end-to-end, read-only intent execution slice.

use oid_common::{
    ApprovedOperationPlan, EvidenceId, IntentContext, IntentId, OidError, OperationId,
    OperationPlan,
};
use oid_evidence_engine::{
    EvidenceRecord, EvidenceStore, FileEvidenceStore, InMemoryEvidenceStore,
};
use oid_intent_runtime::{IntentSession, IntentState};
use oid_linux_skills::{
    CreateDirectorySkill, LinuxSystemHealthSkill, Skill, SkillRequest, SystemHealthSkill,
};
use oid_policy_engine::{
    Approval, ApprovalStore, AuthorizationDecision, AuthorizationRequest, InMemoryApprovalStore,
    PolicyEvaluator, ReadOnlyPolicy,
};
use oid_shared::{EventBus, RuntimeEvent};

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
    let mut intent =
        IntentSession::new(intent_id.clone(), "inspect system health", events.clone())?;
    let skill: Box<dyn Skill> = if std::env::consts::OS == "linux" {
        Box::new(LinuxSystemHealthSkill::default())
    } else {
        Box::new(SystemHealthSkill::default())
    };
    let request = SkillRequest {
        id: operation_id.clone(),
        arguments: String::new(),
    };
    let mut plan = skill.plan(&request)?;
    plan.intent = Some(IntentContext {
        id: intent_id.clone(),
        description: intent.intent().description.clone(),
    });
    plan.validate()?;
    let mut evidence = InMemoryEvidenceStore::default();
    append_evidence(
        &mut evidence,
        &intent_id,
        &operation_id,
        "plan",
        &plan.to_json()?,
    )?;

    intent.transition(IntentState::Authorized)?;
    let policy = ReadOnlyPolicy;
    let decision = policy.evaluate(&AuthorizationRequest {
        operation_id: operation_id.clone(),
        skill_id: plan.skill.clone(),
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
    let plan_output = render_plan(&plan, "Executing");
    let approved = ApprovedOperationPlan::new(plan, "policy:none")?;
    let result = skill.execute(&approved)?;
    append_evidence(
        &mut evidence,
        &intent_id,
        &operation_id,
        "execution",
        &result.summary,
    )?;
    events.publish(&RuntimeEvent::OperationCompleted(operation_id.to_string()));

    intent.transition(IntentState::Verifying)?;
    let report = skill.verify(&result)?;
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

    let mut output = vec![format!(
        "Intent: {} [{}]",
        intent.intent().description,
        intent.intent().state.as_str()
    )];
    output.extend(plan_output);
    output.extend([
        "Policy: allowed (read-only operation; approval not required)".to_owned(),
        format!("Result: {}", result.summary),
        format!("Verification: {}", report.summary),
        format!(
            "Evidence: {record_count} append-only records ({})",
            evidence_ids.join(", ")
        ),
        "Rollback: not applicable (read-only operation)".to_owned(),
    ]);
    Ok(output)
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
#[allow(clippy::too_many_lines)]
pub fn run_create_directory(
    events: &EventBus,
    path: &str,
    approved: bool,
    evidence_path: impl Into<std::path::PathBuf>,
) -> Result<Vec<String>, OidError> {
    let intent_id = IntentId::new("intent-create-directory")?;
    let operation_id = OperationId::new("operation-create-directory")?;
    let mut intent = IntentSession::new(
        intent_id.clone(),
        format!("create directory {path}"),
        events.clone(),
    )?;
    let skill = CreateDirectorySkill::new();
    let request = SkillRequest {
        id: operation_id.clone(),
        arguments: path.to_owned(),
    };
    let mut plan = skill.plan(&request)?;
    plan.intent = Some(IntentContext {
        id: intent_id.clone(),
        description: intent.intent().description.clone(),
    });
    plan.validate()?;
    let mut evidence = FileEvidenceStore::new(evidence_path);
    append_evidence_store(
        &mut evidence,
        &intent_id,
        &operation_id,
        "plan",
        &plan.to_json()?,
    )?;
    let decision = ReadOnlyPolicy.evaluate(&AuthorizationRequest {
        operation_id: operation_id.clone(),
        skill_id: plan.skill.clone(),
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
    intent.transition(IntentState::Authorized)?;
    if !approved {
        intent.transition(IntentState::AwaitingApproval)?;
        events.publish(&RuntimeEvent::ApprovalRequested);
        let mut output = render_plan(&plan, "Awaiting approval");
        output.extend([
            format!("Why: {reason}"),
            "Run again with:".to_owned(),
            format!("create directory {path} --approve"),
            format!("Evidence log: {}", evidence.path().display()),
        ]);
        return Ok(output);
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
    let plan_output = render_plan(&plan, "Executing");
    let approved_plan = ApprovedOperationPlan::new(plan, "user (--approve)")?;
    let result = skill.execute(&approved_plan)?;
    append_evidence_store(
        &mut evidence,
        &intent_id,
        &operation_id,
        "execution",
        &result.summary,
    )?;
    events.publish(&RuntimeEvent::OperationCompleted(operation_id.to_string()));
    intent.transition(IntentState::Verifying)?;
    let report = skill.verify(&result)?;
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
    let mut output = vec![format!(
        "Intent: {} [{}]",
        intent.intent().description,
        intent.intent().state.as_str()
    )];
    output.extend(plan_output);
    output.extend([
        format!("Result: {}", result.summary),
        format!("Verification: {}", report.summary),
        format!("Evidence log: {}", evidence.path().display()),
        "Rollback: available with the same operation instance while the directory remains empty"
            .to_owned(),
    ]);
    Ok(output)
}

/// Render the canonical plan in the standard terminal format.
#[must_use]
pub fn render_plan(plan: &OperationPlan, status: &str) -> Vec<String> {
    let mut lines = vec![
        "Operation Plan".to_owned(),
        "──────────────────────────".to_owned(),
        String::new(),
        format!("Summary\n{}", plan.summary),
        format!("Rationale\n{}", plan.rationale),
        format!("Risk\n{}", plan.risk.as_str()),
        format!("Approval\n{}", plan.approval.as_str()),
        "Steps".to_owned(),
    ];
    lines.extend(
        plan.steps
            .iter()
            .map(|step| format!("{}. {}", step.order, step.description)),
    );
    lines.push("Verification".to_owned());
    lines.extend(
        plan.verification
            .checks
            .iter()
            .map(|check| format!("- {}", check.description)),
    );
    lines.push("Rollback".to_owned());
    lines.push(plan.rollback.as_ref().map_or_else(
        || "Not applicable".to_owned(),
        |rollback| rollback.description.clone(),
    ));
    lines.push(format!("Status\n{status}"));
    lines
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
