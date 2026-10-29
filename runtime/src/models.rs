//! Persistent model metadata registry.

use oid_shared::{EventBus, RuntimeError, RuntimeEvent};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Model lifecycle status represented by metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelStatus {
    /// Metadata is registered but no backend has loaded it.
    Registered,
    /// A backend reports the model as loaded.
    Loaded,
    /// A backend is currently loading the model.
    Loading,
    /// A backend is currently unloading the model.
    Unloading,
    /// Loading or unloading failed.
    Failed,
    /// The model is unavailable or invalid.
    Unavailable,
}

impl ModelStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::Loaded => "loaded",
            Self::Loading => "loading",
            Self::Unloading => "unloading",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "loaded" => Self::Loaded,
            "loading" => Self::Loading,
            "unloading" => Self::Unloading,
            "failed" => Self::Failed,
            "unavailable" => Self::Unavailable,
            _ => Self::Registered,
        }
    }
}

/// Descriptive metadata for a model artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelMetadata {
    /// Stable model identifier.
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Model family.
    pub family: String,
    /// Preferred or compatible backend identifier.
    pub backend: String,
    /// Quantization label.
    pub quantization: String,
    /// Maximum context window in tokens.
    pub context_window: u64,
    /// Estimated memory requirement in megabytes.
    pub memory_requirement_mb: u64,
    /// Declared model capabilities.
    pub capabilities: Vec<String>,
    /// Metadata lifecycle status.
    pub status: ModelStatus,
    /// Optional artifact checksum.
    pub checksum: Option<String>,
    /// Local GGUF projector paired with the text model for vision input.
    pub vision_projector_location: Option<String>,
    /// Local or remote artifact location.
    pub location: String,
}

/// Runtime-facing model metadata registry.
#[derive(Clone)]
pub struct ModelRegistry {
    models: Arc<Mutex<BTreeMap<String, ModelMetadata>>>,
    path: Option<PathBuf>,
    bus: EventBus,
}

impl std::fmt::Debug for ModelRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelRegistry")
            .field("models", &self.list())
            .field("path", &self.path)
            .field("bus", &self.bus)
            .finish()
    }
}

impl ModelRegistry {
    /// Create an in-memory registry for tests and the in-process runtime.
    #[must_use]
    pub fn in_memory(bus: EventBus) -> Self {
        Self {
            models: Arc::new(Mutex::new(BTreeMap::new())),
            path: None,
            bus,
        }
    }

    /// Open a persistent registry file, creating it on first mutation.
    ///
    /// # Errors
    ///
    /// Returns an error when the existing registry cannot be read or decoded.
    pub fn open(path: impl AsRef<Path>, bus: EventBus) -> Result<Self, RuntimeError> {
        let path = path.as_ref().to_path_buf();
        let models = if path.exists() {
            decode_registry(
                &fs::read_to_string(&path)
                    .map_err(|error| RuntimeError::Persistence(error.to_string()))?,
            )?
        } else {
            BTreeMap::new()
        };
        Ok(Self {
            models: Arc::new(Mutex::new(models)),
            path: Some(path),
            bus,
        })
    }

    /// Register model metadata and persist it when backed by a file.
    ///
    /// # Errors
    ///
    /// Re-registering identical metadata is idempotent, which lets discovery
    /// safely scan overlapping cache directories. A conflicting definition for
    /// an existing ID remains an error.
    pub fn register(&self, model: ModelMetadata) -> Result<(), RuntimeError> {
        let id = model.id.clone();
        let mut models = self.lock_models()?;
        if let Some(existing) = models.get(&id) {
            if existing == &model {
                return Ok(());
            }
            return Err(RuntimeError::ModelAlreadyRegistered(id));
        }
        models.insert(id.clone(), model);
        if let Err(error) = self.persist(&models) {
            models.remove(&id);
            return Err(error);
        }
        drop(models);
        self.bus.publish(&RuntimeEvent::ModelRegistered(id));
        Ok(())
    }

    /// Remove model metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID, unavailable registry state, or
    /// persistence failure.
    pub fn remove(&self, id: &str) -> Result<ModelMetadata, RuntimeError> {
        let mut models = self.lock_models()?;
        let removed = models
            .remove(id)
            .ok_or_else(|| RuntimeError::ModelNotFound(id.to_owned()))?;
        if let Err(error) = self.persist(&models) {
            models.insert(id.to_owned(), removed.clone());
            return Err(error);
        }
        Ok(removed)
    }

    /// Pair or unpair a vision projector with an existing model.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is unknown, the registry is unavailable,
    /// or persistence fails.
    pub fn set_vision_projector(
        &self,
        id: &str,
        projector_path: Option<String>,
    ) -> Result<(), RuntimeError> {
        let mut models = self.lock_models()?;
        let model = models
            .get_mut(id)
            .ok_or_else(|| RuntimeError::ModelNotFound(id.to_owned()))?;
        let model_size = fs::metadata(&model.location)
            .map_err(|error| {
                RuntimeError::InvalidModel(format!(
                    "could not inspect model artifact {}: {error}",
                    model.location
                ))
            })?
            .len();
        let projector_size = projector_path
            .as_deref()
            .map(|path| {
                fs::metadata(path)
                    .map(|metadata| metadata.len())
                    .map_err(|error| {
                        RuntimeError::InvalidModel(format!(
                            "could not inspect vision projector {path}: {error}"
                        ))
                    })
            })
            .transpose()?
            .unwrap_or_default();
        let total_size = model_size.checked_add(projector_size).ok_or_else(|| {
            RuntimeError::InvalidModel("model and projector size overflowed".to_owned())
        })?;
        let previous_projector = model.vision_projector_location.clone();
        let previous_capabilities = model.capabilities.clone();
        let previous_memory_requirement = model.memory_requirement_mb;
        let has_projector = projector_path.is_some();
        model.vision_projector_location = projector_path;
        model.memory_requirement_mb = total_size.div_ceil(1_048_576);
        if has_projector {
            if !model
                .capabilities
                .iter()
                .any(|capability| capability == "vision")
            {
                model.capabilities.push("vision".to_owned());
            }
        } else {
            model
                .capabilities
                .retain(|capability| capability != "vision");
        }
        if let Err(error) = self.persist(&models) {
            if let Some(model) = models.get_mut(id) {
                model.vision_projector_location = previous_projector;
                model.capabilities = previous_capabilities;
                model.memory_requirement_mb = previous_memory_requirement;
            }
            return Err(error);
        }
        Ok(())
    }

