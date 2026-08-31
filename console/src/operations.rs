//! Console-facing governed operation workflows.

use oid_common::{ExecutionResult, OidError, OperationId};
use oid_evidence_engine::FileEvidenceStore;
use oid_linux_skills::{
    CreateDirectorySkill, LinuxSystemHealthSkill, SkillRequest, SystemHealthSkill,
};
use oid_operation_coordinator::{
    OperationCoordinator, OperationJournal, OperationRecord, OperationStatus,
};
use oid_policy_engine::FileApprovalStore;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persistent coordinator owned by one interactive console session.
pub struct ConsoleOperations {
    coordinator: OperationCoordinator<FileApprovalStore, FileEvidenceStore>,
    pending_path: PathBuf,
}

impl ConsoleOperations {
    /// Create a coordinator using the user's temporary OID state directory.
    #[must_use]
    pub fn new() -> Self {
        let root = std::env::var_os("OID_STATE_DIR")
            .map_or_else(|| std::env::temp_dir().join("oid-console"), PathBuf::from);
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
        Self {
            coordinator,
            pending_path: root.join("pending.log"),
        }
    }

    /// Render recoverable operation records after startup.
    pub fn recover(&self) -> Result<Vec<String>, OidError> {
        let records = self.coordinator.recoverable()?;
        let pending = self.pending_entries()?;
        if records.is_empty() && pending.is_empty() {
            return Ok(vec!["No recoverable operations.".to_owned()]);
        }
        let mut output = vec![format!(
            "OID detected {} interrupted operation(s).",
            records.len() + pending.len()
        )];
        output.extend(records.iter().map(render_record));
        output.extend(pending.iter().map(|(id, path)| {
            format!("{id}  Create directory {path}\n        State: AWAITING_APPROVAL")
        }));
        output.push(
            "Use :operations inspect <id>, :operations resume <id>, or :operations rollback <id>."
                .to_owned(),
        );
        Ok(output)
    }

    /// Start or execute the deterministic directory intent.
    pub fn intent_create_directory(
        &mut self,
        path: &str,
        approve: bool,
    ) -> Result<Vec<String>, OidError> {
        let operation_id = new_operation_id()?;
        let request = SkillRequest {
            id: operation_id.clone(),
            arguments: path.to_owned(),
        };
        let plan = self.coordinator.preview("create-directory", &request)?;
        if !approve {
            self.remember(&operation_id, path)?;
            let mut output = vec![format!("Intent understood:\nCreate directory {}", path)];
            output.extend(crate::foundation::render_plan(&plan, "Awaiting approval"));
            output.push(format!("Approve with :operations approve {operation_id}"));
            return Ok(output);
        }
        self.remember(&operation_id, path)?;
        self.execute_pending(&operation_id, path)
    }

    /// Approve and execute a pending operation.
    pub fn approve(&mut self, raw_id: &str) -> Result<Vec<String>, OidError> {
        let id = OperationId::new(raw_id.trim())?;
        let path = self
            .pending(&id)?
            .ok_or_else(|| OidError::NotFound(format!("pending operation: {id}")))?;
        self.execute_pending(&id, &path)
    }

    /// Resume an operation explicitly after an interruption.
    pub fn resume(&mut self, raw_id: &str) -> Result<Vec<String>, OidError> {
        self.approve(raw_id)
    }

    /// Inspect the durable plan, lifecycle state, and evidence for one operation.
    pub fn inspect(&self, raw_id: &str) -> Result<Vec<String>, OidError> {
        let id = OperationId::new(raw_id.trim())?;
        let record = self
            .coordinator
            .latest(&id)?
            .ok_or_else(|| OidError::NotFound(format!("operation: {id}")))?;
        let record = self.coordinator.recovery_record(&id)?.unwrap_or(record);
        let path = self
            .pending(&id)?
            .ok_or_else(|| OidError::NotFound(format!("operation input: {id}")))?;
        let history = self.coordinator.evidence_history(&id)?;
        let plan_details = history
            .iter()
            .find(|evidence| evidence.category == "plan")
            .map_or_else(
                || "Plan unavailable".to_owned(),
                |evidence| evidence.details.clone(),
            );
        let mut output = vec![
            format!("Intent\n  \"create a directory {}\"", path),
            String::new(),
        ];
        output.extend([
            "Plan".to_owned(),
            format!("  {plan_details}"),
            format!("State\n  {:?}", record.status).to_uppercase(),
        ]);
        output.extend([String::new(), "Lifecycle".to_owned()]);
        for evidence in history {
            output.push(format!(
                "{}\n  {}",
                title(&evidence.category),
                evidence.details
            ));
        }
        output.push(format!("State\n  {:?}", record.status).to_uppercase());
        Ok(output)
    }

