//! Configured shell execution and persistent working-directory management.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::signals::SignalController;

/// Tracks the console's current and previous working directories.
#[derive(Clone, Debug)]
pub struct WorkingDirectoryManager {
    current: PathBuf,
    previous: Option<PathBuf>,
}

impl WorkingDirectoryManager {
    /// Initialize from the process working directory.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            current: std::env::current_dir()?,
            previous: None,
        })
    }

    /// Construct a manager at an explicit directory, primarily for tests.
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        Self {
            current: path,
            previous: None,
        }
    }

    /// Return the current directory.
    #[must_use]
    pub fn current(&self) -> &Path {
        &self.current
    }

    /// Change directory, supporting `cd`, `cd <path>`, and `cd -`.
    pub fn change_directory(&mut self, argument: Option<&str>) -> io::Result<()> {
        let target = match argument.map(str::trim).filter(|value| !value.is_empty()) {
            None => std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
            Some("-") => self
                .previous
                .clone()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no previous directory"))?,
            Some("~") => std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
            Some(value) if value.starts_with("~/") => std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(value.trim_start_matches("~/")))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
            Some(value) => {
                let path = PathBuf::from(value);
                if path.is_absolute() {
                    path
                } else {
                    self.current.join(path)
                }
            }
        };
        let target = std::fs::canonicalize(target)?;
        if !target.is_dir() {
            return Err(io::Error::other("not a directory"));
        }
        self.previous = Some(self.current.clone());
        self.current = target;
        Ok(())
    }
}

/// Result of executing a shell command.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub struct ShellResult {
    /// Process exit status.
    pub status: ExitStatus,
    /// Captured standard output when capture mode is used.
    pub stdout: String,
    /// Captured standard error when capture mode is used.
    pub stderr: String,
}

/// Executes commands through the user's configured shell.
#[derive(Clone, Debug)]
pub struct ShellExecutor {
    shell: PathBuf,
}