    /// List metadata in stable identifier order.
    #[must_use]
    pub fn list(&self) -> Vec<ModelMetadata> {
        self.models
            .lock()
            .map(|models| models.values().cloned().collect())
            .unwrap_or_default()
    }

    fn is_vision_projector_artifact(&self, path: &Path) -> Result<bool, RuntimeError> {
        let models = self.lock_models()?;
        Ok(models.values().any(|model| {
            model
                .vision_projector_location
                .as_deref()
                .is_some_and(|projector| Path::new(projector) == path)
        }))
    }

    /// Inspect one model by identifier.
    #[must_use]
    pub fn inspect(&self, id: &str) -> Option<ModelMetadata> {
        self.models
            .lock()
            .ok()
            .and_then(|models| models.get(id).cloned())
    }

    /// Update metadata status and publish the corresponding lifecycle event.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is unknown, the registry is unavailable,
    /// or persistence fails.
    pub fn set_status(&self, id: &str, status: &ModelStatus) -> Result<(), RuntimeError> {
        let mut models = self.lock_models()?;
        let model = models
            .get_mut(id)
            .ok_or_else(|| RuntimeError::ModelNotFound(id.to_owned()))?;
        let previous = model.status.clone();
        model.status = status.clone();
        if let Err(error) = self.persist(&models) {
            if let Some(model) = models.get_mut(id) {
                model.status = previous;
            }
            return Err(error);
        }
        drop(models);
        let event = match status {
            ModelStatus::Loaded => RuntimeEvent::ModelLoaded(id.to_owned()),
            ModelStatus::Registered | ModelStatus::Unavailable | ModelStatus::Failed => {
                RuntimeEvent::ModelUnloaded(id.to_owned())
            }
            ModelStatus::Loading | ModelStatus::Unloading => {
                RuntimeEvent::ModelLoading(id.to_owned())
            }
        };
        self.bus.publish(&event);
        Ok(())
    }

    fn lock_models(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, ModelMetadata>>, RuntimeError> {
        self.models
            .lock()
            .map_err(|_| RuntimeError::Persistence("registry lock poisoned".to_owned()))
    }

    fn persist(&self, models: &BTreeMap<String, ModelMetadata>) -> Result<(), RuntimeError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
        }
        let temporary = path.with_extension("tmp");
        let contents = models
            .values()
            .map(encode_model)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&temporary, contents)
            .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
        fs::rename(temporary, path).map_err(|error| RuntimeError::Persistence(error.to_string()))
    }
}

/// Minimal model catalog contract used by the core runtime.
pub trait ModelCatalog: Send + Sync {
    /// Return model metadata.
    fn list(&self) -> Vec<ModelMetadata>;
}

impl ModelCatalog for ModelRegistry {
    fn list(&self) -> Vec<ModelMetadata> {
        Self::list(self)
    }
}

/// Acquires GGUF model artifacts from local paths or HTTP(S) sources.
///
/// Remote acquisition deliberately delegates network transfer to the user's
/// explicit `curl` installation. The artifact is first written to a temporary
/// file in the destination directory, optionally verified, and then renamed
/// into place so readers never observe a partial model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModelAcquirer;

impl ModelAcquirer {
    /// Normalize a SHA-256 digest to lowercase hexadecimal.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied value is not a 64-character
    /// hexadecimal SHA-256 digest.
    pub fn normalize_sha256(checksum: &str) -> Result<String, RuntimeError> {
        normalize_checksum(checksum)
    }

    /// Verify a file against an expected SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or its digest does not
    /// match the expected value.
    pub fn verify_sha256(path: &Path, expected: &str) -> Result<String, RuntimeError> {
        let expected = normalize_checksum(expected)?;
        let actual = sha256_file(path)?;
        if actual != expected {
            return Err(RuntimeError::InvalidModel(format!(
                "SHA-256 checksum mismatch for {}: expected {expected}, got {actual}",
                path.display()
            )));
        }
        Ok(actual)
    }

