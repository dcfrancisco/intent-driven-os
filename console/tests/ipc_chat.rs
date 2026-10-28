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
