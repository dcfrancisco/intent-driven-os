//! Intent command boundary.

use crate::operations::ConsoleOperations;
use crate::signals::SignalController;
use std::io::Write;

use oid_runtime::{
    BackendHealth, GenerationMessage, GenerationOptions, GenerationRequest, ModelMetadata,
    RuntimeService,
};

/// A user-entered intent before parsing or execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentCommand {
    /// Raw text entered at the prompt.
    pub text: String,
}

/// Parser interface reserved for the future command grammar.
pub trait CommandParser {
    /// Parse raw prompt text into an intent command.
    fn parse(&self, input: &str) -> IntentCommand;
}

/// Minimal parser that preserves input without executing it.
#[derive(Clone, Debug, Default)]
pub struct FoundationParser;

impl CommandParser for FoundationParser {
    fn parse(&self, input: &str) -> IntentCommand {
        IntentCommand {
            text: input.to_owned(),
        }
    }
}

/// Action requested by a built-in command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandAction {
    /// Keep the console running.
    Continue,
    /// Clear the terminal before continuing.
    Clear,
    /// Stop the console.
    Exit,
}

/// Result of processing a console command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResult {
    /// Lines to display to the user.
    pub output: Vec<String>,
    /// Lifecycle action requested by the command.
    pub action: CommandAction,
}

/// Read-only view of command history used by command processing.
pub struct HistoryView<'a> {
    entries: &'a [String],
}

impl<'a> HistoryView<'a> {
    /// Create a view over history entries.
    #[must_use]
    pub const fn new(entries: &'a [String]) -> Self {
        Self { entries }
    }

    /// Return history entries.
    #[must_use]
    pub const fn entries(&self) -> &'a [String] {
        self.entries
    }
}

/// Process the Phase 2 built-in command set.
#[must_use]
#[allow(clippy::too_many_lines)]
#[allow(dead_code)]
pub fn execute(
    input: &str,
    history: &HistoryView<'_>,
    runtime: &dyn RuntimeService,
) -> CommandResult {
    let mut operations = ConsoleOperations::new();
    execute_with_operations(input, history, runtime, &mut operations)
}

/// Process commands using the application's persistent operation coordinator.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn execute_with_operations(
    input: &str,
    history: &HistoryView<'_>,
    runtime: &dyn RuntimeService,
    operations: &mut ConsoleOperations,
) -> CommandResult {
    execute_with_operations_and_signals(input, history, runtime, operations, None)
}

