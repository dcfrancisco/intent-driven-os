#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert_ne!(oid_intent_runtime::boundary_name(), "");
}

#[test]
fn diagnostic_proposal_is_typed_and_requires_verification() {
    let proposal = oid_intent_runtime::IntentProposal {
        id: oid_common::IntentId::new("intent-diagnose-slow-system").expect("id"),
        requester: "user:alice".to_owned(),
        desired_outcome: "identify evidence-supported causes of slow performance".to_owned(),
        targets: vec!["local-system".to_owned()],
        required_capabilities: vec!["system-health.read".to_owned(), "processes.read".to_owned()],
        risk: oid_intent_runtime::IntentRisk::ReadOnly,
        required_authority: "user-session-observation".to_owned(),
        proposed_steps: vec!["collect CPU, memory, storage, and process telemetry".to_owned()],
        verification_criteria: vec!["return timestamped measurements with source labels".to_owned()],
        execution_constraints: vec!["no mutation; user scope; bounded duration".to_owned()],
    };

    assert!(proposal.validate().is_ok());
}

#[test]
fn incomplete_proposal_is_rejected_before_planning() {
    let proposal = oid_intent_runtime::IntentProposal {
        id: oid_common::IntentId::new("intent-incomplete").expect("id"),
        requester: "user:alice".to_owned(),
        desired_outcome: "inspect".to_owned(),
        targets: vec!["local-system".to_owned()],
        required_capabilities: vec!["system-health.read".to_owned()],
        risk: oid_intent_runtime::IntentRisk::ReadOnly,
        required_authority: "user-session-observation".to_owned(),
        proposed_steps: vec!["inspect".to_owned()],
        verification_criteria: Vec::new(),
        execution_constraints: vec!["bounded".to_owned()],
    };

    assert!(proposal.validate().is_err());
}