    /// Acquire a GGUF artifact and return metadata for the installed file.
    ///
    /// `source` may be a filesystem path, a `file://` URL, or an HTTP(S) URL.
    /// The model ID is derived from the source filename when it is omitted.
    /// When `checksum` is supplied, it must be a SHA-256 hex digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is unsupported, `curl` is unavailable
    /// or fails, the artifact is not a GGUF file, the checksum does not match,
    /// or the destination cannot be written.
    pub fn acquire(
        source: &str,
        destination: &Path,
        model_id: Option<&str>,
        checksum: Option<&str>,
    ) -> Result<ModelMetadata, RuntimeError> {
        let source_name = source_file_name(source)?;
        if !source_name.to_ascii_lowercase().ends_with(".gguf") {
            return Err(RuntimeError::InvalidModel(format!(
                "model artifact must have a .gguf extension: {source}"
            )));
        }
        let id = model_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(
                || source_name[..source_name.len() - 5].to_owned(),
                str::to_owned,
            );
        if id.is_empty() || id.contains('/') || id.contains('\\') {
            return Err(RuntimeError::InvalidModel(format!(
                "invalid model identifier: {id}"
            )));
        }

        fs::create_dir_all(destination)
            .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
        let target = destination.join(&source_name);
        let temporary = destination.join(format!(".{source_name}.part"));

        if target.is_file() {
            if let Some(expected) = checksum {
                let actual_checksum = Self::verify_sha256(&target, expected)?;
                let bytes = fs::metadata(&target)
                    .map_err(|error| RuntimeError::Persistence(error.to_string()))?
                    .len();
                return Ok(ModelMetadata {
                    id,
                    name: source_name,
                    family: "unknown".to_owned(),
                    backend: "llama.cpp".to_owned(),
                    quantization: "unknown".to_owned(),
                    context_window: 0,
                    memory_requirement_mb: bytes.div_ceil(1_048_576),
                    capabilities: vec!["text".to_owned()],
                    status: ModelStatus::Registered,
                    checksum: Some(actual_checksum),
                    vision_projector_location: None,
                    location: target.display().to_string(),
                });
            }
        }

        let result = (|| {
            acquire_to(source, &temporary)?;
            let actual_checksum = {
                let actual = sha256_file(&temporary)?;
                if let Some(expected) = checksum {
                    let expected = normalize_checksum(expected)?;
                    if actual != expected {
                        let _ = fs::remove_file(&temporary);
                        return Err(RuntimeError::InvalidModel(format!(
                        "SHA-256 checksum mismatch for {source_name}: expected {expected}, got {actual}"
                    )));
                    }
                }
                Some(actual)
            };
            fs::rename(&temporary, &target)
                .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
            let bytes = fs::metadata(&target)
                .map_err(|error| RuntimeError::Persistence(error.to_string()))?
                .len();
            Ok(ModelMetadata {
                id,
                name: source_name,
                family: "unknown".to_owned(),
                backend: "llama.cpp".to_owned(),
                quantization: "unknown".to_owned(),
                context_window: 0,
                memory_requirement_mb: bytes.div_ceil(1_048_576),
                capabilities: vec!["text".to_owned()],
                status: ModelStatus::Registered,
                checksum: actual_checksum,
                vision_projector_location: None,
                location: target.display().to_string(),
            })
        })();
        result
    }
}

#[allow(clippy::large_stack_arrays)]
fn acquire_to(source: &str, temporary: &Path) -> Result<(), RuntimeError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let status = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--retry",
                "3",
                "--retry-all-errors",
                "--continue-at",
                "-",
                "--progress-bar",
                "--show-error",
                "--output",
            ])
            .arg(temporary)
            .arg(source)
            .status()
            .map_err(|error| RuntimeError::Persistence(format!("failed to start curl: {error}")))?;
        if !status.success() {
            return Err(RuntimeError::Persistence(format!(
                "curl failed for {source} with status {status}; partial download retained at {} for recovery",
                temporary.display()
            )));
        }
        return Ok(());
    }
    let path = if let Some(path) = source.strip_prefix("file://") {
        let path = path
            .strip_prefix("localhost")
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .unwrap_or(path);
        #[cfg(windows)]
        let path = path
            .strip_prefix('/')
            .filter(|path| path.as_bytes().get(1) == Some(&b':'))
            .unwrap_or(path);
        PathBuf::from(path)
    } else if source.contains("://") {
        return Err(RuntimeError::InvalidModel(format!(
            "unsupported model source: {source}"
        )));
    } else {
        PathBuf::from(source)
    };
    let source_size = fs::metadata(&path)
        .map_err(|error| {
            RuntimeError::Persistence(format!("failed to inspect {}: {error}", path.display()))
        })?
        .len();
    let partial_size = temporary
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if partial_size > source_size {
        fs::remove_file(temporary).map_err(|error| {
            RuntimeError::Persistence(format!("failed to reset partial model: {error}"))
        })?;
    }
    let mut input = File::open(&path).map_err(|error| {
        RuntimeError::Persistence(format!("failed to open {}: {error}", path.display()))
    })?;
    let partial_size = temporary
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    input.seek(SeekFrom::Start(partial_size)).map_err(|error| {
        RuntimeError::Persistence(format!("failed to seek model source: {error}"))
    })?;
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(temporary)
        .map_err(|error| {
            RuntimeError::Persistence(format!("failed to open partial model: {error}"))
        })?;
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|error| {
            RuntimeError::Persistence(format!("failed to read {}: {error}", path.display()))
        })?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).map_err(|error| {
            RuntimeError::Persistence(format!("failed to write partial model: {error}"))
        })?;
        eprint!(
            "\rmodel download: {}/{} bytes",
            output
                .metadata()
                .map(|metadata| metadata.len())
                .unwrap_or(0),
            source_size
        );
    }
    eprintln!();
    output.flush().map_err(|error| {
        RuntimeError::Persistence(format!("failed to flush partial model: {error}"))
    })
}

