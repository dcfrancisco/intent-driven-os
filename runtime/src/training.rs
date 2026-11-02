//! Backend-neutral model-development contracts and dataset persistence.
//!
//! This module deliberately contains no training framework integration. It
//! establishes the first ADR-0020 production boundary: immutable, validated,
//! content-addressed datasets and explicit training capability/configuration
//! contracts. Training workers remain separate from the inference process.

use crate::models::sha256_file;
use oid_shared::RuntimeError;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Authorization scope required to import or modify datasets.
pub const DATASETS_MANAGE: &str = "datasets:manage";

/// Initially supported immutable training dataset format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetFormat {
    /// UTF-8 JSON Lines containing prompt/completion or conversational records.
    SftJsonlV1,
}

impl DatasetFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::SftJsonlV1 => "talvik.sft-jsonl/v1",
        }
    }

    fn parse(value: &str) -> Result<Self, DatasetError> {
        match value {
            "talvik.sft-jsonl/v1" => Ok(Self::SftJsonlV1),
            _ => Err(DatasetError::CorruptRegistry(format!(
                "unsupported persisted dataset format: {value}"
            ))),
        }
    }
}

/// Resource and diagnostic bounds applied before dataset registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetLimits {
    /// Maximum source artifact size.
    pub max_bytes: u64,
    /// Maximum number of non-empty records.
    pub max_records: u64,
    /// Maximum bytes in one JSONL record.
    pub max_line_bytes: usize,
    /// Maximum detailed diagnostics retained in a validation result.
    pub max_diagnostics: usize,
}

impl Default for DatasetLimits {
    fn default() -> Self {
        Self {
            max_bytes: 256 * 1024 * 1024,
            max_records: 1_000_000,
            max_line_bytes: 1024 * 1024,
            max_diagnostics: 100,
        }
    }
}

/// One bounded dataset validation diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetDiagnostic {
    /// One-based JSONL line, or zero for an artifact-level error.
    pub line: u64,
    /// Stable machine-readable diagnostic code.
    pub code: String,
    /// Non-sensitive human-readable explanation.
    pub message: String,
}

/// Complete validation summary for one source artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetValidationReport {
    /// Whether every record is valid and the dataset is non-empty.
    pub valid: bool,
    /// Number of non-empty input records encountered.
    pub record_count: u64,
    /// Valid records assigned to the training partition.
    pub training_records: u64,
    /// Valid records assigned to the validation partition.
    pub validation_records: u64,
    /// Repeated canonical training examples, retained but explicitly reported.
    pub duplicate_records: u64,
    /// Bounded record and artifact diagnostics.
    pub diagnostics: Vec<DatasetDiagnostic>,
    /// Number of additional diagnostics omitted after reaching the limit.
    pub omitted_diagnostics: u64,
}

/// Explicit request to import one immutable dataset version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetImportRequest {
    /// Stable logical dataset identifier.
    pub dataset_id: String,
    /// Caller-assigned immutable version identifier.
    pub version: String,
    /// Explicitly selected local JSONL source.
    pub source: PathBuf,
    /// Human-readable source/provenance statement.
    pub provenance: String,
    /// Local principal responsible for the import.
    pub owner: String,
    /// Declared source format.
    pub format: DatasetFormat,
}

/// Durable immutable dataset version metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetVersion {
    /// Stable logical dataset identifier.
    pub dataset_id: String,
    /// Immutable version identifier.
    pub version: String,
    /// Source format validated during import.
    pub format: DatasetFormat,
    /// SHA-256 digest of the exact imported bytes.
    pub digest: String,
    /// Source artifact size.
    pub byte_size: u64,
    /// Content-addressed managed artifact path.
    pub artifact_path: PathBuf,
    /// Original explicitly selected source path.
    pub source: PathBuf,
    /// Caller-supplied provenance statement.
    pub provenance: String,
    /// Principal responsible for registration.
    pub owner: String,
    /// Unix timestamp at registration.
    pub created_at_unix_seconds: u64,
    /// Validation evidence captured before publication.
    pub validation: DatasetValidationReport,
}

/// Dataset validation, integrity, or persistence failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DatasetError {
    /// Invalid caller-controlled identifier or metadata.
    InvalidInput(String),
    /// Source data failed schema or encoding validation.
    ValidationFailed(DatasetValidationReport),
    /// An immutable identity already points at different content or metadata.
    Conflict(String),
    /// Filesystem persistence failed.
    Persistence(String),
    /// Durable registry content is malformed or references invalid data.
    CorruptRegistry(String),
}

