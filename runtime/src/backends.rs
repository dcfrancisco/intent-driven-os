//! Backend contracts and runtime-owned backend management.

use oid_shared::{EventBus, RuntimeError, RuntimeEvent};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// Stable backend identity and capability metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDescriptor {
    /// Stable machine-readable identifier.
    pub id: String,
    /// Human-readable backend name.
    pub name: String,
    /// Adapter contract version.
    pub version: String,
}

/// Runtime view of a backend registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSummary {
    /// Backend descriptor supplied by the adapter.
    pub descriptor: BackendDescriptor,
    /// Whether the runtime has enabled the adapter.
    pub enabled: bool,
    /// Whether this is the selected active backend.
    pub active: bool,
}

/// Health result returned by a backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendHealth {
    /// Backend is initialized and available.
    Healthy,
    /// Backend is known but not currently available.
    Unavailable,
    /// Backend reported an operational error.
    Error(String),
}

/// Request passed to a backend generation operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationRequest {
    /// Correlation identifier for cancellation and tracing.
    pub request_id: String,
    /// Model identifier selected by the runtime.
    pub model_id: String,
    /// Prompt or input content.
    pub input: String,
}

/// One backend-neutral streamed output item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamChunk {
    /// Correlation identifier for the generation.
    pub request_id: String,
    /// Text payload; empty for a terminal marker.
    pub text: String,
    /// Whether this is the final chunk.
    pub done: bool,
}

/// Placeholder embeddings request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingsRequest {
    /// Model identifier.
    pub model_id: String,
    /// Input text.
    pub input: String,
}

/// Placeholder embeddings result.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingsResult {
    /// Deterministic placeholder vector.
    pub values: Vec<f32>,
}

/// Placeholder tool capability description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolSupport {
    /// Whether the backend can expose tools.
    pub supported: bool,
    /// Human-readable reason when unsupported.
    pub reason: String,
}

/// Common interface implemented by every local or remote inference backend.
///
/// The runtime owns orchestration and policy. Implementations own only the
/// translation to a concrete engine or provider.
pub trait Backend: Send + Sync {
    /// Return stable backend metadata.
    fn descriptor(&self) -> BackendDescriptor;
    /// Initialize the backend process or provider connection.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot initialize its resources.
    fn initialize(&self) -> Result<(), RuntimeError>;
    /// Shut down backend resources.
    ///
    /// # Errors
    ///
    /// Returns an error when backend resources cannot be released.
    fn shutdown(&self) -> Result<(), RuntimeError>;
    /// Return current backend health.
    fn health(&self) -> BackendHealth;
    /// List models visible to this backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot inspect its model source.
    fn list_models(&self) -> Result<Vec<String>, RuntimeError>;
    /// Load a model by runtime model identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot load the requested model.
    fn load_model(&self, model_id: &str) -> Result<(), RuntimeError>;
    /// Unload a model by runtime model identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot unload the requested model.
    fn unload_model(&self, model_id: &str) -> Result<(), RuntimeError>;
    /// Generate backend-neutral streaming chunks.
    ///
    /// # Errors
    ///
    /// Returns an error when generation cannot be admitted or started.
    fn generate_stream(
        &self,
        request: &GenerationRequest,
    ) -> Result<Vec<StreamChunk>, RuntimeError>;
    /// Cancel an active generation.
    ///
    /// # Errors
    ///
    /// Returns an error when the request cannot be cancelled.
    fn cancel_generation(&self, request_id: &str) -> Result<(), RuntimeError>;
    /// Placeholder embeddings operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot service embeddings.
    fn embeddings(&self, request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError>;
    /// Report tool support without exposing backend-native types.
    fn tool_support(&self) -> ToolSupport;
}

#[derive(Default)]
struct BackendManagerState {
    backends: HashMap<String, Box<dyn Backend>>,
    enabled: HashSet<String>,
    active: Option<String>,
}

/// Runtime-owned registry and selector for backend adapters.
#[derive(Clone, Default)]
pub struct BackendManager {
    state: Arc<Mutex<BackendManagerState>>,
    bus: EventBus,
}

impl std::fmt::Debug for BackendManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackendManager")
            .field("backends", &self.discover())
            .field("active", &self.active_backend())
            .finish()
    }
}

impl BackendManager {
    /// Create an empty manager connected to the runtime event bus.
    #[must_use]
    pub fn new(bus: EventBus) -> Self {
        Self {
            state: Arc::new(Mutex::new(BackendManagerState::default())),
            bus,
        }
    }

    /// Register a backend adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when another adapter has the same identifier.
    pub fn register(&self, backend: Box<dyn Backend>) -> Result<(), RuntimeError> {
        let descriptor = backend.descriptor();
        let id = descriptor.id.clone();
        let mut state = self.lock_state()?;
        if state.backends.contains_key(&id) {
            return Err(RuntimeError::BackendAlreadyRegistered(id));
        }
        state.backends.insert(id.clone(), backend);
        drop(state);
        self.bus.publish(&RuntimeEvent::BackendRegistered(id));
        Ok(())
    }

