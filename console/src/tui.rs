//! Text user interface lifecycle.

use oid_runtime::{logging, RuntimeService};
use oid_shared::{EventBus, RuntimeEvent};
use std::io;

use crate::{
    commands::{self, CommandAction, HistoryView},
    editor::{EditResult, LineEditor},
    history::History,
    input_router::{InputRoute, InputRouter},
    operations::ConsoleOperations,
    presence::PresenceIndicator,
    prompt::Prompt,
    renderer::Renderer,
    shell::ShellCommandController,
    status_bar::StatusBar,
};

/// Console application state.
pub struct Application {
    runtime: Box<dyn RuntimeService>,
    bus: EventBus,
    status_bar: StatusBar,
    prompt: Prompt,
    history: History,
    presence: PresenceIndicator,
    operations: ConsoleOperations,
    router: InputRouter,
    shell: ShellCommandController,
}

impl Application {
    /// Create the `MarinaOS` console backed by the local `Talvik` daemon.
    pub fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let bus = EventBus::new();
        let presence = PresenceIndicator::new(&bus);
        let _logger = logging::initialize();
        bus.publish(&RuntimeEvent::LoggingInitialized);
        if let Ok(backend) = std::env::var("OID_RUNTIME_BACKEND") {
            if backend != "talvik-ipc" {
                return Err(std::io::Error::other(
                    "oid-console is a local MarinaOS client and only supports Talvik IPC; use Talvik's HTTP API from external clients",
                )
                .into());
            }
        }
        crate::ipc_runtime::IpcRuntimeService::probe().map_err(|error| {
            std::io::Error::other(format!(
                "Talvik daemon is required for the MarinaOS console: {error}"
            ))
        })?;
        let service = crate::ipc_runtime::IpcRuntimeService::new(bus.clone());
        bus.publish(&RuntimeEvent::RuntimeStarted);
        let runtime: Box<dyn RuntimeService> = Box::new(service);
        Ok(Self {
            runtime,
            bus,
            status_bar: StatusBar {
                mode: StatusBar::mode(
                    &std::env::var("OID_STATUS_MODE").unwrap_or_else(|_| "compact".to_owned()),
                ),
            },
            prompt: Prompt::default(),
            history: History::load_default(),
            presence,
            operations: ConsoleOperations::new(),
            router: InputRouter,
            shell: ShellCommandController::new().unwrap_or_else(|_| ShellCommandController {
                executor: crate::shell::ShellExecutor::from_path(std::path::PathBuf::from(
                    "/bin/sh",
                )),
                directory: crate::shell::WorkingDirectoryManager::from_path(
                    std::env::current_dir().expect("current directory is available"),
                ),
                signals: crate::signals::SignalController::test(),
            }),
        })
    }

    /// Run the interactive console until exit, cancellation, or end-of-file.
    pub fn run(mut self) -> io::Result<()> {
        Renderer::render_startup();
        self.presence.refresh();
        let mut footer = self.status_bar.attach(self.runtime.as_ref())?;
        loop {
            match LineEditor::read_command(
                &self.prompt,
                &mut self.history,
                self.runtime.as_ref(),
                &mut self.presence,
                &self.shell.signals,
                &mut footer,
            )? {
                EditResult::Submitted(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    self.history.push(line.clone());
                    let result = match self.router.route(&line) {
                        InputRoute::Oid(command) => {
                            footer.update(
                                self.runtime.as_ref(),
                                crate::presence::PresenceState::Busy,
                            )?;
                            commands::execute_with_operations_and_signals(
                                &command,
                                &HistoryView::new(self.history.entries()),
                                self.runtime.as_ref(),
                                &mut self.operations,
                                Some(&self.shell.signals),
                            )
                        }
                        InputRoute::Shell(command) => {
                            footer.suspend()?;
                            let output = self.shell.execute(&command);
                            footer.resume()?;
                            commands::CommandResult {
                                output,
                                action: CommandAction::Continue,
                            }
                        }
                    };
                    let shutdown_requested = self.shell.signals.is_shutdown_requested();
                    self.presence.refresh();
                    if result.action == CommandAction::Clear {
                        Renderer::clear()?;
                    }
                    Renderer::render_output(&result.output);
                    footer.update(self.runtime.as_ref(), self.presence.state())?;
                    if result.action == CommandAction::Exit || shutdown_requested {
                        self.runtime.stop();
                        self.presence.refresh();
                        drop(footer);
                        Renderer::render_shutdown();
                        break;
                    }
                }
                EditResult::Cancelled => {
                    footer.update(self.runtime.as_ref(), self.presence.state())?;
                }
                EditResult::Eof => {
                    self.runtime.stop();
                    self.bus.publish(&RuntimeEvent::RuntimeStopped);
                    drop(footer);
                    Renderer::render_shutdown();
                    break;
                }
            }
        }
        Ok(())
    }
}
