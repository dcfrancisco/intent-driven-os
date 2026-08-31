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
    use super::{EvidenceRecord, EvidenceStore, FileEvidenceStore, InMemoryEvidenceStore};
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
}