    /// Roll back a completed or interrupted directory operation from durable evidence.
    pub fn rollback(&mut self, raw_id: &str) -> Result<Vec<String>, OidError> {
        let id = OperationId::new(raw_id.trim())?;
        let path = self
            .pending(&id)?
            .ok_or_else(|| OidError::NotFound(format!("operation input: {id}")))?;
        let execution = self
            .coordinator
            .evidence_history(&id)?
            .into_iter()
            .find(|record| record.category == "execution")
            .map(|record| ExecutionResult {
                operation_id: id.clone(),
                summary: record.details,
                changed: true,
            })
            .ok_or_else(|| OidError::NotFound(format!("execution evidence: {id}")))?;
        let result = self
            .coordinator
            .rollback_recovered(&id, "create-directory", &execution)?;
        Ok(vec![
            format!("Rollback\n  {}\nState\n  RolledBack", result.summary),
            format!("Target\n  {path}"),
        ])
    }

    /// Compatibility entry point for the original explicit directory command.
    pub fn create_directory(&mut self, path: &str, approve: bool) -> Result<Vec<String>, OidError> {
        self.intent_create_directory(path, approve)
    }

    fn execute_pending(&mut self, id: &OperationId, path: &str) -> Result<Vec<String>, OidError> {
        let request = SkillRequest {
            id: id.clone(),
            arguments: path.to_owned(),
        };
        let result = self.coordinator.run("create-directory", &request, true)?;
        Ok(vec![
            format!("Executing operation {id}..."),
            "✓ Path validated".to_owned(),
            "✓ Directory created".to_owned(),
            "✓ Verification passed".to_owned(),
            "✓ Evidence persisted".to_owned(),
            String::new(),
            format!("Operation completed successfully.\n{}", result.summary),
            format!("Inspect with :operations inspect {id}"),
        ])
    }

    fn remember(&self, id: &OperationId, path: &str) -> Result<(), OidError> {
        if path.contains('\t') || path.contains('\n') {
            return Err(OidError::InvalidInput(
                "directory path contains a forbidden character".to_owned(),
            ));
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.pending_path)
            .map_err(|error| OidError::Evidence(error.to_string()))?;
        writeln!(file, "{id}\t{path}")
            .and_then(|()| file.sync_data())
            .map_err(|error| OidError::Evidence(error.to_string()))
    }

    fn pending(&self, id: &OperationId) -> Result<Option<String>, OidError> {
        Ok(self
            .pending_entries()?
            .into_iter()
            .rev()
            .find(|(candidate, _)| candidate == id)
            .map(|(_, path)| path))
    }

    fn pending_entries(&self) -> Result<Vec<(OperationId, String)>, OidError> {
        let contents = match std::fs::read_to_string(&self.pending_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(OidError::Evidence(error.to_string())),
        };
        contents
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(id, path)| {
                OperationId::new(id.to_owned())
                    .map(|id| (id, path.to_owned()))
                    .map_err(|error| OidError::Evidence(error.to_string()))
            })
            .collect()
    }
}

fn new_operation_id() -> Result<OperationId, OidError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| OidError::Execution(error.to_string()))?
        .as_nanos();
    OperationId::new(format!("op-create-directory-{nanos}"))
}

fn title(category: &str) -> String {
    let mut chars = category.chars();
    chars.next().map_or_else(
        || category.to_owned(),
        |first| first.to_uppercase().collect::<String>() + chars.as_str(),
    )
}

fn render_record(record: &OperationRecord) -> String {
    let state = if matches!(
        record.status,
        OperationStatus::Executing | OperationStatus::Executed
    ) {
        "EXECUTION_INTERRUPTED"
    } else {
        &format!("{:?}", record.status).to_uppercase()
    };
    format!(
        "{}  {}\n        State: {}",
        record.operation_id, record.detail, state
    )
}

#[cfg(test)]
mod tests {
    use super::ConsoleOperations;

    #[test]
    fn directory_intent_survives_restart_and_can_be_inspected_and_rolled_back() {
        let root = std::env::temp_dir().join(format!("oid-e2e-{}", std::process::id()));
        let path = root.join("demo");
        let path_text = path.to_string_lossy().to_string();
        let mut first = ConsoleOperations::with_root(&root);
        let preview = first
            .intent_create_directory(&path_text, false)
            .expect("preview");
        let id = preview
            .iter()
            .find_map(|line| line.strip_prefix("Approve with :operations approve "))
            .expect("operation id")
            .to_owned();

        let mut restarted = ConsoleOperations::with_root(&root);
        let recovered = restarted.recover().expect("recover");
        assert!(recovered.iter().any(|line| line.contains(&id)));
        restarted.approve(&id).expect("execute after restart");
        assert!(path.is_dir());
        let inspection = restarted.inspect(&id).expect("inspect");
        assert!(inspection.iter().any(|line| line.contains("VERIFIED")));
        assert!(inspection.iter().any(|line| line.contains("approval")));

        let mut restarted_again = ConsoleOperations::with_root(&root);
        restarted_again
            .rollback(&id)
            .expect("rollback after restart");
        assert!(!path.exists());
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