impl fmt::Display for DatasetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "invalid dataset input: {message}"),
            Self::ValidationFailed(report) => write!(
                formatter,
                "dataset validation failed with {} diagnostic(s)",
                report.diagnostics.len().saturating_add(
                    usize::try_from(report.omitted_diagnostics).unwrap_or(usize::MAX),
                )
            ),
            Self::Conflict(message) => write!(formatter, "dataset conflict: {message}"),
            Self::Persistence(message) => {
                write!(formatter, "dataset persistence failed: {message}")
            }
            Self::CorruptRegistry(message) => {
                write!(formatter, "dataset registry is corrupt: {message}")
            }
        }
    }
}

impl std::error::Error for DatasetError {}

/// Durable content-addressed dataset registry.
#[derive(Clone, Debug)]
pub struct DatasetRegistry {
    root: PathBuf,
    entries: Arc<Mutex<BTreeMap<String, DatasetVersion>>>,
}

impl DatasetRegistry {
    /// Open a managed dataset store, validating durable metadata and artifacts.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry is malformed, contains duplicate
    /// identities, references a missing artifact, or cannot be read.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, DatasetError> {
        let root = root.as_ref().to_path_buf();
        let registry_path = root.join("registry.jsonl");
        let mut entries = BTreeMap::new();
        if registry_path.exists() {
            let file = File::open(&registry_path).map_err(persistence)?;
            for (index, line) in BufReader::new(file).lines().enumerate() {
                let line = line.map_err(persistence)?;
                if line.trim().is_empty() {
                    continue;
                }
                let entry = decode_dataset(&line).map_err(|error| {
                    DatasetError::CorruptRegistry(format!("line {}: {error}", index + 1))
                })?;
                validate_identifier("dataset_id", &entry.dataset_id)?;
                validate_identifier("version", &entry.version)?;
                if entry.digest.len() != 64
                    || !entry.digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err(DatasetError::CorruptRegistry(format!(
                        "{}@{} has an invalid SHA-256 digest",
                        entry.dataset_id, entry.version
                    )));
                }
                let expected_artifact = root
                    .join("artifacts")
                    .join("sha256")
                    .join(format!("{}.jsonl", entry.digest));
                if entry.artifact_path != expected_artifact {
                    return Err(DatasetError::CorruptRegistry(format!(
                        "{}@{} references an artifact outside its content address",
                        entry.dataset_id, entry.version
                    )));
                }
                if !entry.artifact_path.is_file() {
                    return Err(DatasetError::CorruptRegistry(format!(
                        "{}@{} references missing artifact {}",
                        entry.dataset_id,
                        entry.version,
                        entry.artifact_path.display()
                    )));
                }
                let actual_digest =
                    sha256_file(&entry.artifact_path).map_err(runtime_persistence)?;
                if actual_digest != entry.digest {
                    return Err(DatasetError::CorruptRegistry(format!(
                        "{}@{} artifact digest does not match durable metadata",
                        entry.dataset_id, entry.version
                    )));
                }
                let key = dataset_key(&entry.dataset_id, &entry.version);
                if entries.insert(key.clone(), entry).is_some() {
                    return Err(DatasetError::CorruptRegistry(format!(
                        "duplicate dataset identity {key}"
                    )));
                }
            }
        }
        Ok(Self {
            root,
            entries: Arc::new(Mutex::new(entries)),
        })
    }

    /// Validate a source without copying or registering it.
    ///
    /// # Errors
    ///
    /// Returns an error only when the source cannot be safely inspected or
    /// read. Schema problems are returned in a report with `valid == false`.
    pub fn validate(
        source: &Path,
        format: DatasetFormat,
        limits: &DatasetLimits,
    ) -> Result<DatasetValidationReport, DatasetError> {
        if format != DatasetFormat::SftJsonlV1 {
            return Err(DatasetError::InvalidInput(
                "unsupported dataset format".to_owned(),
            ));
        }
        let metadata = fs::metadata(source).map_err(persistence)?;
        if !metadata.is_file() {
            return Err(DatasetError::InvalidInput(format!(
                "dataset source is not a regular file: {}",
                source.display()
            )));
        }
        if metadata.len() > limits.max_bytes {
            return Ok(DatasetValidationReport::artifact_failure(
                "dataset_too_large",
                format!(
                    "dataset is {} bytes; limit is {} bytes",
                    metadata.len(),
                    limits.max_bytes
                ),
            ));
        }

        let file = File::open(source).map_err(persistence)?;
        let reader = BufReader::new(file);
        let mut bytes = Vec::new();
        reader
            .take(limits.max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(persistence)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.max_bytes {
            return Ok(DatasetValidationReport::artifact_failure(
                "dataset_too_large",
                format!("dataset grew beyond the {} byte limit", limits.max_bytes),
            ));
        }
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(error) => {
                return Ok(DatasetValidationReport::artifact_failure(
                    "invalid_utf8",
                    format!("dataset is not UTF-8 at byte {}", error.valid_up_to()),
                ));
            }
        };

        Ok(validate_jsonl_text(text, limits))
    }

    /// Validate, copy, verify, and atomically register an immutable dataset.
    ///
    /// Repeating an identical request is idempotent. Reusing an identity for
    /// different bytes or metadata is rejected.
    ///
    /// # Errors
    ///
    /// Returns a validation report on malformed data, or a typed integrity,
    /// identity, or persistence failure.
    pub fn import(
        &self,
        request: &DatasetImportRequest,
        limits: &DatasetLimits,
    ) -> Result<DatasetVersion, DatasetError> {
        validate_identifier("dataset_id", &request.dataset_id)?;
        validate_identifier("version", &request.version)?;
        validate_metadata("owner", &request.owner, 256)?;
        validate_metadata("provenance", &request.provenance, 4096)?;
        let source_report = Self::validate(&request.source, request.format, limits)?;
        if !source_report.valid {
            return Err(DatasetError::ValidationFailed(source_report));
        }
        let digest = sha256_file(&request.source).map_err(runtime_persistence)?;
        let byte_size = fs::metadata(&request.source).map_err(persistence)?.len();
        let key = dataset_key(&request.dataset_id, &request.version);
        let artifact_directory = self.root.join("artifacts").join("sha256");
        let artifact_path = artifact_directory.join(format!("{digest}.jsonl"));
        let entry = DatasetVersion {
            dataset_id: request.dataset_id.clone(),
            version: request.version.clone(),
            format: request.format,
            digest,
            byte_size,
            artifact_path,
            source: request.source.clone(),
            provenance: request.provenance.clone(),
            owner: request.owner.clone(),
            created_at_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            validation: source_report,
        };

        let mut entries = self.lock_entries()?;
        if let Some(existing) = entries.get(&key) {
            if same_immutable_import(existing, &entry) {
                return Ok(existing.clone());
            }
            return Err(DatasetError::Conflict(format!(
                "{key} is already registered with different immutable metadata"
            )));
        }

        let artifact_created =
            publish_artifact(&request.source, &entry.artifact_path, &entry.digest)?;
        let artifact_report = Self::validate(&entry.artifact_path, request.format, limits)?;
        if !artifact_report.valid {
            if artifact_created {
                let _ = fs::remove_file(&entry.artifact_path);
            }
            return Err(DatasetError::ValidationFailed(artifact_report));
        }
        let mut entry = entry;
        entry.validation = artifact_report;
        entries.insert(key.clone(), entry.clone());
        if let Err(error) = self.persist(&entries) {
            entries.remove(&key);
            if artifact_created
                && !entries
                    .values()
                    .any(|existing| existing.artifact_path == entry.artifact_path)
            {
                let _ = fs::remove_file(&entry.artifact_path);
            }
            return Err(error);
        }
        Ok(entry)
    }

    /// List all registered versions in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error when the in-process registry lock is unavailable.
    pub fn list(&self) -> Result<Vec<DatasetVersion>, DatasetError> {
        Ok(self.lock_entries()?.values().cloned().collect())
    }

    /// Inspect one immutable dataset version.
    ///
    /// # Errors
    ///
    /// Returns an error when the in-process registry lock is unavailable.
    pub fn inspect(
        &self,
        dataset_id: &str,
        version: &str,
    ) -> Result<Option<DatasetVersion>, DatasetError> {
        Ok(self
            .lock_entries()?
            .get(&dataset_key(dataset_id, version))
            .cloned())
    }

    fn lock_entries(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, DatasetVersion>>, DatasetError> {
        self.entries
            .lock()
            .map_err(|_| DatasetError::Persistence("dataset registry lock poisoned".to_owned()))
    }

    fn persist(&self, entries: &BTreeMap<String, DatasetVersion>) -> Result<(), DatasetError> {
        fs::create_dir_all(&self.root).map_err(persistence)?;
        let registry_path = self.root.join("registry.jsonl");
        let temporary_path = self.root.join(".registry.jsonl.tmp");
        let mut temporary = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)
            .map_err(persistence)?;
        for entry in entries.values() {
            serde_json::to_writer(&mut temporary, &encode_dataset(entry))
                .map_err(|error| DatasetError::Persistence(error.to_string()))?;
            temporary.write_all(b"\n").map_err(persistence)?;
        }
        temporary.sync_all().map_err(persistence)?;
        fs::rename(&temporary_path, registry_path).map_err(persistence)
    }
}

