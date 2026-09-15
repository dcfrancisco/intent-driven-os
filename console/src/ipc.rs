//! Minimal local IPC protocol for Marina and marinactl.

use std::env;
use std::io;
use std::path::PathBuf;

/// Return the configured Marina Unix socket path.
#[must_use]
pub fn socket_path() -> PathBuf {
    env::var_os("MARINA_SOCKET").map_or_else(
        || {
            env::var_os("HOME").map_or_else(
                || PathBuf::from("/tmp/marina.sock"),
                |home| PathBuf::from(home).join(".marina/marina.sock"),
            )
        },
        PathBuf::from,
    )
}

/// Encode a single protocol field without allowing it to create new lines.
#[must_use]
pub fn encode_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Decode a protocol field.
#[must_use]
pub fn decode_field(value: &str) -> String {
    let mut output = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(match character {
                'n' => '\n',
                'r' => '\r',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    if escaped {
        output.push('\\');
    }
    output
}

/// Ensure the socket parent directory exists.
#[allow(dead_code)]
pub fn ensure_socket_parent() -> io::Result<()> {
    if let Some(parent) = socket_path().parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
