#![allow(missing_docs)]

use std::process::Command;

mod support;

#[test]
fn standalone_chat_uses_the_talvik_ipc_daemon() {
    let state_root = std::env::temp_dir().join(format!(
        "talvik-ipc-chat-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&state_root).expect("chat test state");
    let daemon = support::MockTalvik::start(&state_root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_oid-marina-chat"));
    daemon.configure(&mut command);
    let output = command
        .env("OID_MARINA_MODEL", "qwen")
        .arg("hello")
        .output()
        .expect("chat client should run");
    drop(daemon);
    std::fs::remove_dir_all(&state_root).expect("cleanup chat test state");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("chat response is UTF-8");
    assert!(stdout.contains("mock response"));
}

#[test]
fn import_preserves_source_id_and_checksum_in_the_cli_request() {
    let state_root = std::env::temp_dir().join(format!("talvik-import-{}", std::process::id()));
    std::fs::create_dir_all(&state_root).expect("import test state");
    let daemon = support::MockTalvik::start(&state_root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_talvik"));
    daemon.configure(&mut command);
    let checksum = "a".repeat(64);
    let output = command
        .args([
            "import",
            "https://example.invalid/source.gguf",
            "candidate",
            &checksum,
        ])
        .output()
        .expect("import client should run");
    drop(daemon);
    std::fs::remove_dir_all(&state_root).expect("cleanup import test state");
    // The mock rejects downloads and reports the received wire request. This
    // exercises argv dispatch as well as the client-to-daemon transport.
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(
        stderr.contains("mock daemon does not pull models"),
        "{stderr}"
    );
    assert!(
        stderr.contains("https://example.invalid/source.gguf"),
        "{stderr}"
    );
    assert!(stderr.contains("candidate"), "{stderr}");
    assert!(stderr.contains(&checksum), "{stderr}");
}
