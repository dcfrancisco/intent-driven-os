//! Local bearer-token management for Talvik clients.

use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Scope required to discover models.
pub const MODELS_READ: &str = "models:read";
/// Scope required to run inference.
pub const INFERENCE: &str = "inference";
/// Scope required to cancel another principal's request.
pub const INFERENCE_CANCEL_ANY: &str = "inference:cancel:any";
/// Scope required to load, unload, or acquire models.
pub const MODELS_ADMIN: &str = "models:admin";

/// Authenticated bearer-token identity and scopes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenIdentity {
    /// Stable local principal label.
    pub principal: String,
    /// Explicit capabilities granted to the token.
    pub scopes: Vec<String>,
}

impl TokenIdentity {
    /// Return whether this identity has a scope.
    #[must_use]
    pub fn allows(&self, scope: &str) -> bool {
        self.scopes.iter().any(|candidate| candidate == scope)
    }
}

/// Return the local token database path.
#[must_use]
pub fn token_file() -> PathBuf {
    std::env::var_os("TALVIK_TOKEN_FILE")
        .or_else(|| std::env::var_os("MARINA_TOKEN_FILE"))
        .map_or_else(
            || {
                crate::config::talvik_home_directory().map_or_else(
                    || PathBuf::from(".marina/tokens"),
                    |home| home.join("tokens"),
                )
            },
            PathBuf::from,
        )
}

/// Create a new opaque bearer token and return it once to the caller.
///
/// # Errors
///
/// Returns an I/O error when the token database cannot be created or written.
pub fn create_token(label: &str) -> io::Result<String> {
    create_token_with_scopes(label, &[MODELS_READ, INFERENCE, "inference:cancel:self"])
}

/// Create a token with explicitly configured scopes.
///
/// # Errors
///
/// Returns an I/O error when the token database cannot be created or written.
pub fn create_token_with_scopes(label: &str, scopes: &[&str]) -> io::Result<String> {
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
    writeln!(
        file,
        "{}\t{}\t{}",
        token,
        sanitize_label(label),
        scopes.join(",")
    )?;
    Ok(token)
}

/// Validate a bearer token against the local token database.
#[must_use]
pub fn authenticate(candidate: &str) -> bool {
    identity(candidate).is_some()
}

/// Authenticate a bearer token and return its local principal and scopes.
#[must_use]
pub fn identity(candidate: &str) -> Option<TokenIdentity> {
    fs::read_to_string(token_file()).ok().and_then(|contents| {
        contents.lines().find_map(|line| {
            let mut fields = line.split('\t');
            let token = fields.next()?;
            if !constant_time_eq(token.as_bytes(), candidate.as_bytes()) {
                return None;
            }
            let principal = fields.next().unwrap_or("local-client").to_owned();
            let scopes = fields.next().map_or_else(
                || {
                    vec![
                        MODELS_READ.to_owned(),
                        INFERENCE.to_owned(),
                        "inference:cancel:self".to_owned(),
                    ]
                },
                |value| {
                    value
                        .split(',')
                        .filter(|scope| !scope.trim().is_empty())
                        .map(str::to_owned)
                        .collect()
                },
            );
            Some(TokenIdentity { principal, scopes })
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
            let time_byte = u8::try_from((now >> ((index % 16) * 8)) & 0xff).unwrap_or_default();
            let process_byte = u8::try_from(std::process::id() % 256).unwrap_or_default();
            *byte = time_byte ^ process_byte;
        }
    }
    let mut token = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
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

#[allow(dead_code, clippy::verbose_bit_mask)]
fn _path_is_private(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o077 == 0)
            .unwrap_or(false)
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

    #[test]
    fn token_identity_requires_explicit_scopes() {
        let identity = TokenIdentity {
            principal: "client-a".to_owned(),
            scopes: vec![MODELS_READ.to_owned(), INFERENCE.to_owned()],
        };
        assert!(identity.allows(MODELS_READ));
        assert!(identity.allows(INFERENCE));
        assert!(!identity.allows(MODELS_ADMIN));
        assert!(!identity.allows(INFERENCE_CANCEL_ANY));
    }
}
