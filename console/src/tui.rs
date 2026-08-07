//! Text user interface lifecycle.

use oid_runtime::{config, logging, MockRuntime, RuntimeService};
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
    runtime: MockRuntime,
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
    /// Create the console, event bus, and mock runtime.
    #[must_use]
    pub fn start() -> Self {
        let configuration = config::load().expect("default runtime configuration is valid");
        let bus = EventBus::new();
        let presence = PresenceIndicator::new(&bus);
        bus.publish(&RuntimeEvent::ConfigurationLoaded);
        let _logger = logging::initialize();
        bus.publish(&RuntimeEvent::LoggingInitialized);
        let runtime = MockRuntime::start(configuration, bus.clone());
        Self {
            runtime,
            bus,
            status_bar: StatusBar {
                mode: StatusBar::mode(
                    &std::env::var("OID_STATUS_MODE").unwrap_or_else(|_| "compact".to_owned()),
                ),
            },
            prompt: Prompt::default(),
            history: History::new(),
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
            }),
        }
    }

    /// Run the interactive console until exit, cancellation, or end-of-file.
    pub fn run(mut self) -> io::Result<()> {
        Renderer::render_startup();
        self.presence.refresh();
        Renderer::render_status(&self.runtime, &self.status_bar);
        loop {
            self.prompt.set_directory(self.shell.directory.current());
            match LineEditor::read_command(
                &self.prompt,
                &mut self.history,
                &self.runtime,
                &mut self.presence,
            )? {
                EditResult::Submitted(line) => {
                    self.history.push(line.clone());
                    let result = match self.router.route(&line) {
                        InputRoute::Oid(command) => commands::execute_with_operations(
                            &command,
                            &HistoryView::new(self.history.entries()),
                            &self.runtime,
                            &mut self.operations,
                        ),
                        InputRoute::Shell(command) => commands::CommandResult {
                            output: self.shell.execute(&command),
                            action: CommandAction::Continue,
                        },
                    };
                    self.presence.refresh();
                    if result.action == CommandAction::Clear {
                        Renderer::clear()?;
                    }
                    Renderer::render_output(&result.output);
                    Renderer::render_status(&self.runtime, &self.status_bar);
                    if result.action == CommandAction::Exit {
                        self.runtime.stop();
                        self.presence.refresh();
                        Renderer::render_shutdown();
                        break;
                    }
                }
                EditResult::Cancelled => {
                    Renderer::render_status(&self.runtime, &self.status_bar);
                }
                EditResult::Eof => {
                    self.runtime.stop();
                    self.bus.publish(&RuntimeEvent::RuntimeStopped);
                    Renderer::render_shutdown();
                    break;
                }
            }
        }
        Ok(())
    }
}