impl DatasetValidationReport {
    fn artifact_failure(code: &str, message: String) -> Self {
        Self {
            valid: false,
            record_count: 0,
            training_records: 0,
            validation_records: 0,
            duplicate_records: 0,
            diagnostics: vec![DatasetDiagnostic {
                line: 0,
                code: code.to_owned(),
                message,
            }],
            omitted_diagnostics: 0,
        }
    }

    fn add_diagnostic(&mut self, limits: &DatasetLimits, line: u64, code: &str, message: &str) {
        self.valid = false;
        if self.diagnostics.len() < limits.max_diagnostics {
            self.diagnostics.push(DatasetDiagnostic {
                line,
                code: code.to_owned(),
                message: message.to_owned(),
            });
        } else {
            self.omitted_diagnostics = self.omitted_diagnostics.saturating_add(1);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DatasetPartition {
    Train,
    Validation,
}

fn validate_jsonl_text(text: &str, limits: &DatasetLimits) -> DatasetValidationReport {
    let mut report = DatasetValidationReport {
        valid: true,
        record_count: 0,
        training_records: 0,
        validation_records: 0,
        duplicate_records: 0,
        diagnostics: Vec::new(),
        omitted_diagnostics: 0,
    };
    let mut examples = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = u64::try_from(index + 1).unwrap_or(u64::MAX);
        if line.trim().is_empty() {
            report.add_diagnostic(
                limits,
                line_number,
                "empty_record",
                "empty JSONL records are not allowed",
            );
            continue;
        }
        report.record_count = report.record_count.saturating_add(1);
        if report.record_count > limits.max_records {
            report.add_diagnostic(
                limits,
                line_number,
                "record_limit_exceeded",
                &format!("dataset exceeds {} records", limits.max_records),
            );
            break;
        }
        if line.len() > limits.max_line_bytes {
            report.add_diagnostic(
                limits,
                line_number,
                "record_too_large",
                &format!(
                    "record is {} bytes; limit is {} bytes",
                    line.len(),
                    limits.max_line_bytes
                ),
            );
            continue;
        }
        validate_jsonl_line(line, line_number, limits, &mut examples, &mut report);
    }
    if report.record_count == 0 {
        report.add_diagnostic(
            limits,
            0,
            "empty_dataset",
            "dataset contains no training records",
        );
    }
    report.valid = report.diagnostics.is_empty() && report.omitted_diagnostics == 0;
    report
}

fn validate_jsonl_line(
    line: &str,
    line_number: u64,
    limits: &DatasetLimits,
    examples: &mut BTreeSet<String>,
    report: &mut DatasetValidationReport,
) {
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            report.add_diagnostic(
                limits,
                line_number,
                "invalid_json",
                &format!("invalid JSON: {error}"),
            );
            return;
        }
    };
    match validate_record(&value) {
        Ok((split, canonical)) => {
            if !examples.insert(canonical) {
                report.duplicate_records = report.duplicate_records.saturating_add(1);
            }
            match split {
                DatasetPartition::Train => {
                    report.training_records = report.training_records.saturating_add(1);
                }
                DatasetPartition::Validation => {
                    report.validation_records = report.validation_records.saturating_add(1);
                }
            }
        }
        Err((code, message)) => {
            report.add_diagnostic(limits, line_number, code, &message);
        }
    }
}

