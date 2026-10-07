//! Local bearer-token management for Marina clients.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Return the local token database path.
#[must_use]
pub fn token_file() -> PathBuf {
    std::env::var_os("MARINA_TOKEN_FILE").map_or_else(
        || {
            std::env::var_os("MARINA_HOME")
                .or_else(|| std::env::var_os("HOME"))
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map_or_else(
                    || PathBuf::from(".marina/tokens"),
                    |home| PathBuf::from(home).join(".marina/tokens"),
                )
        },
        PathBuf::from,
    )
}

/// Create a new opaque bearer token and return it once to the caller.
pub fn create_token(label: &str) -> io::Result<String> {
    let path = token_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
    }
    let token = random_token()?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    restrict_permissions(&file)?;
    writeln!(file, "{}\t{}", token, sanitize_label(label))?;
    Ok(token)
}

/// Validate a bearer token against the local token database.
#[must_use]
pub fn authenticate(candidate: &str) -> bool {
    fs::read_to_string(token_file()).is_ok_and(|contents| {
        contents.lines().any(|line| {
            line.split_once('\t')
                .is_some_and(|(token, _)| constant_time_eq(token.as_bytes(), candidate.as_bytes()))
        })
    })
}

fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .filter(|character| !character.is_control() && *character != '\t')
        .take(64)
        .collect()
}

fn random_token() -> io::Result<String> {
    let mut bytes = [0_u8; 32];
    if let Ok(mut source) = fs::File::open("/dev/urandom") {
        source.read_exact(&mut bytes)?;
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = ((now >> ((index % 16) * 8)) as u8) ^ (std::process::id() as u8);
        }
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(left.get(index).copied().unwrap_or_default())
            ^ usize::from(right.get(index).copied().unwrap_or_default());
    }
    difference == 0
}

fn restrict_permissions(file: &std::fs::File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[allow(dead_code)]
fn _path_is_private(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o077 == 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_tokens_without_prefix_confusion() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
