//! Evidence and audit contracts for Open Intelligence Desktop.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_common::{EvidenceId, IntentId, OidError, OperationId};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// An immutable record of a plan, approval, execution, or verification event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRecord {
    /// Stable record identity.
    pub id: EvidenceId,
    /// Related intent.
    pub intent_id: IntentId,
    /// Related operation.
    pub operation_id: OperationId,
    /// Event category.
    pub category: String,
    /// Human-readable details.
    pub details: String,
}

/// Durable, privacy-preserving evidence for one model inference request.
///
/// This record intentionally stores request metadata and outcomes, but never
/// raw prompts, generated text, bearer credentials, or provider secrets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceEvidenceRecord {
    /// Stable evidence identity.
    pub id: EvidenceId,
    /// OID interactive session identity.
    pub session_id: String,
    /// OID request identity.
    pub request_id: String,
    /// Marina generation identity, when assigned.
    pub generation_id: Option<String>,
    /// Model identity.
    pub model: String,
    /// Backend identity.
    pub backend: String,
    /// Non-sensitive request parameters, serialized by the caller.
    pub parameters: String,
    /// Start timestamp supplied by the caller.
    pub started_at: String,
    /// Terminal timestamp, absent for an in-progress record.
    pub completed_at: Option<String>,
    /// Lifecycle outcome, such as started, completed, cancelled, or failed.
    pub outcome: String,
    /// Cancellation or timeout reason, when applicable.
    pub termination_reason: Option<String>,
    /// Structured client/runtime error classification, when applicable.
    pub error_class: Option<String>,
    /// Verification result supplied by the OID response path.
    pub verification: String,
    /// Related evidence identifiers, serialized as a comma-separated list.
    pub links: Vec<String>,
    /// Metrics rendered as `key=value`; unavailable values must be `unknown`.
    pub metrics: String,
}

/// Evidence persistence boundary.
pub trait EvidenceStore: Send + Sync {
    /// Append a record without mutating existing history.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be persisted.
    fn append(&mut self, record: EvidenceRecord) -> Result<(), OidError>;

    /// Return records for an operation in append order.
    ///
    /// # Errors
    ///
    /// Returns an error when history cannot be read.
    fn history(&self, operation_id: &OperationId) -> Result<Vec<EvidenceRecord>, OidError>;

    /// Append inference evidence to this same evidence store.
    ///
    /// # Errors
    ///
    /// Returns an error when the store cannot persist inference evidence.
    fn append_inference(&mut self, _record: InferenceEvidenceRecord) -> Result<(), OidError> {
        Err(OidError::Evidence(
            "inference evidence is unsupported by this evidence store".to_owned(),
        ))
    }

    /// Return inference evidence for one OID request identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when the store cannot read inference evidence.
    fn inference_history(
        &self,
        _request_id: &str,
    ) -> Result<Vec<InferenceEvidenceRecord>, OidError> {
        Err(OidError::Evidence(
            "inference evidence is unsupported by this evidence store".to_owned(),
        ))
    }
}

/// Append-only in-memory evidence store for the reference client and tests.
#[derive(Clone, Debug, Default)]
pub struct InMemoryEvidenceStore {
    records: Vec<EvidenceRecord>,
}

impl InMemoryEvidenceStore {
    /// Return all records in append order.
    #[must_use]
    pub fn records(&self) -> &[EvidenceRecord] {
        &self.records
    }

    /// Group records by operation for inspection and test assertions.
    #[must_use]
    pub fn by_operation(&self) -> BTreeMap<OperationId, Vec<EvidenceRecord>> {
        let mut grouped = BTreeMap::new();
        for record in &self.records {
            grouped
                .entry(record.operation_id.clone())
                .or_insert_with(Vec::new)
                .push(record.clone());
        }
        grouped
    }
}

impl EvidenceStore for InMemoryEvidenceStore {
    fn append(&mut self, record: EvidenceRecord) -> Result<(), OidError> {
        self.records.push(record);
        Ok(())
    }

    fn history(&self, operation_id: &OperationId) -> Result<Vec<EvidenceRecord>, OidError> {
        Ok(self
            .records
            .iter()
            .filter(|record| &record.operation_id == operation_id)
            .cloned()
            .collect())
    }