fn source_file_name(source: &str) -> Result<String, RuntimeError> {
    let source = source.split(['?', '#']).next().unwrap_or(source);
    let path = source
        .strip_prefix("file://")
        .or_else(|| source.strip_prefix("http://"))
        .or_else(|| source.strip_prefix("https://"))
        .unwrap_or(source);
    let name = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| RuntimeError::InvalidModel(format!("source has no filename: {source}")))?;
    Ok(name.to_owned())
}

fn normalize_checksum(value: &str) -> Result<String, RuntimeError> {
    let value = value.trim().strip_prefix("sha256:").unwrap_or(value).trim();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeError::InvalidModel(
            "SHA-256 checksum must be 64 hexadecimal characters".to_owned(),
        ));
    }
    Ok(value.to_ascii_lowercase())
}

#[allow(clippy::large_stack_arrays)]
fn sha256_file(path: &Path) -> Result<String, RuntimeError> {
    let mut file =
        File::open(path).map_err(|error| RuntimeError::Persistence(error.to_string()))?;
    let mut hasher = Sha256::default();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finish())
}

#[derive(Clone, Debug)]
struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length: u64,
}

#[allow(clippy::unreadable_literal)]
impl Default for Sha256 {
    fn default() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffered: 0,
            length: 0,
        }
    }
}