fn validate_record(value: &Value) -> Result<(DatasetPartition, String), (&'static str, String)> {
    let object = value.as_object().ok_or_else(|| {
        (
            "record_not_object",
            "each record must be a JSON object".to_owned(),
        )
    })?;
    let split = match object.get("split").and_then(Value::as_str) {
        None | Some("train") => DatasetPartition::Train,
        Some("validation") => DatasetPartition::Validation,
        Some(other) => {
            return Err((
                "unsupported_split",
                format!("split must be train or validation, got {other}"),
            ));
        }
    };

    let has_prompt = object.contains_key("prompt") || object.contains_key("completion");
    let has_messages = object.contains_key("messages");
    if has_prompt && has_messages {
        return Err((
            "ambiguous_record",
            "record cannot contain both prompt/completion and messages".to_owned(),
        ));
    }
    if has_prompt {
        let prompt = required_nonempty_string(object, "prompt")?;
        let completion = required_nonempty_string(object, "completion")?;
        return Ok((split, format!("prompt\0{prompt}\0{completion}")));
    }
    if has_messages {
        let messages = object
            .get("messages")
            .and_then(Value::as_array)
            .filter(|messages| !messages.is_empty())
            .ok_or_else(|| {
                (
                    "invalid_messages",
                    "messages must be a non-empty array".to_owned(),
                )
            })?;
        let mut saw_user = false;
        let mut saw_assistant = false;
        let mut canonical = Vec::with_capacity(messages.len());
        for (index, message) in messages.iter().enumerate() {
            let message = message.as_object().ok_or_else(|| {
                (
                    "invalid_message",
                    format!("message {} must be an object", index + 1),
                )
            })?;
            let role = required_nonempty_string(message, "role")?;
            if !matches!(role, "system" | "user" | "assistant") {
                return Err((
                    "unsupported_role",
                    format!("message {} has unsupported role {role}", index + 1),
                ));
            }
            let content = required_nonempty_string(message, "content")?;
            saw_user |= role == "user";
            saw_assistant |= role == "assistant";
            canonical.push(Value::Array(vec![
                Value::String(role.to_owned()),
                Value::String(content.to_owned()),
            ]));
        }
        if !saw_user || !saw_assistant {
            return Err((
                "incomplete_conversation",
                "messages must contain at least one user and assistant turn".to_owned(),
            ));
        }
        return Ok((split, serde_json::to_string(&canonical).unwrap_or_default()));
    }
    Err((
        "unsupported_schema",
        "record must contain prompt/completion or messages".to_owned(),
    ))
}

