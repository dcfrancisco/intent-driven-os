#![allow(missing_docs)]

use std::process::Command;

#[test]
fn console_binary_starts_with_runtime_prompt() {
    let output = Command::new(env!("CARGO_BIN_EXE_oid-console"))
        .output()
        .expect("console binary should run");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("startup output is UTF-8");
    assert!(stdout.contains("AI CONSOLE"));
    assert!(stdout.contains("✓ Runtime initialized"));
    assert!(stdout.contains("[OID Healthy | model none]"));
    assert!(stdout.contains("$ "));
}