impl ShellExecutor {
    /// Resolve `$SHELL`, then `/bin/bash`, then `/bin/sh`.
    pub fn new() -> io::Result<Self> {
        let shell = std::env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| {
                ["/bin/bash", "/bin/sh"]
                    .iter()
                    .map(PathBuf::from)
                    .find(|path| path.is_file())
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no usable shell found"))?;
        Ok(Self { shell })
    }

    /// Construct an executor for a known shell, primarily for tests.
    #[must_use]
    pub fn from_path(shell: PathBuf) -> Self {
        Self { shell }
    }

    /// Return the resolved shell path.
    #[must_use]
    #[allow(dead_code)]
    pub fn shell(&self) -> &Path {
        &self.shell
    }

    /// Execute with inherited standard input/output/error.
    #[allow(dead_code)]
    pub fn execute(&self, command: &str, directory: &Path) -> io::Result<ExitStatus> {
        self.execute_with_signals(command, directory, &SignalController::test())
    }

    /// Execute while supervising signals and cleaning up the child process.
    pub fn execute_with_signals(
        &self,
        command: &str,
        directory: &Path,
        signals: &SignalController,
    ) -> io::Result<ExitStatus> {
        let grouped = process_groups_available();
        let mut child = self.spawn(command, directory, grouped)?;
        loop {
            if signals.take_interrupt() || signals.take_shutdown() {
                terminate_child(&mut child, grouped)?;
            }
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Execute with captured output for tests and non-interactive clients.
    #[allow(dead_code)]
    pub fn execute_capture(&self, command: &str, directory: &Path) -> io::Result<ShellResult> {
        let output = Command::new(&self.shell)
            .arg("-c")
            .arg(command)
            .current_dir(directory)
            .output()?;
        Ok(ShellResult {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn spawn(
        &self,
        command: &str,
        directory: &Path,
        grouped: bool,
    ) -> io::Result<std::process::Child> {
        #[cfg(unix)]
        {
            // A new session gives the supervisor a process-group boundary.
            // The trap restores default dispositions inherited from OID before
            // the configured shell starts, so SIGINT remains child-directed.
            let wrapper = ["/usr/bin/setsid", "/bin/setsid"]
                .iter()
                .find(|path| std::path::Path::new(path).is_file());
            let mut process = if let (true, Some(wrapper)) = (grouped, wrapper) {
                let mut process = Command::new(wrapper);
                process.args(["/bin/sh", "-c"]);
                process
            } else {
                let mut process = Command::new("/bin/sh");
                process.args(["-c"]);
                process
            };
            process
                .args([
                    "trap - INT TERM HUP; exec \"$1\" -c \"$2\"",
                    "oid-child-wrapper",
                    self.shell.to_string_lossy().as_ref(),
                    command,
                ])
                .current_dir(directory)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
        }
        #[cfg(not(unix))]
        {
            Command::new(&self.shell)
                .arg("-c")
                .arg(command)
                .current_dir(directory)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
        }
    }
}

/// Execute `cd` internally or delegate ordinary commands to the shell.
#[derive(Clone, Debug)]
pub struct ShellCommandController {
    /// Shell executor.
    pub executor: ShellExecutor,
    /// Persistent directory manager.
    pub directory: WorkingDirectoryManager,
    /// Process-signal controller shared with the application.
    pub signals: SignalController,
}

impl ShellCommandController {
    /// Create a controller using the resolved shell and current directory.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            executor: ShellExecutor::new()?,
            directory: WorkingDirectoryManager::new()?,
            signals: SignalController::new()?,
        })
    }

    /// Execute one input line and return printable output lines.
    pub fn execute(&mut self, command: &str) -> Vec<String> {
        let trimmed = command.trim();
        if trimmed == "cd" || trimmed.starts_with("cd ") {
            let argument = trimmed
                .strip_prefix("cd")
                .map(str::trim)
                .filter(|value| !value.is_empty());
            return match self.directory.change_directory(argument) {
                Ok(()) => vec![format!(
                    "Changed directory to {}",
                    self.directory.current().display()
                )],
                Err(error) => vec![format!("cd: {error}")],
            };
        }
        match self
            .executor
            .execute_with_signals(trimmed, self.directory.current(), &self.signals)
        {
            Ok(status) if status.success() => Vec::new(),
            Ok(status) => vec![format!(
                "[exit status: {}]",
                status
                    .code()
                    .map_or_else(|| "signal termination".to_owned(), |code| code.to_string())
            )],
            Err(error) => vec![format!("shell: {error}")],
        }
    }
}

fn terminate_child(child: &mut std::process::Child, grouped: bool) -> io::Result<()> {
    #[cfg(unix)]
    if grouped {
        let pid = child.id();
        {
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{pid}")])
                .status();
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
    while std::time::Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let _ = child.kill();
    let _ = child.wait()?;
    Ok(())
}

fn process_groups_available() -> bool {
    cfg!(unix)
        && ["/usr/bin/setsid", "/bin/setsid"]
            .iter()
            .any(|path| std::path::Path::new(path).is_file())
}

#[cfg(test)]
mod tests {
    use super::{ShellCommandController, ShellExecutor, WorkingDirectoryManager};
    use crate::signals::ProcessSignal;
    use std::path::PathBuf;

    #[test]
    fn shell_executes_pipelines_and_redirection() {
        let directory = std::env::temp_dir();
        let executor = ShellExecutor::from_path(PathBuf::from("/bin/sh"));
        let result = executor
            .execute_capture("printf 'a\\nb\\n' | wc -l", &directory)
            .expect("shell execution");
        assert_eq!(result.status.code(), Some(0));
        assert!(result.stdout.trim().ends_with('2'));
    }

    #[test]
    fn cd_updates_persistent_directory_and_supports_cd_dash() {
        let root = std::env::temp_dir();
        let child = root.join(format!("oid-shell-test-{}", std::process::id()));
        std::fs::create_dir_all(&child).expect("fixture");
        let mut manager = WorkingDirectoryManager::from_path(root.clone());
        manager
            .change_directory(Some(child.to_str().expect("utf8 path")))
            .expect("cd");
        manager.change_directory(Some("-")).expect("cd -");
        assert_eq!(
            manager.current(),
            std::fs::canonicalize(root).expect("canonical root")
        );
        std::fs::remove_dir_all(child).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_child_returns_signal_status_and_leaves_parent_usable() {
        let directory = std::env::temp_dir();
        let executor = ShellExecutor::from_path(PathBuf::from("/bin/sh"));
        let signals = crate::signals::SignalController::test();
        let injected = signals.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            injected.inject(ProcessSignal::Interrupt);
        });
        let status = executor
            .execute_with_signals(
                "trap '' TERM INT; while :; do :; done",
                &directory,
                &signals,
            )
            .expect("child status");
        thread.join().expect("signal injector");
        assert!(status.code().is_none());
        let mut controller = ShellCommandController {
            executor,
            directory: WorkingDirectoryManager::from_path(directory),
            signals,
        };
        assert!(controller.execute("true").is_empty());
    }
}