#[allow(
    clippy::format_collect,
    clippy::many_single_char_names,
    clippy::unreadable_literal
)]
impl Sha256 {
    fn update(&mut self, bytes: &[u8]) {
        self.length += bytes.len() as u64;
        let mut bytes = bytes;
        if self.buffered != 0 {
            let take = (64 - self.buffered).min(bytes.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&bytes[..take]);
            self.buffered += take;
            bytes = &bytes[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
        for chunk in bytes.chunks_exact(64) {
            self.compress(chunk);
        }
        self.buffered = bytes.chunks_exact(64).remainder().len();
        self.buffer[..self.buffered].copy_from_slice(&bytes[bytes.len() - self.buffered..]);
    }

    fn finish(mut self) -> String {
        let bit_length = self.length * 8;
        self.buffer[self.buffered] = 0x80;
        self.buffered += 1;
        if self.buffered > 56 {
            self.buffer[self.buffered..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffered = 0;
        }
        self.buffer[self.buffered..56].fill(0);
        self.buffer[56..].copy_from_slice(&bit_length.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);
        self.state
            .iter()
            .flat_map(|word| word.to_be_bytes())
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn compress(&mut self, block: &[u8]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes([
                block[index * 4],
                block[index * 4 + 1],
                block[index * 4 + 2],
                block[index * 4 + 3],
            ]);
        }
        for index in 16..64 {
            let value = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let other = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(value)
                .wrapping_add(words[index - 7])
                .wrapping_add(other);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for index in 0..64 {
            let choose = (e & f) ^ (!e & g);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp1 = h
                .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let temp2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22))
                .wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Empty catalog retained for the lower-level foundation runtime.
#[derive(Clone, Debug, Default)]
pub struct EmptyModelCatalog;

/// Configurable GGUF model discovery service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDiscovery {
    directories: Vec<PathBuf>,
}

impl ModelDiscovery {
    /// Construct discovery for explicit directories.
    #[must_use]
    pub const fn new(directories: Vec<PathBuf>) -> Self {
        Self { directories }
    }

    /// Return the default user and workspace model directories.
    #[must_use]
    pub fn default_directories() -> Vec<PathBuf> {
        let mut directories = Vec::new();
        let mut add_directory = |directory: PathBuf| {
            if !directories.contains(&directory) {
                directories.push(directory);
            }
        };
        if let Some(talvik_home) = crate::config::talvik_home_directory() {
            add_directory(talvik_home.join("models"));
            let user_home = crate::config::home_directory().unwrap_or_default();
            add_directory(user_home.join(".local/share/intelligent-runtime/models"));
            add_directory(user_home.join("Models"));
            add_directory(user_home.join(".cache/huggingface/hub"));
            add_directory(user_home.join(".cache/modelscope/hub"));
        }
        if let Some(hf_home) = std::env::var_os("HF_HOME") {
            add_directory(PathBuf::from(hf_home).join("hub"));
        }
        if let Some(transformers_cache) = std::env::var_os("TRANSFORMERS_CACHE") {
            add_directory(PathBuf::from(transformers_cache));
        }
        if let Some(xdg_cache_home) = std::env::var_os("XDG_CACHE_HOME") {
            let xdg_cache_home = PathBuf::from(xdg_cache_home);
            add_directory(xdg_cache_home.join("huggingface/hub"));
            add_directory(xdg_cache_home.join("modelscope/hub"));
        }
        directories
    }

    /// Discover GGUF files and register metadata without loading models.
    ///
    /// # Errors
    ///
    /// Returns an error when a discovery directory cannot be read or metadata
    /// cannot be registered.
    pub fn discover(
        &self,
        registry: &ModelRegistry,
        backend: &str,
    ) -> Result<Vec<ModelMetadata>, RuntimeError> {
        let mut discovered = Vec::new();
        Self::prune_duplicate_artifacts(registry)?;
        for directory in &self.directories {
            if directory.is_dir() {
                Self::visit(directory, registry, backend, &mut discovered)?;
            }
        }
        Ok(discovered)
    }

    fn same_artifact(left: &str, right: &str) -> bool {
        left.replace('\\', "/")
            .eq_ignore_ascii_case(&right.replace('\\', "/"))
    }

    /// Drop registry entries that point at the same artifact as a better entry
    /// (one with a paired projector, else the first by ID).
    fn prune_duplicate_artifacts(registry: &ModelRegistry) -> Result<(), RuntimeError> {
        let models = registry.list();
        for model in &models {
            let duplicate_of_better = models.iter().any(|other| {
                other.id != model.id
                    && Self::same_artifact(&other.location, &model.location)
                    && model.status == ModelStatus::Registered
                    && other.vision_projector_location.is_some()
                    && model.vision_projector_location.is_none()
            });
            if duplicate_of_better {
                registry.remove(&model.id)?;
            }
        }
        Ok(())
    }

    fn visit(
        directory: &Path,
        registry: &ModelRegistry,
        backend: &str,
        discovered: &mut Vec<ModelMetadata>,
    ) -> Result<(), RuntimeError> {
        let entries = fs::read_dir(directory)
            .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
        for entry in entries {
            let path = entry
                .map_err(|error| RuntimeError::Persistence(error.to_string()))?
                .path();
            if path.is_dir() {
                Self::visit(&path, registry, backend, discovered)?;
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf"))
            {
                if registry.is_vision_projector_artifact(&path)?
                    || path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.to_ascii_lowercase().starts_with("mmproj"))
                {
                    continue;
                }
                let id = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .ok_or_else(|| RuntimeError::InvalidModel(path.display().to_string()))?
                    .to_owned();
                let location = path.display().to_string();
                if registry.inspect(&id).is_some()
                    || registry
                        .list()
                        .iter()
                        .any(|model| Self::same_artifact(&model.location, &location))
                {
                    continue;
                }
                let bytes = path
                    .metadata()
                    .map(|metadata| metadata.len())
                    .unwrap_or_default();
                let model = ModelMetadata {
                    id: id.clone(),
                    name: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&id)
                        .to_owned(),
                    family: "unknown".to_owned(),
                    backend: backend.to_owned(),
                    quantization: "unknown".to_owned(),
                    context_window: 0,
                    memory_requirement_mb: bytes.div_ceil(1_048_576),
                    capabilities: vec!["text".to_owned()],
                    status: ModelStatus::Registered,
                    checksum: None,
                    vision_projector_location: None,
                    location: path.display().to_string(),
                };
                registry.register(model.clone())?;
                discovered.push(model);
            }
        }
        Ok(())
    }
}

impl ModelCatalog for EmptyModelCatalog {
    fn list(&self) -> Vec<ModelMetadata> {
        Vec::new()
    }
}

fn encode_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\p")
        .replace('\n', "\\n")
}

fn decode_fields(line: &str) -> Result<Vec<String>, RuntimeError> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            field.push(match character {
                'n' => '\n',
                'p' => '|',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '|' {
            fields.push(field);
            field = String::new();
        } else {
            field.push(character);
        }
    }
    if escaped {
        return Err(RuntimeError::Persistence("unterminated escape".to_owned()));
    }
    fields.push(field);
    Ok(fields)
}

fn encode_model(model: &ModelMetadata) -> String {
    let capabilities = model.capabilities.join(",");
    let context_window = model.context_window.to_string();
    let memory_requirement = model.memory_requirement_mb.to_string();
    [
        model.id.as_str(),
        model.name.as_str(),
        model.family.as_str(),
        model.backend.as_str(),
        model.quantization.as_str(),
        context_window.as_str(),
        memory_requirement.as_str(),
        capabilities.as_str(),
        model.status.as_str(),
        model.checksum.as_deref().unwrap_or(""),
        model.vision_projector_location.as_deref().unwrap_or(""),
        model.location.as_str(),
    ]
    .iter()
    .map(|field| encode_field(field))
    .collect::<Vec<_>>()
    .join("|")
}

fn decode_registry(contents: &str) -> Result<BTreeMap<String, ModelMetadata>, RuntimeError> {
    let mut models = BTreeMap::new();
    for line in contents.lines().filter(|line| !line.is_empty()) {
        let fields = decode_fields(line)?;
        if fields.len() != 11 && fields.len() != 12 {
            return Err(RuntimeError::Persistence("invalid model record".to_owned()));
        }
        let model = ModelMetadata {
            id: fields[0].clone(),
            name: fields[1].clone(),
            family: fields[2].clone(),
            backend: fields[3].clone(),
            quantization: fields[4].clone(),
            context_window: fields[5]
                .parse()
                .map_err(|_| RuntimeError::Persistence("invalid context window".to_owned()))?,
            memory_requirement_mb: fields[6]
                .parse()
                .map_err(|_| RuntimeError::Persistence("invalid memory requirement".to_owned()))?,
            capabilities: if fields[7].is_empty() {
                Vec::new()
            } else {
                fields[7].split(',').map(str::to_owned).collect()
            },
            status: ModelStatus::parse(&fields[8]),
            checksum: (!fields[9].is_empty()).then(|| fields[9].clone()),
            vision_projector_location: fields
                .get(10)
                .filter(|_| fields.len() == 12)
                .filter(|value| !value.is_empty())
                .cloned(),
            location: fields[if fields.len() == 12 { 11 } else { 10 }].clone(),
        };
        models.insert(model.id.clone(), model);
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::{
        sha256_file, source_file_name, ModelAcquirer, ModelDiscovery, ModelMetadata, ModelRegistry,
        ModelStatus, Sha256,
    };
    use oid_shared::{EventBus, RuntimeEvent};
    use std::path::PathBuf;

    fn temporary_directory(label: &str) -> PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("oid-{label}-{}-{timestamp}", std::process::id()))
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::default();
        hasher.update(bytes);
        hasher.finish()
    }

    fn model() -> ModelMetadata {
        ModelMetadata {
            id: "demo".to_owned(),
            name: "Demo Model".to_owned(),
            family: "demo".to_owned(),
            backend: "mock".to_owned(),
            quantization: "none".to_owned(),
            context_window: 1024,
            memory_requirement_mb: 512,
            capabilities: vec!["text".to_owned()],
            status: ModelStatus::Registered,
            checksum: Some("abc".to_owned()),
            vision_projector_location: None,
            location: "/models/demo".to_owned(),
        }
    }

    #[test]
    fn registers_inspects_and_removes_metadata() {
        let bus = EventBus::new();
        let receiver = bus.subscribe();
        let registry = ModelRegistry::in_memory(bus);
        registry.register(model()).expect("register");
        assert_eq!(
            registry.inspect("demo").expect("inspect").name,
            "Demo Model"
        );
        registry.remove("demo").expect("remove");
        assert!(matches!(
            receiver.try_iter().next(),
            Some(RuntimeEvent::ModelRegistered(_))
        ));
        assert!(registry.list().is_empty());
    }

    #[test]
    fn persists_metadata_across_registry_instances() {
        let path =
            std::env::temp_dir().join(format!("oid-model-registry-{}.db", std::process::id()));
        let bus = EventBus::new();
        let registry = ModelRegistry::open(&path, bus.clone()).expect("open registry");
        registry.register(model()).expect("persist model");
        let reopened = ModelRegistry::open(&path, bus).expect("reopen registry");
        assert_eq!(
            reopened.inspect("demo").expect("persisted model").id,
            "demo"
        );
        std::fs::remove_file(path).expect("remove test registry");
    }

    #[test]
    fn persists_explicit_projector_pairs_and_reads_legacy_records() {
        let path = temporary_directory("vision-registry").join("models.db");
        let model_artifact = path.with_file_name("demo.gguf");
        let projector_artifact = path.with_file_name("demo-mmproj.gguf");
        std::fs::create_dir_all(path.parent().expect("registry parent"))
            .expect("create registry directory");
        std::fs::write(&model_artifact, b"GGUF model").expect("write model artifact");
        std::fs::write(&projector_artifact, b"GGUF projector").expect("write projector artifact");
        std::fs::write(
            &path,
            "legacy|Legacy|demo|mock|none|1024|512|text|registered||/models/legacy\n",
        )
        .expect("write legacy record");
        let registry = ModelRegistry::open(&path, EventBus::new()).expect("read legacy registry");
        assert_eq!(
            registry
                .inspect("legacy")
                .expect("legacy model")
                .vision_projector_location,
            None
        );
        let mut model = model();
        model.location = model_artifact.display().to_string();
        registry
            .register(model)
            .expect("register model with no projector");
        registry
            .set_vision_projector("demo", Some(projector_artifact.display().to_string()))
            .expect("pair projector");

        let reopened =
            ModelRegistry::open(&path, EventBus::new()).expect("reopen projector registry");
        let configured = reopened.inspect("demo").expect("configured model");
        assert_eq!(
            configured.vision_projector_location.as_deref(),
            Some(projector_artifact.to_string_lossy().as_ref())
        );
        assert!(configured
            .capabilities
            .iter()
            .any(|capability| capability == "vision"));

        reopened
            .set_vision_projector("demo", None)
            .expect("clear projector");
        assert_eq!(
            reopened
                .inspect("demo")
                .expect("cleared model")
                .vision_projector_location,
            None
        );
        assert_eq!(
            reopened
                .inspect("demo")
                .expect("cleared model")
                .memory_requirement_mb,
            std::fs::metadata(&model_artifact)
                .expect("model file size")
                .len()
                .div_ceil(1_048_576)
        );
        std::fs::remove_dir_all(path.parent().expect("registry parent"))
            .expect("remove registry directory");
    }

    #[test]
    fn discovers_only_gguf_files_and_registers_metadata() {
        let directory =
            std::env::temp_dir().join(format!("oid-model-discovery-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create discovery directory");
        std::fs::write(directory.join("qwen3.gguf"), [0_u8; 8]).expect("write gguf");
        std::fs::write(directory.join("notes.txt"), "not a model").expect("write note");
        let bus = EventBus::new();
        let registry = ModelRegistry::in_memory(bus);
        let discovered = ModelDiscovery::new(vec![directory.clone()])
            .discover(&registry, "llama.cpp")
            .expect("discover models");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].id, "qwen3");
        assert_eq!(registry.list().len(), 1);
        std::fs::remove_dir_all(directory).expect("remove discovery directory");
    }