fn required_nonempty_string<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, (&'static str, String)> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            (
                "missing_text",
                format!("{field} must be a non-empty string"),
            )
        })
}

fn validate_identifier(field: &str, value: &str) -> Result<(), DatasetError> {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(DatasetError::InvalidInput(format!(
            "{field} must be 1-128 ASCII letters, digits, '.', '_' or '-'"
        )));
    }
    Ok(())
}

fn validate_metadata(field: &str, value: &str, maximum: usize) -> Result<(), DatasetError> {
    if value.trim().is_empty() || value.len() > maximum || value.contains(['\0', '\n', '\r']) {
        return Err(DatasetError::InvalidInput(format!(
            "{field} must be non-empty, single-line, and at most {maximum} bytes"
        )));
    }
    Ok(())
}

fn publish_artifact(source: &Path, target: &Path, digest: &str) -> Result<bool, DatasetError> {
    if target.exists() {
        let existing = sha256_file(target).map_err(runtime_persistence)?;
        if existing != digest {
            return Err(DatasetError::Conflict(format!(
                "managed artifact {} does not match its content address",
                target.display()
            )));
        }
        return Ok(false);
    }
    let parent = target.parent().ok_or_else(|| {
        DatasetError::Persistence("managed artifact path has no parent".to_owned())
    })?;
    fs::create_dir_all(parent).map_err(persistence)?;
    let temporary = parent.join(format!(".{digest}.part"));
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(persistence)?;
    }
    let result = (|| {
        let mut input = File::open(source).map_err(persistence)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(persistence)?;
        std::io::copy(&mut input, &mut output).map_err(persistence)?;
        output.sync_all().map_err(persistence)?;
        let copied_digest = sha256_file(&temporary).map_err(runtime_persistence)?;
        if copied_digest != digest {
            return Err(DatasetError::Conflict(
                "source changed while it was being imported".to_owned(),
            ));
        }
        fs::rename(&temporary, target)
            .map_err(persistence)
            .map(|()| true)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn same_immutable_import(left: &DatasetVersion, right: &DatasetVersion) -> bool {
    left.dataset_id == right.dataset_id
        && left.version == right.version
        && left.format == right.format
        && left.digest == right.digest
        && left.byte_size == right.byte_size
        && left.provenance == right.provenance
        && left.owner == right.owner
}

fn dataset_key(dataset_id: &str, version: &str) -> String {
    format!("{dataset_id}@{version}")
}

#[allow(clippy::needless_pass_by_value)]
fn persistence(error: std::io::Error) -> DatasetError {
    DatasetError::Persistence(error.to_string())
}

#[allow(clippy::needless_pass_by_value)]
fn runtime_persistence(error: RuntimeError) -> DatasetError {
    DatasetError::Persistence(error.to_string())
}

fn encode_dataset(entry: &DatasetVersion) -> Value {
    serde_json::json!({
        "schema": "talvik.dataset-registry/v1",
        "dataset_id": entry.dataset_id,
        "version": entry.version,
        "format": entry.format.as_str(),
        "digest": entry.digest,
        "byte_size": entry.byte_size,
        "artifact_path": entry.artifact_path,
        "source": entry.source,
        "provenance": entry.provenance,
        "owner": entry.owner,
        "created_at_unix_seconds": entry.created_at_unix_seconds,
        "validation": {
            "record_count": entry.validation.record_count,
            "training_records": entry.validation.training_records,
            "validation_records": entry.validation.validation_records,
            "duplicate_records": entry.validation.duplicate_records
        }
    })
}

fn decode_dataset(line: &str) -> Result<DatasetVersion, DatasetError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| DatasetError::CorruptRegistry(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| DatasetError::CorruptRegistry("record is not an object".to_owned()))?;
    if string_field(object, "schema")? != "talvik.dataset-registry/v1" {
        return Err(DatasetError::CorruptRegistry(
            "unsupported registry schema".to_owned(),
        ));
    }
    let validation = object
        .get("validation")
        .and_then(Value::as_object)
        .ok_or_else(|| DatasetError::CorruptRegistry("missing validation".to_owned()))?;
    Ok(DatasetVersion {
        dataset_id: string_field(object, "dataset_id")?.to_owned(),
        version: string_field(object, "version")?.to_owned(),
        format: DatasetFormat::parse(string_field(object, "format")?)?,
        digest: string_field(object, "digest")?.to_owned(),
        byte_size: integer_field(object, "byte_size")?,
        artifact_path: PathBuf::from(string_field(object, "artifact_path")?),
        source: PathBuf::from(string_field(object, "source")?),
        provenance: string_field(object, "provenance")?.to_owned(),
        owner: string_field(object, "owner")?.to_owned(),
        created_at_unix_seconds: integer_field(object, "created_at_unix_seconds")?,
        validation: DatasetValidationReport {
            valid: true,
            record_count: integer_field(validation, "record_count")?,
            training_records: integer_field(validation, "training_records")?,
            validation_records: integer_field(validation, "validation_records")?,
            duplicate_records: integer_field(validation, "duplicate_records")?,
            diagnostics: Vec::new(),
            omitted_diagnostics: 0,
        },
    })
}

fn string_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, DatasetError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| DatasetError::CorruptRegistry(format!("missing string field {field}")))
}