    /// Enable and initialize a registered backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend is unknown or initialization fails.
    pub fn enable(&self, id: &str) -> Result<(), RuntimeError> {
        let state = self.lock_state()?;
        let backend = state
            .backends
            .get(id)
            .ok_or_else(|| RuntimeError::BackendNotFound(id.to_owned()))?;
        backend.initialize()?;
        drop(state);
        let mut state = self.lock_state()?;
        state.enabled.insert(id.to_owned());
        if state.active.is_none() {
            state.active = Some(id.to_owned());
        }
        drop(state);
        self.bus
            .publish(&RuntimeEvent::BackendEnabled(id.to_owned()));
        Ok(())
    }

    /// Disable and shut down a backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend is unknown or shutdown fails.
    pub fn disable(&self, id: &str) -> Result<(), RuntimeError> {
        let state = self.lock_state()?;
        let backend = state
            .backends
            .get(id)
            .ok_or_else(|| RuntimeError::BackendNotFound(id.to_owned()))?;
        backend.shutdown()?;
        drop(state);
        let mut state = self.lock_state()?;
        state.enabled.remove(id);
        if state.active.as_deref() == Some(id) {
            state.active = state.enabled.iter().next().cloned();
        }
        drop(state);
        self.bus
            .publish(&RuntimeEvent::BackendDisabled(id.to_owned()));
        Ok(())
    }

    /// Discover registered adapters and their enabled state.
    #[must_use]
    pub fn discover(&self) -> Vec<BackendSummary> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        let mut summaries: Vec<_> = state
            .backends
            .values()
            .map(|backend| {
                let descriptor = backend.descriptor();
                let enabled = state.enabled.contains(&descriptor.id);
                let active = state.active.as_deref() == Some(descriptor.id.as_str());
                BackendSummary {
                    descriptor,
                    enabled,
                    active,
                }
            })
            .collect();
        summaries.sort_by(|left, right| left.descriptor.id.cmp(&right.descriptor.id));
        summaries
    }

    /// Return whether a backend is enabled.
    #[must_use]
    pub fn is_enabled(&self, id: &str) -> bool {
        self.state
            .lock()
            .map(|state| state.enabled.contains(id))
            .unwrap_or(false)
    }

    /// Select an enabled backend as active.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend is not registered or enabled.
    pub fn select_active(&self, id: &str) -> Result<(), RuntimeError> {
        let mut state = self.lock_state()?;
        if !state.backends.contains_key(id) {
            return Err(RuntimeError::BackendNotFound(id.to_owned()));
        }
        if !state.enabled.contains(id) {
            return Err(RuntimeError::BackendNotEnabled(id.to_owned()));
        }
        state.active = Some(id.to_owned());
        Ok(())
    }

    /// Return the active backend identifier, if any.
    #[must_use]
    pub fn active_backend(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.active.clone())
    }

    /// Return backend health.
    #[must_use]
    pub fn health(&self, id: &str) -> Option<BackendHealth> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.backends.get(id).map(|backend| backend.health()))
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, BackendManagerState>, RuntimeError> {
        self.state
            .lock()
            .map_err(|_| RuntimeError::BackendManagerUnavailable)
    }
}

/// Deterministic backend used by the mock runtime.
#[derive(Clone, Debug, Default)]
pub struct MockBackend;

impl Backend for MockBackend {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: "mock".to_owned(),
            name: "Mock Backend".to_owned(),
            version: "0.1".to_owned(),
        }
    }

    fn initialize(&self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> BackendHealth {
        BackendHealth::Healthy
    }

    fn list_models(&self) -> Result<Vec<String>, RuntimeError> {
        Ok(Vec::new())
    }

    fn load_model(&self, _model_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn unload_model(&self, _model_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn generate_stream(
        &self,
        request: &GenerationRequest,
    ) -> Result<Vec<StreamChunk>, RuntimeError> {
        Ok(vec![StreamChunk {
            request_id: request.request_id.clone(),
            text: "Mock generation is not AI inference.".to_owned(),
            done: true,
        }])
    }

    fn cancel_generation(&self, _request_id: &str) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn embeddings(&self, _request: &EmbeddingsRequest) -> Result<EmbeddingsResult, RuntimeError> {
        Ok(EmbeddingsResult { values: Vec::new() })
    }

    fn tool_support(&self) -> ToolSupport {
        ToolSupport {
            supported: false,
            reason: "mock backend has no tool implementation".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendManager, MockBackend};
    use oid_shared::EventBus;

    #[test]
    fn manager_registers_enables_and_selects_backend() {
        let manager = BackendManager::new(EventBus::new());
        manager
            .register(Box::new(MockBackend))
            .expect("register backend");
        manager.enable("mock").expect("enable backend");
        let summary = manager.discover();
        assert_eq!(summary.len(), 1);
        assert!(summary[0].enabled);
        assert!(summary[0].active);
        assert_eq!(manager.active_backend().as_deref(), Some("mock"));
    }
}
