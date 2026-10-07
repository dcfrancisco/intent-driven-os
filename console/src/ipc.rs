//! Minimal local IPC protocol for Marina and marinactl.

#![allow(dead_code)]

use std::env;
use std::io;
use std::path::PathBuf;

#[cfg(windows)]
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(unix)]
/// Marina local listener transport.
pub type MarinaListener = UnixListener;
#[cfg(windows)]
/// Marina local listener transport.
pub type MarinaListener = TcpListener;
#[cfg(unix)]
/// Marina local stream transport.
pub type MarinaStream = UnixStream;
#[cfg(windows)]
/// Marina local stream transport.
pub type MarinaStream = TcpStream;

/// Return the configured Marina Unix socket path.
#[must_use]
pub fn socket_path() -> PathBuf {
    env::var_os("MARINA_SOCKET").map_or_else(
        || {
            env::var_os("MARINA_HOME")
                .or_else(|| env::var_os("HOME"))
                .map_or_else(
                    || PathBuf::from(".marina/marina.sock"),
                    |home| PathBuf::from(home).join(".marina/marina.sock"),
                )
        },
        PathBuf::from,
    )
}

/// Return the configured local endpoint for the current platform.
#[must_use]
pub fn endpoint() -> String {
    #[cfg(unix)]
    {
        return socket_path().display().to_string();
    }
    #[cfg(windows)]
    {
        return env::var("MARINA_TCP_ADDR").unwrap_or_else(|_| "127.0.0.1:11434".to_owned());
    }
}

/// Bind Marina's local transport.
pub fn bind_listener() -> io::Result<MarinaListener> {
    #[cfg(unix)]
    {
        let socket = socket_path();
        if socket.exists() {
            std::fs::remove_file(&socket)?;
        }
        let listener = UnixListener::bind(&socket)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        }
        return Ok(listener);
    }
    #[cfg(windows)]
    {
        return TcpListener::bind(endpoint());
    }
}

/// Connect a local Marina client transport.
pub fn connect() -> io::Result<MarinaStream> {
    #[cfg(unix)]
    {
        return UnixStream::connect(socket_path());
    }
    #[cfg(windows)]
    {
        return TcpStream::connect(endpoint());
    }
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
    #[cfg(windows)]
    {
        return Ok(());
    }
    #[cfg(unix)]
    {
        if let Some(parent) = socket_path().parent() {
            std::fs::create_dir_all(parent)?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
}