    fn append_inference(&mut self, record: InferenceEvidenceRecord) -> Result<(), OidError> {
        // In-memory evidence is only used by the foundation demonstration. It
        // intentionally does not pretend to be durable, but it still offers
        // the same append/idempotency contract for tests.
        let _ = record;
        Err(OidError::Evidence(
            "inference evidence requires a durable file evidence store".to_owned(),
        ))
    }
}

/// Durable append-only evidence store using a local tab-separated log file.
#[derive(Clone, Debug)]
pub struct FileEvidenceStore {
    path: PathBuf,
}

impl FileEvidenceStore {
    /// Create a store for a file. The file is created on the first append.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Return the configured log path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl EvidenceStore for FileEvidenceStore {
    fn append(&mut self, record: EvidenceRecord) -> Result<(), OidError> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| {
                OidError::Evidence(format!("open {}: {error}", self.path.display()))
            })?;
        writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}",
            encode(record.id.as_str()),
            encode(record.intent_id.as_str()),
            encode(record.operation_id.as_str()),
            encode(&record.category),
            encode(&record.details)
        )
        .and_then(|()| file.sync_data())
        .map_err(|error| OidError::Evidence(format!("append {}: {error}", self.path.display())))
    }

    fn history(&self, operation_id: &OperationId) -> Result<Vec<EvidenceRecord>, OidError> {
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
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| {
                OidError::Evidence(format!("read {}: {error}", self.path.display()))
            })?;
            let fields = line
                .split('\t')
                .map(decode)
                .collect::<Result<Vec<_>, _>>()?;
            if fields.len() != 5 {
                return Err(OidError::Evidence("malformed evidence record".to_owned()));
            }
            let record = EvidenceRecord {
                id: EvidenceId::new(fields[0].clone())
                    .map_err(|error| OidError::Evidence(error.to_string()))?,
                intent_id: IntentId::new(fields[1].clone())
                    .map_err(|error| OidError::Evidence(error.to_string()))?,
                operation_id: OperationId::new(fields[2].clone())
                    .map_err(|error| OidError::Evidence(error.to_string()))?,
                category: fields[3].clone(),
                details: fields[4].clone(),
            };
            if &record.operation_id == operation_id {
                records.push(record);
            }
        }
        Ok(records)
    }

    fn append_inference(&mut self, record: InferenceEvidenceRecord) -> Result<(), OidError> {
        let existing = self.inference_history(&record.request_id)?;
        if let Some(previous) = existing.iter().find(|previous| previous.id == record.id) {
            if previous == &record {
                return Ok(());
            }
            return Err(OidError::Evidence(format!(
                "conflicting duplicate inference evidence id: {}",
                record.id
            )));
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| {
                OidError::Evidence(format!("open {}: {error}", self.path.display()))
            })?;
        let fields = [
            "inference".to_owned(),
            record.id.to_string(),
            record.session_id,
            record.request_id,
            record.generation_id.unwrap_or_default(),
            record.model,
            record.backend,
            record.parameters,
            record.started_at,
            record.completed_at.unwrap_or_default(),
            record.outcome,
            record.termination_reason.unwrap_or_default(),
            record.error_class.unwrap_or_default(),
            record.verification,
            record.links.join(","),
            record.metrics,
        ];
        let line = fields
            .iter()
            .map(|field| encode(field))
            .collect::<Vec<_>>()
            .join("\t");
        writeln!(file, "{line}")
            .and_then(|()| file.sync_data())
            .map_err(|error| OidError::Evidence(format!("append {}: {error}", self.path.display())))
    }

    fn inference_history(
        &self,
        request_id: &str,
    ) -> Result<Vec<InferenceEvidenceRecord>, OidError> {
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
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| {
                OidError::Evidence(format!("read {}: {error}", self.path.display()))
            })?;
            let fields = line
                .split('\t')
                .map(decode)
                .collect::<Result<Vec<_>, _>>()?;
            if fields.first().map(String::as_str) != Some("inference") {
                continue;
            }
            if fields.len() != 16 {
                return Err(OidError::Evidence(
                    "malformed inference evidence record".to_owned(),
                ));
            }
            if fields[3] != request_id {
                continue;
            }
            records.push(InferenceEvidenceRecord {
                id: EvidenceId::new(fields[1].clone())
                    .map_err(|error| OidError::Evidence(error.to_string()))?,
                session_id: fields[2].clone(),
                request_id: fields[3].clone(),
                generation_id: nonempty(&fields[4]),
                model: fields[5].clone(),
                backend: fields[6].clone(),
                parameters: fields[7].clone(),
                started_at: fields[8].clone(),
                completed_at: nonempty(&fields[9]),
                outcome: fields[10].clone(),
                termination_reason: nonempty(&fields[11]),
                error_class: nonempty(&fields[12]),
                verification: fields[13].clone(),
                links: if fields[14].is_empty() {
                    Vec::new()
                } else {
                    fields[14].split(',').map(str::to_owned).collect()
                },
                metrics: fields[15].clone(),
            });
        }
        Ok(records)
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
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
        return Err(OidError::Evidence(
            "trailing escape in evidence record".to_owned(),
        ));
    }
    Ok(output)
}