fn integer_field(object: &Map<String, Value>, field: &str) -> Result<u64, DatasetError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| DatasetError::CorruptRegistry(format!("missing integer field {field}")))
}

/// Training method requested from a replaceable worker backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrainingMethod {
    /// Parameter-efficient low-rank adaptation.
    Lora,
    /// Quantized low-rank adaptation; a distinct capability from `LoRA`.
    Qlora,
    /// Full parameter fine-tuning extension point; not currently executable.
    FullFineTune,
    /// Experimental training-from-scratch extension point; not executable.
    FromScratch,
}

/// Backend capability advertisement with no framework-native types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingBackendCapabilities {
    /// Stable backend implementation identifier.
    pub backend_id: String,
    /// Backend contract/version string.
    pub backend_version: String,
    /// Explicit supported methods.
    pub methods: Vec<TrainingMethod>,
    /// Explicit device classes, such as `cpu`, `cuda`, `rocm`, or `mps`.
    pub devices: Vec<String>,
    /// Whether checkpoint resume is implemented by this backend.
    pub supports_resume: bool,
}

/// Backend-neutral bounded adapter-training configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainingConfig {
    /// Requested training method.
    pub method: TrainingMethod,
    /// Exact base model identity/digest.
    pub base_model: String,
    /// Exact dataset identity and version.
    pub dataset: String,
    /// Required execution device; never silently substituted.
    pub device: String,
    /// Training epochs.
    pub epochs: u32,
    /// Optimizer learning rate.
    pub learning_rate: f64,
    /// Per-device batch size.
    pub batch_size: u32,
    /// Gradient accumulation steps.
    pub gradient_accumulation: u32,
    /// Maximum training sequence length.
    pub sequence_length: u32,
    /// `LoRA` rank.
    pub lora_rank: u32,
    /// `LoRA` alpha.
    pub lora_alpha: u32,
    /// `LoRA` dropout probability.
    pub lora_dropout: f64,
    /// Explicit target module names.
    pub target_modules: Vec<String>,
    /// Deterministic seed supplied to the backend.
    pub seed: u64,
}

