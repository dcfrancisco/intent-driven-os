#![allow(missing_docs)]

use std::process::Command;

mod support;

#[test]
fn console_binary_starts_with_runtime_prompt() {
    let state_root = std::env::temp_dir().join(format!(
        "talvik-console-startup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&state_root).expect("startup test state");
    let daemon = support::MockTalvik::start(&state_root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_oid-console"));
    daemon.configure(&mut command);
    let output = command
        .env("OID_RUNTIME_BACKEND", "talvik-ipc")
        .output()
        .expect("console binary should run");
    drop(daemon);
    std::fs::remove_dir_all(&state_root).expect("cleanup startup test state");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("startup output is UTF-8");
    assert!(stdout.contains("OID Console"));
    assert!(stdout.contains("Type :help to begin."));
    assert!(!stdout.contains("Available models:"));
    assert!(!stdout.contains("Intelligent Runtime"));
    assert!(!stdout.contains('\x1b'));
    assert!(!stdout.contains("No models installed."));
    assert!(stdout.contains(". "));
}

#[test]
fn local_console_fails_clearly_when_talvik_ipc_is_unavailable() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oid-console"));
    command.env("OID_RUNTIME_BACKEND", "talvik-ipc");
    #[cfg(unix)]
    {
        let unavailable_endpoint = std::env::temp_dir().join(format!(
            "talvik-console-unavailable-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        command.env("TALVIK_SOCKET", unavailable_endpoint);
    }
    #[cfg(windows)]
    command.env("TALVIK_TCP_ADDR", "127.0.0.1:0");

    let output = command
        .output()
        .expect("console binary should report startup failure");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("startup error is UTF-8");
    assert!(stderr.contains("Talvik daemon is required for the MarinaOS console"));
}

#[test]
fn local_console_rejects_http_backend_selection() {
    let output = Command::new(env!("CARGO_BIN_EXE_oid-console"))
        .env("OID_RUNTIME_BACKEND", "marina-http")
        .output()
        .expect("console binary should report unsupported remote mode");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("startup error is UTF-8");
    assert!(stderr.contains("oid-console is a local MarinaOS client"));
    assert!(stderr.contains("only supports Talvik IPC"));
}

#[test]
fn daemon_rejects_an_occupied_http_port_before_starting_the_runtime() {
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").expect("occupied port");
    let root = std::env::temp_dir().join(format!("talvik-duplicate-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_talvikd"))
        .env(
            "TALVIK_HTTP_ADDR",
            occupied.local_addr().expect("address").to_string(),
        )
        .env("TALVIK_HOME", &root)
        .env("TALVIK_CONFIG", root.join("config.yaml"))
        .env("TALVIK_SOCKET", root.join("daemon.sock"))
        .output()
        .expect("duplicate daemon should exit");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot bind Talvik HTTP listener"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("loaded meta data"),
        "model loading must not start"
    );
    if root.exists() {
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