    #[test]
    fn discovery_does_not_register_a_paired_vision_projector_as_a_model() {
        let directory = temporary_directory("vision-projector-discovery");
        std::fs::create_dir_all(&directory).expect("create discovery directory");
        let model_path = directory.join("demo.gguf");
        let projector_path = directory.join("mmproj-demo.gguf");
        std::fs::write(&model_path, b"GGUF model").expect("write model fixture");
        std::fs::write(&projector_path, b"GGUF projector").expect("write projector fixture");

        let bus = EventBus::new();
        let registry = ModelRegistry::in_memory(bus);
        let mut registered = model();
        registered.location = model_path.display().to_string();
        registered.vision_projector_location = Some(projector_path.display().to_string());
        registered.capabilities.push("vision".to_owned());
        registry
            .register(registered)
            .expect("register paired model");

        let discovered = ModelDiscovery::new(vec![directory.clone()])
            .discover(&registry, "llama.cpp")
            .expect("discover model directory");
        assert!(discovered.is_empty());
        assert_eq!(registry.list().len(), 1);
        std::fs::remove_dir_all(directory).expect("remove discovery directory");
    }

    #[test]
    fn discovery_skips_artifacts_registered_under_another_id_and_prunes_stubs() {
        let directory = temporary_directory("alias-discovery");
        std::fs::create_dir_all(&directory).expect("create discovery directory");
        let model_path = directory.join("Demo-Q4.gguf");
        std::fs::write(&model_path, b"GGUF model").expect("write model fixture");
        let location = model_path.display().to_string();

        let registry = ModelRegistry::in_memory(EventBus::new());
        let mut alias = model();
        alias.id = "demo-q4".to_owned();
        alias.location = location.clone();
        registry.register(alias).expect("register alias");
        let discovery = ModelDiscovery::new(vec![directory.clone()]);
        assert!(discovery
            .discover(&registry, "llama.cpp")
            .expect("discover")
            .is_empty());
        assert_eq!(registry.list().len(), 1);

        let mut stub = model();
        stub.id = "Demo-Q4".to_owned();
        stub.location = location;
        registry.register(stub).expect("register stub");
        let mut paired = registry.inspect("demo-q4").expect("alias");
        paired.vision_projector_location = Some("mmproj-demo.gguf".to_owned());
        registry.remove("demo-q4").expect("remove");
        registry.register(paired).expect("register paired");
        discovery.discover(&registry, "llama.cpp").expect("discover");
        let ids: Vec<_> = registry.list().into_iter().map(|model| model.id).collect();
        assert_eq!(ids, ["demo-q4"]);
        std::fs::remove_dir_all(directory).expect("remove discovery directory");
    }

