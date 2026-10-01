//! Persistent model metadata registry.

use oid_shared::{EventBus, RuntimeError, RuntimeEvent};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static ACQUISITION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    /// Returns an error for duplicate IDs, unavailable registry state, or
    /// persistence failure.
    pub fn register(&self, model: ModelMetadata) -> Result<(), RuntimeError> {
        let id = model.id.clone();
        let mut models = self.lock_models()?;
        if models.contains_key(&id) {
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

    /// List metadata in stable identifier order.
    #[must_use]
    pub fn list(&self) -> Vec<ModelMetadata> {
        self.models
            .lock()
            .map(|models| models.values().cloned().collect())
            .unwrap_or_default()
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
            .map(str::to_owned)
            .unwrap_or_else(|| source_name[..source_name.len() - 5].to_owned());
        if id.is_empty() || id.contains('/') || id.contains('\\') {
            return Err(RuntimeError::InvalidModel(format!(
                "invalid model identifier: {id}"
            )));
        }

        fs::create_dir_all(destination)
            .map_err(|error| RuntimeError::Persistence(error.to_string()))?;
        let target = destination.join(&source_name);
        let sequence = ACQUISITION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = destination.join(format!(
            ".{}.{}.{}.part",
            source_name,
            std::process::id(),
            sequence
        ));

        let result = (|| {
            acquire_to(source, &temporary)?;
            let actual_checksum = if let Some(expected) = checksum {
                let actual = sha256_file(&temporary)?;
                let expected = normalize_checksum(expected)?;
                if actual != expected {
                    return Err(RuntimeError::InvalidModel(format!(
                        "SHA-256 checksum mismatch for {source_name}: expected {expected}, got {actual}"
                    )));
                }
                Some(actual)
            } else {
                None
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
                location: target.display().to_string(),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn acquire_to(source: &str, temporary: &Path) -> Result<(), RuntimeError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let status = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--output",
            ])
            .arg(temporary)
            .arg(source)
            .status()
            .map_err(|error| RuntimeError::Persistence(format!("failed to start curl: {error}")))?;
        if !status.success() {
            return Err(RuntimeError::Persistence(format!(
                "curl failed for {source} with status {status}"
            )));
        }
        return Ok(());
    }
    let path = if let Some(path) = source.strip_prefix("file://") {
        if let Some(path) = path.strip_prefix("localhost") {
            PathBuf::from(path)
        } else {
            PathBuf::from(path)
        }
    } else if source.contains("://") {
        return Err(RuntimeError::InvalidModel(format!(
            "unsupported model source: {source}"
        )));
    } else {
        PathBuf::from(source)
    };
    fs::copy(&path, temporary).map(|_| ()).map_err(|error| {
        RuntimeError::Persistence(format!("failed to copy {}: {error}", path.display()))
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
        .trim_end_matches('/')
        .rsplit('/')
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
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            add_directory(home.join(".local/share/intelligent-runtime/models"));
            add_directory(home.join("Models"));
            add_directory(home.join(".cache/huggingface/hub"));
            add_directory(home.join(".cache/modelscope/hub"));
            add_directory(home.join(".ollama/models"));
            add_directory(home.join(".cache/ollama/models"));
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
            add_directory(xdg_cache_home.join("ollama/models"));
        }
        add_directory(PathBuf::from("models"));
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
        for directory in &self.directories {
            if directory.is_dir() {
                Self::visit(directory, registry, backend, &mut discovered)?;
            }
        }
        Ok(discovered)
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
                let id = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .ok_or_else(|| RuntimeError::InvalidModel(path.display().to_string()))?
                    .to_owned();
                if registry.inspect(&id).is_some() {
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
        if fields.len() != 11 {
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
            location: fields[10].clone(),
        };
        models.insert(model.id.clone(), model);
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::{
        sha256_file, ModelAcquirer, ModelDiscovery, ModelMetadata, ModelRegistry, ModelStatus,
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

        std::fs::remove_dir_all(source_directory).expect("remove source directory");
        std::fs::remove_dir_all(destination).expect("remove destination directory");
    }

    #[test]
    fn acquires_file_url_and_preserves_explicit_id() {
        let source_directory = temporary_directory("file-url-source");
        let destination = temporary_directory("file-url-destination");
        std::fs::create_dir_all(&source_directory).expect("create source directory");
        let source = source_directory.join("model.GGUF");
        std::fs::write(&source, b"model bytes").expect("write source");
        let source_url = format!("file://{}", source.display());

        let metadata = ModelAcquirer::acquire(&source_url, &destination, Some("custom-id"), None)
            .expect("acquire file URL");

        assert_eq!(metadata.id, "custom-id");
        assert_eq!(metadata.name, "model.GGUF");
        assert!(destination.join("model.GGUF").is_file());
        assert_eq!(metadata.checksum, None);

        std::fs::remove_dir_all(source_directory).expect("remove source directory");
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
