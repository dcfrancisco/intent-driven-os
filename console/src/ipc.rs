//! Minimal local IPC protocol for Talvik.

#![allow(dead_code)]

use std::env;
use std::io;
use std::path::PathBuf;

/// Talvik local IPC protocol version.
pub const PROTOCOL_VERSION: &str = "3";

#[cfg(windows)]
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(unix)]
/// Talvik local listener transport.
pub type TalvikListener = UnixListener;
#[cfg(windows)]
/// Marina local listener transport.
pub type TalvikListener = TcpListener;
#[cfg(unix)]
/// Talvik local stream transport.
pub type TalvikStream = UnixStream;
#[cfg(windows)]
/// Marina local stream transport.
pub type TalvikStream = TcpStream;

/// Return the configured Talvik Unix socket path.
#[must_use]
pub fn socket_path() -> PathBuf {
    env::var_os("TALVIK_SOCKET")
        .or_else(|| env::var_os("MARINA_SOCKET"))
        .map_or_else(
            || {
                env::var_os("TALVIK_HOME")
                    .or_else(|| env::var_os("MARINA_HOME"))
                    .map_or_else(
                        || {
                            env::var_os("HOME").map_or_else(
                                || PathBuf::from(".marina/marina.sock"),
                                |home| PathBuf::from(home).join(".marina/marina.sock"),
                            )
                        },
                        |home| PathBuf::from(home).join("marina.sock"),
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
        socket_path().display().to_string()
    }
    #[cfg(windows)]
    {
        env::var("TALVIK_TCP_ADDR")
            .or_else(|_| env::var("MARINA_TCP_ADDR"))
            .unwrap_or_else(|_| "127.0.0.1:11434".to_owned())
    }
}

/// Bind Talvik's local transport.
pub fn bind_listener() -> io::Result<TalvikListener> {
    #[cfg(unix)]
    {
        bind_unix_listener(&socket_path())
    }
    #[cfg(windows)]
    {
        TcpListener::bind(endpoint())
    }
}

#[cfg(unix)]
fn bind_unix_listener(socket: &std::path::Path) -> io::Result<UnixListener> {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    if socket.exists() {
        if !std::fs::symlink_metadata(socket)?.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Talvik socket path is not a socket",
            ));
        }
        match UnixStream::connect(socket) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    format!("Talvik is already running at {}", socket.display()),
                ))
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                match std::fs::remove_file(socket) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Connect a local Talvik client transport.
pub fn connect() -> io::Result<TalvikStream> {
    #[cfg(unix)]
    {
        UnixStream::connect(socket_path())
    }
    #[cfg(windows)]
    {
        TcpStream::connect(endpoint())
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
#[allow(dead_code, clippy::items_after_statements)]
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

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn running_socket_is_preserved_and_stale_socket_can_be_restarted() {
        let socket = std::env::temp_dir().join(format!("tv-singleton-{}.sock", std::process::id()));
        let first = super::bind_unix_listener(&socket).expect("first daemon");
        let error = super::bind_unix_listener(&socket).expect_err("duplicate daemon must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        let client = super::UnixStream::connect(&socket).expect("original listener remains live");
        // Drain both health probes before closing. macOS may keep a listener
        // reachable while accepted-but-unconsumed connections remain queued.
        drop(first.accept().expect("duplicate probe"));
        drop(first.accept().expect("connectivity probe"));
        drop(client);
        drop(first);
        // Other tests fork shell children concurrently. Until their exec closes
        // inherited descriptors, macOS can still see this listener as live.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let restarted = loop {
            match super::bind_unix_listener(&socket) {
                Ok(listener) => break listener,
                Err(error)
                    if error.kind() == std::io::ErrorKind::AddrInUse
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("stale socket is recoverable: {error}"),
            }
        };
        drop(restarted);
        std::fs::remove_file(socket).expect("cleanup socket");
    }
}