/// Process commands while allowing active generation to observe Ctrl+C.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn execute_with_operations_and_signals(
    input: &str,
    history: &HistoryView<'_>,
    runtime: &dyn RuntimeService,
    operations: &mut ConsoleOperations,
    signals: Option<&SignalController>,
) -> CommandResult {
    let parsed = FoundationParser.parse(input.trim());
    let command = parsed.text.trim();
    let (output, action) = match command {
        "help" => (
            vec![
                "Commands:".to_owned(),
                "OID commands:".to_owned(),
                "  :help     Show this help".to_owned(),
                "  :status   Show runtime status".to_owned(),
                "  :runtime  Show runtime service details".to_owned(),
                "  :models   List registered model metadata".to_owned(),
                "  :intent create a directory <path>  Plan a governed operation".to_owned(),
                "  :operations recover|inspect|approve|resume|rollback  Manage operations"
                    .to_owned(),
                "  :quit     Shut down the console".to_owned(),
            ],
            CommandAction::Continue,
        ),
        "status" | "runtime" => (runtime_lines(runtime), CommandAction::Continue),
        "health" => {
            let snapshot = runtime.snapshot();
            (
                vec![
                    format!("Runtime health: {}", snapshot.health),
                    format!("Active backend: {}", snapshot.backend),
                ],
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("intent create a directory ") => {
            let raw = command
                .trim_start_matches("intent create a directory ")
                .trim();
            let approved = raw.ends_with(" --approve");
            let path = if approved {
                raw.trim_end_matches(" --approve").trim()
            } else {
                raw
            };
            (
                operations
                    .intent_create_directory(path, approved)
                    .unwrap_or_else(|error| vec![format!("Intent failed: {error}")]),
                CommandAction::Continue,
            )
        }
        "inspect system" => (
            crate::foundation::run_system_health(&runtime.event_bus())
                .unwrap_or_else(|error| vec![format!("Foundation flow failed: {error}")]),
            CommandAction::Continue,
        ),
        _ if command.starts_with("create directory ") => {
            let arguments = command.trim_start_matches("create directory ").trim();
            let approved = arguments.ends_with(" --approve");
            let path = if approved {
                arguments.trim_end_matches(" --approve").trim()
            } else {
                arguments
            };
            (
                operations
                    .create_directory(path, approved)
                    .unwrap_or_else(|error| vec![format!("Directory flow failed: {error}")]),
                CommandAction::Continue,
            )
        }
        "operations recover" => (
            operations
                .recover()
                .unwrap_or_else(|error| vec![format!("Recovery failed: {error}")]),
            CommandAction::Continue,
        ),
        _ if command.starts_with("operations inspect ") => {
            let id = command.trim_start_matches("operations inspect ").trim();
            (
                operations
                    .inspect(id)
                    .unwrap_or_else(|error| vec![format!("Inspection failed: {error}")]),
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("operations approve ") => {
            let id = command.trim_start_matches("operations approve ").trim();
            (
                operations
                    .approve(id)
                    .unwrap_or_else(|error| vec![format!("Approval failed: {error}")]),
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("operations resume ") => {
            let id = command.trim_start_matches("operations resume ").trim();
            (
                operations
                    .resume(id)
                    .unwrap_or_else(|error| vec![format!("Resume failed: {error}")]),
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("operations rollback ") => {
            let id = command.trim_start_matches("operations rollback ").trim();
            (
                operations
                    .rollback(id)
                    .unwrap_or_else(|error| vec![format!("Rollback failed: {error}")]),
                CommandAction::Continue,
            )
        }
        "backend" | "backend list" => (backend_lines(runtime), CommandAction::Continue),
        "backend info" => (backend_info_lines(runtime), CommandAction::Continue),
        "backend status" => {
            let snapshot = runtime.snapshot();
            let health = runtime
                .backend_health(&snapshot.backend)
                .map_or("Unavailable", |health| backend_health_label(&health));
            (
                vec![
                    format!("Active backend: {}", snapshot.backend),
                    format!("Health: {health}"),
                ],
                CommandAction::Continue,
            )
        }
        "models" => (model_lines(runtime), CommandAction::Continue),
        _ if command.starts_with("model inspect ") => {
            let id = command.trim_start_matches("model inspect ").trim();
            let output = runtime.model_inspect(id).map_or_else(
                || vec![format!("Model not found: {id}")],
                |model| model_lines_for(&model),
            );
            (output, CommandAction::Continue)
        }
        _ if command.starts_with("model load ") => {
            let id = command.trim_start_matches("model load ").trim();
            (
                operation_result_unit(runtime.model_load(id), &format!("Model loaded: {id}")),
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("model unload ") => {
            let id = command.trim_start_matches("model unload ").trim();
            (
                operation_result_unit(runtime.model_unload(id), &format!("Model unloaded: {id}")),
                CommandAction::Continue,
            )
        }
        "model status" => (model_lines(runtime), CommandAction::Continue),
        _ if command.starts_with("tokenize ") => {
            let text = command.trim_start_matches("tokenize ");
            (token_count_lines(runtime, text), CommandAction::Continue)
        }
        _ if command.starts_with("count ") => {
            let path = command.trim_start_matches("count ").trim();
            let output = std::fs::read_to_string(path)
                .map_err(|error| oid_runtime::RuntimeError::Persistence(error.to_string()))
                .and_then(|text| runtime.tokenize(&text));
            (
                operation_result(output, &format!("Tokens in {path}")),
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("generate ") || command.starts_with("explain ") => {
            let raw_prompt = command
                .split_once(' ')
                .map_or("", |(_, value)| value)
                .trim_matches('"');
            let (max_tokens, prompt) = raw_prompt
                .strip_prefix("--max-tokens ")
                .and_then(|value| value.split_once(' '))
                .and_then(|(count, prompt)| count.parse().ok().map(|count| (Some(count), prompt)))
                .unwrap_or((None, raw_prompt));
            (
                generate_lines_with_interrupt(runtime, prompt, signals, max_tokens),
                CommandAction::Continue,
            )
        }
        _ if command.starts_with("complete ") => {
            let path = command.trim_start_matches("complete ").trim();
            let output = std::fs::read_to_string(path)
                .map_err(|error| oid_runtime::RuntimeError::Persistence(error.to_string()))
                .and_then(|text| generate_lines_result(runtime, &text));
            (
                output.unwrap_or_else(|error| vec![format!("Error: {error}")]),
                CommandAction::Continue,
            )
        }
        "hardware" => (hardware_lines(runtime), CommandAction::Continue),
        "history" => (
            history.entries().iter().map(ToOwned::to_owned).collect(),
            CommandAction::Continue,
        ),
        "clear" => (Vec::new(), CommandAction::Clear),
        "version" => (vec!["AI Console v0.1".to_owned()], CommandAction::Continue),
        "about" => (
            vec!["AI Console is the reference client for the Intelligent Runtime.".to_owned()],
            CommandAction::Continue,
        ),
        "exit" | "quit" => (vec!["Goodbye.".to_owned()], CommandAction::Exit),
        "" => (Vec::new(), CommandAction::Continue),
        _ => (
            vec![format!("Unknown OID command: {command}. Type :help.")],
            CommandAction::Continue,
        ),
    };
    CommandResult { output, action }
}

fn runtime_lines(runtime: &dyn RuntimeService) -> Vec<String> {
    let snapshot = runtime.snapshot();
    vec![
        format!("Runtime : {}", snapshot.health),
        format!("Backend : {}", snapshot.backend),
        format!(
            "Model   : {}",
            snapshot.loaded_model.as_deref().unwrap_or("None")
        ),
        format!("Models  : {}", snapshot.models),
        format!("Memory  : {}", snapshot.memory),
        format!("Uptime  : {}s", snapshot.uptime_seconds),
    ]
}

fn backend_lines(runtime: &dyn RuntimeService) -> Vec<String> {
    let backends = runtime.backend_list();
    if backends.is_empty() {
        return vec!["No backends registered.".to_owned()];
    }
    backends
        .into_iter()
        .map(|backend| {
            format!(
                "{} ({}) [{}{}]",
                backend.descriptor.id,
                backend.descriptor.name,
                if backend.enabled {
                    "enabled"
                } else {
                    "disabled"
                },
                if backend.active { ", active" } else { "" }
            )
        })
        .collect()
}

fn backend_info_lines(runtime: &dyn RuntimeService) -> Vec<String> {
    let snapshot = runtime.snapshot();
    vec![
        format!("Backend : {}", snapshot.backend),
        format!(
            "Version : {}",
            snapshot.backend_version.as_deref().unwrap_or("Unavailable")
        ),
        format!("Health  : {}", snapshot.health),
    ]
}

fn token_count_lines(runtime: &dyn RuntimeService, text: &str) -> Vec<String> {
    operation_result(runtime.tokenize(text), "Tokens")
}

fn generate_lines_with_interrupt(
    runtime: &dyn RuntimeService,
    prompt: &str,
    signals: Option<&SignalController>,
    max_tokens: Option<u32>,
) -> Vec<String> {
    generate_lines_result_with_interrupt(runtime, prompt, signals, max_tokens)
        .unwrap_or_else(|error| vec![format!("Error: {error}")])
}

fn generate_lines_result(
    runtime: &dyn RuntimeService,
    prompt: &str,
) -> Result<Vec<String>, oid_runtime::RuntimeError> {
    generate_lines_result_with_interrupt(runtime, prompt, None, None)
}

fn generate_lines_result_with_interrupt(
    runtime: &dyn RuntimeService,
    prompt: &str,
    signals: Option<&SignalController>,
    max_tokens: Option<u32>,
) -> Result<Vec<String>, oid_runtime::RuntimeError> {
    let mut options = GenerationOptions::default();
    if let Ok(value) = std::env::var("OID_MAX_TOKENS") {
        if let Ok(max_tokens) = value.parse() {
            options.max_tokens = max_tokens;
        }
    }
    if let Ok(value) = std::env::var("OID_CONTEXT_SIZE") {
        if let Ok(context_size) = value.parse() {
            options.context_size = context_size;
        }
    }
    if let Some(max_tokens) = max_tokens {
        options.max_tokens = max_tokens;
    }
    let request = GenerationRequest {
        request_id: format!("console-{}", std::process::id()),
        model_id: runtime.snapshot().loaded_model.unwrap_or_default(),
        prompt: prompt.to_owned(),
        options,
    };
    let stream = runtime.generate(request)?;
    let mut lines = vec!["Generating...".to_owned()];
    loop {
        if signals.is_some_and(SignalController::take_interrupt) {
            runtime.cancel_generation();
        }
        match stream.recv_timeout(std::time::Duration::from_millis(25)) {
            Ok(GenerationMessage::Token(token)) => {
                if signals.is_some() {
                    print!("{token}");
                    let _ = std::io::stdout().flush();
                } else {
                    lines.push(token);
                }
            }
            Ok(GenerationMessage::Completed(result)) => {
                lines.push(format!(
                    "Generated {} tokens at {:.1} tokens/sec",
                    result.statistics.generated_tokens, result.statistics.tokens_per_second
                ));
                lines.push(format!(
                    "Prompt tokens: {} | Context: {} | Latency: {} ms",
                    result.statistics.prompt_tokens,
                    result.statistics.context_tokens,
                    result.statistics.latency_ms
                ));
                break;
            }
            Ok(GenerationMessage::Cancelled(result)) => {
                lines.push(format!(
                    "Generation cancelled after {} tokens",
                    result.statistics.generated_tokens
                ));
                break;
            }
            Ok(GenerationMessage::Failed(error)) => return Err(error),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(oid_runtime::RuntimeError::ModelLifecycle(
                    "generation stream closed".to_owned(),
                ));
            }
        }
    }
    Ok(lines)
}

fn operation_result<T: std::fmt::Display>(
    result: Result<T, oid_runtime::RuntimeError>,
    success: &str,
) -> Vec<String> {
    result.map_or_else(
        |error| vec![format!("Error: {error}")],
        |value| vec![format!("{success}: {value}")],
    )
}

fn operation_result_unit(
    result: Result<(), oid_runtime::RuntimeError>,
    success: &str,
) -> Vec<String> {
    result.map_or_else(
        |error| vec![format!("Error: {error}")],
        |()| vec![success.to_owned()],
    )
}

fn model_lines(runtime: &dyn RuntimeService) -> Vec<String> {
    let models = runtime.model_list();
    if models.is_empty() {
        return vec!["No models registered.".to_owned()];
    }
    models
        .into_iter()
        .map(|model| model_lines_for(&model)[0].clone())
        .collect()
}

fn model_lines_for(model: &ModelMetadata) -> Vec<String> {
    vec![
        format!("ID              : {}", model.id),
        format!("Name            : {}", model.name),
        format!("Family          : {}", model.family),
        format!("Backend         : {}", model.backend),
        format!("Quantization    : {}", model.quantization),
        format!("Context window  : {}", model.context_window),
        format!("Memory (MB)     : {}", model.memory_requirement_mb),
        format!("Capabilities    : {}", model.capabilities.join(", ")),
        format!("Status          : {:?}", model.status),
        format!(
            "Checksum        : {}",
            model.checksum.as_deref().unwrap_or("None")
        ),
        format!("Location        : {}", model.location),
    ]
}

fn hardware_lines(runtime: &dyn RuntimeService) -> Vec<String> {
    let hardware = runtime.hardware();
    vec![
        format!("Operating system : {}", hardware.operating_system),
        format!("Architecture     : {}", hardware.architecture),
        format!("CPU              : {}", hardware.cpu),
        format!("Logical cores    : {}", hardware.logical_cores),
        format!(
            "Physical cores   : {}",
            hardware
                .physical_cores
                .map_or_else(|| "--".to_owned(), |cores| cores.to_string())
        ),
        format!(
            "Installed RAM    : {}",
            format_bytes(hardware.installed_ram_bytes)
        ),
        format!(
            "Available RAM    : {}",
            format_bytes(hardware.available_ram_bytes)
        ),
        format!(
            "SIMD             : {}",
            if hardware.simd_capabilities.is_empty() {
                "None".to_owned()
            } else {
                hardware.simd_capabilities.join(", ")
            }
        ),
        format!(
            "GPU              : {}",
            hardware.gpu.as_deref().unwrap_or("Not implemented")
        ),
        format!(
            "NPU              : {}",
            hardware.npu.as_deref().unwrap_or("Not implemented")
        ),
    ]
}

fn format_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(
        || "--".to_owned(),
        |bytes| format!("{} MB", bytes / 1_048_576),
    )
}

fn backend_health_label(health: &BackendHealth) -> &'static str {
    match health {
        BackendHealth::Healthy => "Healthy",
        BackendHealth::Unavailable => "Unavailable",
        BackendHealth::Error(_) => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::{execute, CommandAction, HistoryView};
    use crate::operations::ConsoleOperations;
    use oid_runtime::MockRuntime;
    use oid_shared::{EventBus, RuntimeConfig};

    #[test]
    fn built_in_commands_return_mock_information() {
        let runtime = MockRuntime::start(RuntimeConfig::default(), EventBus::new());
        let result = execute("status", &HistoryView::new(&[]), &runtime);
        assert_eq!(result.action, CommandAction::Continue);
        assert!(result.output.iter().any(|line| line.contains("Healthy")));
    }

    #[test]
    fn exit_is_explicit() {
        let runtime = MockRuntime::start(RuntimeConfig::default(), EventBus::new());
        let result = execute("exit", &HistoryView::new(&[]), &runtime);
        assert_eq!(result.action, CommandAction::Exit);
    }

    #[test]
    fn operation_commands_use_the_session_coordinator() {
        let runtime = MockRuntime::start(RuntimeConfig::default(), EventBus::new());
        let root = std::env::temp_dir().join(format!("oid-console-command-{}", std::process::id()));
        let mut operations = ConsoleOperations::with_root(root.clone());
        let result = super::execute_with_operations(
            "operations recover",
            &HistoryView::new(&[]),
            &runtime,
            &mut operations,
        );
        assert_eq!(result.output, vec!["No recoverable operations."]);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
