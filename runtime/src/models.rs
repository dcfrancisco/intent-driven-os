//! Persistent model metadata registry.

use oid_shared::{EventBus, RuntimeError, RuntimeEvent};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
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
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            directories.push(home.join(".local/share/intelligent-runtime/models"));
            directories.push(home.join("Models"));
        }
        directories.push(PathBuf::from("models"));
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
    use super::{ModelDiscovery, ModelMetadata, ModelRegistry, ModelStatus};
    use oid_shared::{EventBus, RuntimeEvent};

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
}