    #[test]
    fn discovery_skips_unpaired_mmproj_files() {
        let directory = temporary_directory("unpaired-projector-discovery");
        std::fs::create_dir_all(&directory).expect("create discovery directory");
        std::fs::write(directory.join("demo.gguf"), b"GGUF model").expect("write model fixture");
        std::fs::write(directory.join("mmproj-demo.gguf"), b"GGUF projector")
            .expect("write projector fixture");

        let registry = ModelRegistry::in_memory(EventBus::new());
        let discovered = ModelDiscovery::new(vec![directory.clone()])
            .discover(&registry, "llama.cpp")
            .expect("discover model directory");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].id, "demo");
        assert_eq!(registry.list().len(), 1);
        std::fs::remove_dir_all(directory).expect("remove discovery directory");
    }

    #[test]
    fn acquires_local_gguf_atomically_and_derives_metadata() {
        let source_directory = temporary_directory("acquire-source");
        let destination = temporary_directory("acquire-destination");
        std::fs::create_dir_all(&source_directory).expect("create source directory");
        let source = source_directory.join("tiny-model.gguf");
        std::fs::write(&source, b"GGUF").expect("write source");

        let metadata = ModelAcquirer::acquire(
            source.to_str().expect("source path"),
            &destination,
            None,
            Some("sha256:b83633aa785344791618f2fddf131b010ea04912a60430760b070bad293f65bd"),
        )
        .expect("acquire local model");

        assert_eq!(metadata.id, "tiny-model");
        assert_eq!(metadata.name, "tiny-model.gguf");
        assert_eq!(metadata.backend, "llama.cpp");
        assert_eq!(
            metadata.checksum.as_deref(),
            Some("b83633aa785344791618f2fddf131b010ea04912a60430760b070bad293f65bd")
        );
        assert_eq!(
            std::fs::read(destination.join("tiny-model.gguf")).expect("read target"),
            b"GGUF"
        );
        assert!(!destination.join(".tiny-model.gguf").exists());
        let reused = ModelAcquirer::acquire(
            "https://example.invalid/tiny-model.gguf",
            &destination,
            None,
            Some("b83633aa785344791618f2fddf131b010ea04912a60430760b070bad293f65bd"),
        )
        .expect("reuse verified artifact already in destination");
        assert_eq!(reused.location, metadata.location);

        std::fs::remove_dir_all(source_directory).expect("remove source directory");
        std::fs::remove_dir_all(destination).expect("remove destination directory");
    }

    #[test]
    fn verifies_sha256_of_existing_artifacts() {
        let directory = temporary_directory("verify-checksum");
        std::fs::create_dir_all(&directory).expect("create verification directory");
        let artifact = directory.join("demo.gguf");
        std::fs::write(&artifact, b"GGUF").expect("write artifact");

        assert_eq!(
            ModelAcquirer::verify_sha256(
                &artifact,
                "sha256:b83633aa785344791618f2fddf131b010ea04912a60430760b070bad293f65bd"
            )
            .expect("verify matching checksum"),
            "b83633aa785344791618f2fddf131b010ea04912a60430760b070bad293f65bd"
        );
        assert!(ModelAcquirer::verify_sha256(&artifact, &"0".repeat(64))
            .expect_err("reject mismatched checksum")
            .to_string()
            .contains("checksum mismatch"));
        std::fs::remove_dir_all(directory).expect("remove verification directory");
    }

    #[test]
    fn acquires_file_url_and_preserves_explicit_id() {
        let source_directory = temporary_directory("file-url-source");
        let destination = temporary_directory("file-url-destination");
        std::fs::create_dir_all(&source_directory).expect("create source directory");
        let source = source_directory.join("model.GGUF");
        std::fs::write(&source, b"model bytes").expect("write source");
        let source_path = source.display().to_string().replace('\\', "/");
        let source_url = if cfg!(windows) {
            format!("file:///{source_path}")
        } else {
            format!("file://{source_path}")
        };

        let metadata = ModelAcquirer::acquire(&source_url, &destination, Some("custom-id"), None)
            .expect("acquire file URL");

        assert_eq!(metadata.id, "custom-id");
        assert_eq!(metadata.name, "model.GGUF");
        assert!(destination.join("model.GGUF").is_file());
        assert_eq!(
            metadata.checksum.as_deref(),
            Some("9cb7487000bc86ac36ce83c4acfabe8878552be99572a6770f65ab1d048a5c48")
        );

        std::fs::remove_dir_all(source_directory).expect("remove source directory");
        std::fs::remove_dir_all(destination).expect("remove destination directory");
    }

    #[test]
    fn derives_filename_from_windows_paths_and_file_urls() {
        assert_eq!(
            source_file_name(r"C:\models\tiny-model.gguf").expect("Windows path filename"),
            "tiny-model.gguf"
        );
        assert_eq!(
            source_file_name(r"file://C:\models\tiny-model.gguf")
                .expect("Windows file URL filename"),
            "tiny-model.gguf"
        );
    }

    #[test]
    fn resumes_local_partial_artifact_and_records_observed_checksum() {
        let source_directory = temporary_directory("resume-source");
        let destination = temporary_directory("resume-destination");
        std::fs::create_dir_all(&source_directory).expect("create source directory");
        let source = source_directory.join("resume.gguf");
        std::fs::write(&source, b"GGUF-resumable-model").expect("write source");
        std::fs::create_dir_all(&destination).expect("create destination directory");
        std::fs::write(destination.join(".resume.gguf.part"), b"GGUF-resumable-")
            .expect("write partial artifact");

        let metadata = ModelAcquirer::acquire(
            source.to_str().expect("source path"),
            &destination,
            None,
            None,
        )
        .expect("resume local model");

        assert_eq!(
            std::fs::read(destination.join("resume.gguf")).expect("read target"),
            b"GGUF-resumable-model"
        );
        assert!(metadata.checksum.is_some());
        assert!(!destination.join(".resume.gguf.part").exists());

        std::fs::remove_dir_all(source_directory).expect("remove source directory");
        std::fs::remove_dir_all(destination).expect("remove destination directory");
    }

    #[test]
    fn resumes_interrupted_http_transfer_with_range_request() {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;
        use std::thread;

        let bytes = b"GGUF-http-resumable-model".to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("server address");
        let server_bytes = bytes.clone();
        let server = thread::spawn(move || {
            for attempt in 0..5 {
                let (mut stream, _) = listener.accept().expect("accept download");
                let mut request = [0_u8; 1024];
                let size = stream.read(&mut request).expect("read request");
                let request = String::from_utf8_lossy(&request[..size]);
                if attempt < 4 {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        server_bytes.len()
                    )
                    .expect("write initial headers");
                    stream
                        .write_all(&server_bytes[..4])
                        .expect("write interrupted body");
                } else {
                    assert!(request.contains("Range: bytes=4-"));
                    write!(
                        stream,
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes 4-{}/{}\r\nConnection: close\r\n\r\n",
                        server_bytes.len() - 4,
                        server_bytes.len() - 1,
                        server_bytes.len()
                    )
                    .expect("write range headers");
                    stream
                        .write_all(&server_bytes[4..])
                        .expect("write range body");
                }
            }
        });

        let destination = temporary_directory("http-resume-destination");
        let source = format!("http://{address}/resume.gguf");
        let expected = sha256_bytes(&bytes);
        let interrupted = ModelAcquirer::acquire(&source, &destination, None, Some(&expected))
            .expect_err("first HTTP attempt should be interrupted");
        assert!(interrupted
            .to_string()
            .contains("partial download retained"));
        let metadata = ModelAcquirer::acquire(&source, &destination, None, Some(&expected))
            .expect("resume interrupted HTTP model");
        server.join().expect("server thread");
        assert_eq!(metadata.checksum.as_deref(), Some(expected.as_str()));
        assert_eq!(
            std::fs::read(destination.join("resume.gguf")).expect("read HTTP target"),
            bytes
        );
        std::fs::remove_dir_all(destination).expect("remove destination directory");
    }

    #[test]
    fn rejects_checksum_without_leaving_partial_artifact() {
        let source_directory = temporary_directory("checksum-source");
        let destination = temporary_directory("checksum-destination");
        std::fs::create_dir_all(&source_directory).expect("create source directory");
        let source = source_directory.join("bad.gguf");
        std::fs::write(&source, b"wrong").expect("write source");

        let error = ModelAcquirer::acquire(
            source.to_str().expect("source path"),
            &destination,
            None,
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
        )
        .expect_err("checksum should fail");
        assert!(matches!(error, oid_shared::RuntimeError::InvalidModel(_)));
        assert!(!destination.join("bad.gguf").exists());
        assert!(std::fs::read_dir(&destination)
            .expect("read destination")
            .next()
            .is_none());

        std::fs::remove_dir_all(source_directory).expect("remove source directory");
        std::fs::remove_dir_all(destination).expect("remove destination directory");
    }

    #[test]
    fn sha256_matches_known_vector() {
        let directory = temporary_directory("hash");
        std::fs::create_dir_all(&directory).expect("create hash directory");
        let path = directory.join("input");
        std::fs::write(&path, b"abc").expect("write input");
        assert_eq!(
            sha256_file(&path).expect("hash input"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        std::fs::remove_dir_all(directory).expect("remove hash directory");
    }
}
