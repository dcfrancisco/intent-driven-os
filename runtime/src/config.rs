//! Runtime configuration boundary.

use oid_shared::RuntimeConfig;
use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Native HTTP listener configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpConfig {
    /// Optional loopback address. `None` keeps HTTP disabled.
    pub address: Option<String>,
    /// Require a valid Marina bearer token for API routes.
    pub authentication_enabled: bool,
}

/// Load the native HTTP listener configuration.
pub fn http_config() -> Result<HttpConfig, String> {
    let values = read_config_file(&config_file())?;
    let configured_address = values
        .get("server.listeners.0.bind_address")
        .zip(values.get("server.listeners.0.port"))
        .map(|(host, port)| format!("{host}:{port}"));
    let nested_transport = values.get("server.listeners.0.transport");
    let address = std::env::var("MARINA_HTTP_ADDR")
        .ok()
        .or_else(|| values.get("http_address").cloned())
        .or_else(|| values.get("server_http_address").cloned())
        .or(configured_address)
        .filter(|_| nested_transport.is_none_or(|transport| transport == "tcp"));
    if let Some(address) = address.as_deref() {
        let parsed = address
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid MARINA_HTTP_ADDR {address}: {error}"))?;
        let allow_non_loopback = std::env::var("MARINA_HTTP_ALLOW_NON_LOOPBACK")
            .ok()
            .or_else(|| values.get("http_allow_non_loopback").cloned())
            .is_some_and(|value| {
                matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
            });
        if !parsed.ip().is_loopback() && !allow_non_loopback {
            return Err(format!(
                "HTTP address {address} is not loopback; set MARINA_HTTP_ALLOW_NON_LOOPBACK=true only for an explicitly secured deployment"
            ));
        }
    }
    let authentication_enabled = std::env::var("MARINA_HTTP_AUTH")
        .ok()
        .or_else(|| values.get("server.authentication.enabled").cloned())
        .or_else(|| values.get("http_authentication_enabled").cloned())
        .map_or(true, |value| {
            !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "no")
        });
    Ok(HttpConfig {
        address,
        authentication_enabled,
    })
}

/// Return the current user's home directory across supported platforms.
#[must_use]
pub fn home_directory() -> Option<std::path::PathBuf> {
    std::env::var_os("MARINA_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Return the user-scoped YAML configuration path.
#[must_use]
pub fn config_file() -> PathBuf {
    std::env::var_os("MARINA_CONFIG").map_or_else(
        || {
            home_directory().map_or_else(
                || PathBuf::from(".marina/config.yaml"),
                |home| home.join(".marina/config.yaml"),
            )
        },
        PathBuf::from,
    )
}

/// Load the deterministic Phase 1 configuration.
///
/// # Errors
///
/// Returns a validation message if the foundation configuration is invalid.
pub fn load() -> Result<RuntimeConfig, String> {
    let mut config = RuntimeConfig::default();
    let file_values = read_config_file(&config_file())?;
    let state = std::env::var_os("MARINA_STATE_DIR").map_or_else(
        || {
            file_values
                .get("state_directory")
                .map(|value| configured_path(value))
                .or_else(|| home_directory().map(|home| home.join(".marina")))
        },
        |path| Some(std::path::PathBuf::from(path)),
    );
    if let Some(name) = file_values.get("name") {
        config.name.clone_from(name);
    }
    if let Some(version) = file_values.get("version") {
        config.version.clone_from(version);
    }
    // Leave the model directory unset unless explicitly configured. The
    // runtime then uses its complete discovery set, including the canonical
    // Marina store and compatible local caches. An explicit directory is an
    // isolation boundary used by tests and operators.
    config.model_directory = std::env::var_os("MARINA_MODEL_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            file_values
                .get("model_directory")
                .map(|value| configured_path(value))
        });
    config.registry_path = std::env::var_os("MARINA_REGISTRY")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            file_values
                .get("registry_path")
                .map(|value| configured_path(value))
        })
        .or_else(|| state.map(|path| path.join("models.registry")));
    config.validate().map(|()| config)
}

fn configured_path(value: &str) -> PathBuf {
    if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home_directory().map_or_else(|| PathBuf::from(value), |home| home.join(relative));
    }
    PathBuf::from(value)
}