/// Identifies the evidence boundary.
///
/// ```
/// assert_eq!(oid_evidence_engine::boundary_name(), "evidence-engine");
/// ```
#[must_use]
pub const fn boundary_name() -> &'static str {
    "evidence-engine"
}

#[cfg(test)]
mod tests {
    use super::{
        EvidenceRecord, EvidenceStore, FileEvidenceStore, InMemoryEvidenceStore,
        InferenceEvidenceRecord,
    };
    use oid_common::{EvidenceId, IntentId, OperationId};

    #[test]
    fn evidence_is_append_only_and_filterable() {
        let intent_id = IntentId::new("intent-1").expect("valid id");
        let operation_id = OperationId::new("operation-1").expect("valid id");
        let mut store = InMemoryEvidenceStore::default();
        store
            .append(EvidenceRecord {
                id: EvidenceId::new("evidence-1").expect("valid id"),
                intent_id,
                operation_id: operation_id.clone(),
                category: "plan".to_owned(),
                details: "inspect".to_owned(),
            })
            .expect("append works");
        assert_eq!(
            store.history(&operation_id).expect("history works").len(),
            1
        );
        assert_eq!(store.records().len(), 1);
    }

    #[test]
    fn file_store_round_trips_records() {
        let path = std::env::temp_dir().join(format!("oid-evidence-test-{}", std::process::id()));
        let intent_id = IntentId::new("intent-file").expect("valid id");
        let operation_id = OperationId::new("operation-file").expect("valid id");
        let mut store = FileEvidenceStore::new(&path);
        store
            .append(EvidenceRecord {
                id: EvidenceId::new("evidence-file").expect("valid id"),
                intent_id,
                operation_id: operation_id.clone(),
                category: "details".to_owned(),
                details: "line one\nline two".to_owned(),
            })
            .expect("file append works");
        let history = store.history(&operation_id).expect("file read works");
        assert_eq!(history[0].details, "line one\nline two");
        std::fs::remove_file(path).expect("test log cleanup works");
    }

    #[test]
    fn inference_evidence_survives_store_reopen_and_duplicate_append() {
        let path = std::env::temp_dir().join(format!(
            "oid-inference-evidence-test-{}",
            std::process::id()
        ));
        let record = InferenceEvidenceRecord {
            id: EvidenceId::new("inference-request-terminal").expect("valid id"),
            session_id: "session-1".to_owned(),
            request_id: "request-1".to_owned(),
            generation_id: Some("marina-generation-1".to_owned()),
            model: "model-1".to_owned(),
            backend: "marina-http".to_owned(),
            parameters: r#"{"max_tokens":8}"#.to_owned(),
            started_at: "100".to_owned(),
            completed_at: Some("200".to_owned()),
            outcome: "completed".to_owned(),
            termination_reason: None,
            error_class: None,
            verification: "response received".to_owned(),
            links: vec!["inference-request-started".to_owned()],
            metrics: "prompt_tokens=2;generated_tokens=3;latency_ms=4".to_owned(),
        };
        let mut store = FileEvidenceStore::new(&path);
        store
            .append_inference(record.clone())
            .expect("inference evidence append works");
        store
            .append_inference(record.clone())
            .expect("identical duplicate is idempotent");
        let reopened = FileEvidenceStore::new(&path);
        let history = reopened
            .inference_history("request-1")
            .expect("reopened history works");
        assert_eq!(history, vec![record]);
        std::fs::remove_file(path).expect("test log cleanup works");
    }
}
