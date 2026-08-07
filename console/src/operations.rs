//! Console-facing operation coordinator service.

use oid_common::{OidError, OperationId};
use oid_evidence_engine::FileEvidenceStore;
use oid_linux_skills::{
    CreateDirectorySkill, LinuxSystemHealthSkill, SkillRequest, SystemHealthSkill,
};
use oid_operation_coordinator::{OperationCoordinator, OperationJournal, OperationRecord};
use oid_policy_engine::FileApprovalStore;
use std::path::PathBuf;

/// Persistent coordinator owned by one interactive console session.
pub struct ConsoleOperations {
    coordinator: OperationCoordinator<FileApprovalStore, FileEvidenceStore>,
}

impl ConsoleOperations {
    /// Create a coordinator using the user's temporary OID state directory.
    #[must_use]
    pub fn new() -> Self {
        let root = std::env::temp_dir().join("oid-console");
        Self::with_root(root)
    }

    /// Create a coordinator rooted at an explicit directory.
    #[must_use]
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let _ = std::fs::create_dir_all(&root);
        let mut coordinator = OperationCoordinator::new(
            FileApprovalStore::new(root.join("approvals.log")),
            FileEvidenceStore::new(root.join("evidence.log")),
            OperationJournal::new(root.join("operations.log")),
        );
        let health: Box<dyn oid_linux_skills::Skill> = if std::env::consts::OS == "linux" {
            Box::new(LinuxSystemHealthSkill::default())
        } else {
            Box::new(SystemHealthSkill::default())
        };
        coordinator
            .register_skill(health)
            .expect("static health skill is unique");
        coordinator
            .register_skill(Box::new(CreateDirectorySkill::default()))
            .expect("static directory skill is unique");
        Self { coordinator }
    }

    /// Render recoverable operation records.
    pub fn recover(&self) -> Result<Vec<String>, OidError> {
        let records = self.coordinator.recoverable()?;
        if records.is_empty() {
            return Ok(vec!["No recoverable operations.".to_owned()]);
        }
        Ok(records.iter().map(render_record).collect())
    }

    /// Persist explicit approval for an operation ID.
    pub fn approve(&mut self, raw_id: &str) -> Result<Vec<String>, OidError> {
        let id = OperationId::new(raw_id.trim())?;
        self.coordinator.approve(&id)?;
        Ok(vec![format!(
            "Approved operation {id}. Re-run the originating command to execute it."
        )])
    }

    /// Roll back a completed operation in this coordinator session.
    pub fn rollback(&mut self, raw_id: &str) -> Result<Vec<String>, OidError> {
        let id = OperationId::new(raw_id.trim())?;
        let result = self.coordinator.rollback(&id)?;
        Ok(vec![format!("Rollback: {}", result.summary)])
    }

    /// Preview or execute the governed directory operation.
    pub fn create_directory(&mut self, path: &str, approve: bool) -> Result<Vec<String>, OidError> {
        let request = SkillRequest {
            id: OperationId::new("operation-create-directory")?,
            arguments: path.to_owned(),
        };
        let plan = self.coordinator.preview("create-directory", &request)?;
        if !approve && !self.coordinator.is_approved(&request.id)? {
            return Ok(crate::foundation::render_plan(&plan, "Awaiting approval"));
        }
        let result = self
            .coordinator
            .run("create-directory", &request, approve)?;
        Ok(vec![
            format!("Result: {}", result.summary),
            "Verification completed.".to_owned(),
        ])
    }
}

fn render_record(record: &OperationRecord) -> String {
    format!(
        "{} — {:?}: {}",
        record.operation_id, record.status, record.detail
    )
}
