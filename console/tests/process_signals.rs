#![cfg(unix)]
#![allow(missing_docs)]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

struct ConsoleProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl ConsoleProcess {
    fn start_with_state(state: Option<&std::path::Path>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oid-console"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(state) = state {
            command.env("OID_STATE_DIR", state);
        }
        let mut child = command.spawn().expect("console should start");
        let stdin = child.stdin.take().expect("console stdin");
        let stdout = BufReader::new(child.stdout.take().expect("console stdout"));
        let mut process = Self {
            child,
            stdin,
            stdout,
        };
        process.wait_for_text("✓ Console ready");
        process
    }

    fn start() -> Self {
        Self::start_with_state(None)
    }

    fn wait_for_text(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut line = String::new();
        while Instant::now() < deadline {
            line.clear();
            let read = self.stdout.read_line(&mut line).expect("console output");
            assert!(read > 0, "console exited before emitting {needle}");
            if line.contains(needle) {
                return;
            }
        }
        panic!("timed out waiting for {needle}");
    }

    fn send(&mut self, input: &str) {
        self.stdin
            .write_all(input.as_bytes())
            .and_then(|()| self.stdin.flush())
            .expect("console input");
    }

    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .args([name, &self.child.id().to_string()])
            .status()
            .expect("kill command");
        assert!(status.success(), "kill {name} should succeed");
    }

    fn wait(mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().expect("console status") {
                return status;
            }
            assert!(Instant::now() < deadline, "console did not terminate");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[test]
fn sigterm_while_idle_restores_a_clean_process() {
    let process = ConsoleProcess::start();
    process.signal("-TERM");
    assert!(process.wait().success());
}

#[test]
fn sighup_while_idle_restores_a_clean_process() {
    let process = ConsoleProcess::start();
    process.signal("-HUP");
    assert!(process.wait().success());
}

#[test]
fn interrupting_a_native_child_keeps_oid_usable() {
    let mut process = ConsoleProcess::start();
    process.send("printf child-ready; trap '' TERM INT; while :; do :; done\n");
    let mut output = Vec::new();
    while !output.ends_with(b"child-ready") {
        let mut byte = [0_u8; 1];
        assert_eq!(process.stdout.read(&mut byte).expect("child output"), 1);
        output.push(byte[0]);
    }
    process.signal("-INT");
    process.send(":quit\n");
    assert!(process.wait().success());
}

#[test]
fn repeated_interrupts_do_not_corrupt_the_console() {
    let mut process = ConsoleProcess::start();
    process.send("printf child-ready; trap '' TERM INT; while :; do :; done\n");
    let mut output = Vec::new();
    while !output.ends_with(b"child-ready") {
        let mut byte = [0_u8; 1];
        assert_eq!(process.stdout.read(&mut byte).expect("child output"), 1);
        output.push(byte[0]);
    }
    process.signal("-INT");
    process.signal("-INT");
    process.send(":quit\n");
    assert!(process.wait().success());
}

#[test]
fn sigterm_stops_a_child_that_ignores_term() {
    let mut process = ConsoleProcess::start();
    process.send("printf child-ready; trap '' TERM; while :; do :; done\n");
    let mut output = Vec::new();
    while !output.ends_with(b"child-ready") {
        let mut byte = [0_u8; 1];
        assert_eq!(process.stdout.read(&mut byte).expect("child output"), 1);
        output.push(byte[0]);
    }
    process.signal("-TERM");
    assert!(process.wait().success());
}

#[test]
fn sigkill_restart_discovers_pending_governed_operation() {
    let root = std::env::temp_dir().join(format!("oid-sigkill-{}", std::process::id()));
    let target = root.join("target");
    let mut process = ConsoleProcess::start_with_state(Some(&root));
    process.send(&format!(
        ":intent create a directory {}\n",
        target.display()
    ));
    process.wait_for_text("Awaiting approval");
    process.signal("-KILL");
    assert!(!process.wait().success());

    let mut restarted = ConsoleProcess::start_with_state(Some(&root));
    restarted.send(":operations recover\n");
    restarted.wait_for_text("interrupted operation");
    restarted.send(":quit\n");
    assert!(restarted.wait().success());
    std::fs::remove_dir_all(root).expect("cleanup");
}