fn read_config_file(path: &std::path::Path) -> Result<BTreeMap<String, String>, String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Ok(BTreeMap::new());
    };
    let mut values = BTreeMap::new();
    let mut sections: Vec<(usize, String)> = Vec::new();
    let mut listener_index = None;
    for (line_number, raw_line) in contents.lines().enumerate() {
        let without_comment = raw_line
            .split_once('#')
            .map_or(raw_line, |(value, _)| value);
        let indent = without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let line = without_comment.trim();
        if line.is_empty() {
            continue;
        }
        while sections.last().is_some_and(|(level, _)| *level >= indent) {
            sections.pop();
        }
        if let Some(item) = line.strip_prefix("- ") {
            if item.starts_with("name:") {
                listener_index = Some(listener_index.map_or(0, |index| index + 1));
                sections.push((indent, listener_index.unwrap_or_default().to_string()));
            }
            let line = item;
            let Some((key, value)) = line.split_once(':') else {
                return Err(format!("invalid Marina config at line {}", line_number + 1));
            };
            let mut prefix = sections
                .iter()
                .map(|(_, key)| key.clone())
                .collect::<Vec<_>>();
            prefix.push(key.trim().to_owned());
            let value = value.trim().trim_matches(['"', '\'']);
            values.insert(prefix.join("."), value.to_owned());
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("invalid Marina config at line {}", line_number + 1));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!(
                "empty Marina config key at line {}",
                line_number + 1
            ));
        }
        let value = value.trim().trim_matches(['"', '\'']);
        let mut prefix = sections
            .iter()
            .map(|(_, key)| key.clone())
            .collect::<Vec<_>>();
        prefix.push(key.to_owned());
        if value.is_empty() {
            sections.push((indent, key.to_owned()));
        } else {
            values.insert(prefix.join("."), value.to_owned());
            values.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{http_config, read_config_file};

    #[test]
    fn reads_small_yaml_key_value_config() {
        let path = std::env::temp_dir().join(format!(
            "marina-config-{}-{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::write(&path, "model_directory: /tmp/models\n# comment\n").expect("write");
        let values = read_config_file(&path).expect("parse");
        assert_eq!(
            values.get("model_directory").map(String::as_str),
            Some("/tmp/models")
        );
        std::fs::remove_file(path).expect("remove");
    }

    #[test]
    fn rejects_non_loopback_http_without_explicit_override() {
        let previous = std::env::var_os("MARINA_HTTP_ADDR");
        let previous_override = std::env::var_os("MARINA_HTTP_ALLOW_NON_LOOPBACK");
        std::env::set_var("MARINA_HTTP_ADDR", "0.0.0.0:11434");
        std::env::remove_var("MARINA_HTTP_ALLOW_NON_LOOPBACK");
        assert!(http_config().is_err());
        match previous {
            Some(value) => std::env::set_var("MARINA_HTTP_ADDR", value),
            None => std::env::remove_var("MARINA_HTTP_ADDR"),
        }
        match previous_override {
            Some(value) => std::env::set_var("MARINA_HTTP_ALLOW_NON_LOOPBACK", value),
            None => std::env::remove_var("MARINA_HTTP_ALLOW_NON_LOOPBACK"),
        }
    }

    #[test]
    fn reads_nested_listener_schema_and_flattens_listener_fields() {
        let path = std::env::temp_dir().join(format!(
            "marina-nested-config-{}-{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::write(
            &path,
            "server:\n  listeners:\n    - name: local\n      transport: tcp\n      bind_address: 127.0.0.1\n      port: 12434\n  authentication:\n    enabled: true\n",
        )
        .expect("write");
        let values = read_config_file(&path).expect("parse nested config");
        assert_eq!(
            values
                .get("server.listeners.0.bind_address")
                .map(String::as_str),
            Some("127.0.0.1")
        );
        assert_eq!(
            values.get("server.listeners.0.port").map(String::as_str),
            Some("12434")
        );
        assert_eq!(
            values
                .get("server.authentication.enabled")
                .map(String::as_str),
            Some("true")
        );
        std::fs::remove_file(path).expect("remove");
    }
}