impl TrainingConfig {
    /// Validate configuration against actual advertised backend capabilities.
    ///
    /// # Errors
    ///
    /// Returns all discovered actionable configuration errors. No fallback
    /// device or method is selected.
    pub fn validate(&self, capabilities: &TrainingBackendCapabilities) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if !capabilities.methods.contains(&self.method) {
            errors.push(format!(
                "backend {} does not support {:?}",
                capabilities.backend_id, self.method
            ));
        }
        if !capabilities
            .devices
            .iter()
            .any(|device| device == &self.device)
        {
            errors.push(format!(
                "backend {} does not provide requested device {}; no fallback is allowed",
                capabilities.backend_id, self.device
            ));
        }
        if self.base_model.trim().is_empty() {
            errors.push("base_model is required".to_owned());
        }
        if self.dataset.trim().is_empty() {
            errors.push("dataset is required".to_owned());
        }
        if self.epochs == 0 {
            errors.push("epochs must be greater than zero".to_owned());
        }
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            errors.push("learning_rate must be finite and greater than zero".to_owned());
        }
        if self.batch_size == 0 || self.gradient_accumulation == 0 {
            errors
                .push("batch_size and gradient_accumulation must be greater than zero".to_owned());
        }
        if self.sequence_length == 0 {
            errors.push("sequence_length must be greater than zero".to_owned());
        }
        if matches!(self.method, TrainingMethod::Lora | TrainingMethod::Qlora) {
            if self.lora_rank == 0 || self.lora_alpha == 0 {
                errors.push("LoRA rank and alpha must be greater than zero".to_owned());
            }
            if !self.lora_dropout.is_finite() || !(0.0..1.0).contains(&self.lora_dropout) {
                errors.push("LoRA dropout must be finite and in [0, 1)".to_owned());
            }
            if self.target_modules.is_empty()
                || self
                    .target_modules
                    .iter()
                    .any(|module| module.trim().is_empty())
            {
                errors.push("at least one non-empty LoRA target module is required".to_owned());
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Persisted training job state from ADR-0020.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrainingJobState {
    /// Durable pending job record exists but validation has not started.
    Pending,
    /// Dataset, model, backend, and resources are being validated.
    Validating,
    /// Valid job is waiting for resource admission.
    Queued,
    /// Isolated worker environment is being prepared.
    Preparing,
    /// Worker is updating model or adapter weights.
    Running,
    /// Worker is publishing a durable checkpoint.
    Checkpointing,
    /// Candidate is undergoing configured evaluation.
    Evaluating,
    /// Job and required artifact publication completed successfully.
    Completed,
    /// Job ended unsuccessfully.
    Failed,
    /// Cancellation has been requested and worker termination is pending.
    Cancelling,
    /// Job was cancelled without publishing a successful candidate.
    Cancelled,
}

impl TrainingJobState {
    /// Return whether a state-machine transition is valid.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        use TrainingJobState::{
            Cancelled, Cancelling, Checkpointing, Completed, Evaluating, Failed, Pending,
            Preparing, Queued, Running, Validating,
        };
        match self {
            Pending => matches!(next, Validating | Failed | Cancelling),
            Validating => matches!(next, Queued | Failed | Cancelling),
            Queued => matches!(next, Preparing | Failed | Cancelling),
            Preparing => matches!(next, Running | Failed | Cancelling),
            Running => matches!(next, Checkpointing | Evaluating | Failed | Cancelling),
            Checkpointing => matches!(next, Running | Evaluating | Failed | Cancelling),
            Evaluating => matches!(next, Completed | Failed | Cancelling),
            Cancelling => matches!(next, Cancelled | Failed),
            Completed | Failed | Cancelled => false,
        }
    }

    /// Return whether no further execution transition is allowed.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "talvik-training-{label}-{}-{timestamp}",
            std::process::id()
        ))
    }

    fn request(source: PathBuf) -> DatasetImportRequest {
        DatasetImportRequest {
            dataset_id: "support-sft".to_owned(),
            version: "v1".to_owned(),
            source,
            provenance: "explicit local test fixture".to_owned(),
            owner: "test-principal".to_owned(),
            format: DatasetFormat::SftJsonlV1,
        }
    }

    #[test]
    fn validates_prompt_and_conversation_records_deterministically() {
        let root = temporary_directory("validate");
        fs::create_dir_all(&root).expect("root");
        let source = root.join("dataset.jsonl");
        fs::write(
            &source,
            concat!(
                "{\"prompt\":\"Q\",\"completion\":\"A\"}\n",
                "{\"messages\":[{\"role\":\"user\",\"content\":\"Q2\"},{\"role\":\"assistant\",\"content\":\"A2\"}],\"split\":\"validation\"}\n",
                "{\"completion\":\"A\",\"prompt\":\"Q\"}\n"
            ),
        )
        .expect("fixture");
        let report = DatasetRegistry::validate(
            &source,
            DatasetFormat::SftJsonlV1,
            &DatasetLimits::default(),
        )
        .expect("validate");
        assert!(report.valid);
        assert_eq!(report.record_count, 3);
        assert_eq!(report.training_records, 2);
        assert_eq!(report.validation_records, 1);
        assert_eq!(report.duplicate_records, 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn reports_every_malformed_record_without_importing_it() {
        let root = temporary_directory("invalid");
        fs::create_dir_all(&root).expect("root");
        let source = root.join("dataset.jsonl");
        fs::write(
            &source,
            "not-json\n{\"prompt\":\"Q\"}\n{\"messages\":[{\"role\":\"tool\",\"content\":\"x\"}]}\n",
        )
        .expect("fixture");
        let store = DatasetRegistry::open(root.join("store")).expect("store");
        let error = store
            .import(&request(source), &DatasetLimits::default())
            .expect_err("invalid import");
        let DatasetError::ValidationFailed(report) = error else {
            panic!("expected validation report");
        };
        assert_eq!(report.record_count, 3);
        assert_eq!(report.diagnostics.len(), 3);
        assert!(store.list().expect("list").is_empty());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn import_is_content_addressed_idempotent_and_survives_restart() {
        let root = temporary_directory("persist");
        fs::create_dir_all(&root).expect("root");
        let source = root.join("dataset.jsonl");
        fs::write(
            &source,
            "{\"prompt\":\"Question\",\"completion\":\"Answer\",\"split\":\"train\"}\n",
        )
        .expect("fixture");
        let store_root = root.join("store");
        let store = DatasetRegistry::open(&store_root).expect("store");
        let first = store
            .import(&request(source.clone()), &DatasetLimits::default())
            .expect("import");
        let second = store
            .import(&request(source), &DatasetLimits::default())
            .expect("idempotent import");
        assert_eq!(first, second);
        assert!(first.artifact_path.is_file());
        drop(store);

        let reopened = DatasetRegistry::open(&store_root).expect("reopen");
        assert_eq!(reopened.list().expect("list"), vec![first]);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn immutable_identity_rejects_changed_content() {
        let root = temporary_directory("conflict");
        fs::create_dir_all(&root).expect("root");
        let source = root.join("dataset.jsonl");
        fs::write(&source, "{\"prompt\":\"Q\",\"completion\":\"A\"}\n").expect("fixture");
        let store = DatasetRegistry::open(root.join("store")).expect("store");
        store
            .import(&request(source.clone()), &DatasetLimits::default())
            .expect("first import");
        fs::write(&source, "{\"prompt\":\"Q\",\"completion\":\"B\"}\n").expect("changed fixture");
        assert!(matches!(
            store.import(&request(source), &DatasetLimits::default()),
            Err(DatasetError::Conflict(_))
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn identifiers_cannot_escape_the_managed_store() {
        let root = temporary_directory("traversal");
        fs::create_dir_all(&root).expect("root");
        let source = root.join("dataset.jsonl");
        fs::write(&source, "{\"prompt\":\"Q\",\"completion\":\"A\"}\n").expect("fixture");
        let store = DatasetRegistry::open(root.join("store")).expect("store");
        let mut invalid = request(source);
        invalid.dataset_id = "../escape".to_owned();
        assert!(matches!(
            store.import(&invalid, &DatasetLimits::default()),
            Err(DatasetError::InvalidInput(_))
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn job_state_machine_rejects_skips_and_terminal_restarts() {
        assert!(TrainingJobState::Pending.can_transition_to(TrainingJobState::Validating));
        assert!(TrainingJobState::Running.can_transition_to(TrainingJobState::Checkpointing));
        assert!(TrainingJobState::Evaluating.can_transition_to(TrainingJobState::Completed));
        assert!(!TrainingJobState::Pending.can_transition_to(TrainingJobState::Running));
        assert!(!TrainingJobState::Completed.can_transition_to(TrainingJobState::Running));
        assert!(TrainingJobState::Completed.is_terminal());
    }

    #[test]
    fn qlora_and_requested_device_require_explicit_capabilities() {
        let capabilities = TrainingBackendCapabilities {
            backend_id: "cpu-lora".to_owned(),
            backend_version: "1".to_owned(),
            methods: vec![TrainingMethod::Lora],
            devices: vec!["cpu".to_owned()],
            supports_resume: false,
        };
        let config = TrainingConfig {
            method: TrainingMethod::Qlora,
            base_model: "base@sha256:abc".to_owned(),
            dataset: "support-sft@v1".to_owned(),
            device: "cuda".to_owned(),
            epochs: 1,
            learning_rate: 0.0002,
            batch_size: 1,
            gradient_accumulation: 1,
            sequence_length: 128,
            lora_rank: 8,
            lora_alpha: 16,
            lora_dropout: 0.05,
            target_modules: vec!["q_proj".to_owned()],
            seed: 42,
        };
        let errors = config.validate(&capabilities).expect_err("unsupported");
        assert!(errors.iter().any(|error| error.contains("Qlora")));
        assert!(errors.iter().any(|error| error.contains("no fallback")));
    }
}
