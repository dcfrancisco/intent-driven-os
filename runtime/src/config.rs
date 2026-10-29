//! Runtime configuration boundary.

use oid_shared::RuntimeConfig;
use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;

const DEFAULT_MODEL_ID: &str = "gemma-4-e4b-it-q4_0";
const DEFAULT_MODEL_SOURCE: &str = "https://huggingface.co/ggml-org/gemma-4-E4B-it-GGUF/resolve/a69f018d7678a2b9f5e705f8903d6fb29b9b25dc/gemma-4-E4B-it-Q4_0.gguf";
const DEFAULT_MODEL_SHA256: &str =
    "a555b900214b477d8880e7832e0b8925e139b0159640036b09fe472b6f2097f2";
const DEFAULT_VISION_PROJECTOR_SOURCE: &str = "https://huggingface.co/ggml-org/gemma-4-E4B-it-GGUF/resolve/a69f018d7678a2b9f5e705f8903d6fb29b9b25dc/mmproj-gemma-4-E4B-it-Q8_0.gguf";
const DEFAULT_VISION_PROJECTOR_SHA256: &str =
    "197f49a93027f9843772bd24a6a9e0be2a32a788de5a3def330e9c585d86edd1";

/// Default model and optional projector to acquire and load when Talvik starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultModelConfig {
    /// Model identifier to load.
    pub id: String,
    /// Optional source used to acquire a missing model.
    pub source: Option<String>,
    /// Optional expected SHA-256 for acquisition.
    pub checksum: Option<String>,
    /// Optional source used to acquire a missing vision projector.
    pub vision_projector_source: Option<String>,
    /// Optional expected SHA-256 for the vision projector.
    pub vision_projector_checksum: Option<String>,
    /// Whether startup should provision this model when it is unavailable.
    pub auto_download: bool,
    /// Whether startup should load this model after it is available.
    pub auto_load: bool,
}

/// Native HTTP listener configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpConfig {
    /// Optional loopback address. `None` keeps HTTP disabled.
    pub address: Option<String>,
    /// Require a valid Talvik bearer token for API routes.
    pub authentication_enabled: bool,
}

/// Load the native HTTP listener configuration.
///
/// # Errors
///
/// Returns an error when the configuration file cannot be read or the
/// configured listener address is invalid or unsafe.
pub fn http_config() -> Result<HttpConfig, String> {
    let values = read_config_file(&config_file())?;
    let configured_address = values
        .get("server.listeners.0.bind_address")
        .zip(values.get("server.listeners.0.port"))
        .map(|(host, port)| format!("{host}:{port}"));
    let nested_transport = values.get("server.listeners.0.transport");
    let address = std::env::var("TALVIK_HTTP_ADDR")
        .or_else(|_| std::env::var("MARINA_HTTP_ADDR"))
        .ok()
        .or_else(|| values.get("http_address").cloned())
        .or_else(|| values.get("server_http_address").cloned())
        .or(configured_address)
        .filter(|_| nested_transport.is_none_or(|transport| transport == "tcp"));
    if let Some(address) = address.as_deref() {
        let parsed = address
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid TALVIK_HTTP_ADDR {address}: {error}"))?;
        let allow_non_loopback = std::env::var("TALVIK_HTTP_ALLOW_NON_LOOPBACK")
            .or_else(|_| std::env::var("MARINA_HTTP_ALLOW_NON_LOOPBACK"))
            .ok()
            .or_else(|| values.get("http_allow_non_loopback").cloned())
            .is_some_and(|value| {
                matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes")
            });
        if !parsed.ip().is_loopback() && !allow_non_loopback {
            return Err(format!(
                "HTTP address {address} is not loopback; set TALVIK_HTTP_ALLOW_NON_LOOPBACK=true only for an explicitly secured deployment"
            ));
        }
    }
    let authentication_enabled = std::env::var("TALVIK_HTTP_AUTH")
        .or_else(|_| std::env::var("MARINA_HTTP_AUTH"))
        .ok()
        .or_else(|| values.get("server.authentication.enabled").cloned())
        .or_else(|| values.get("http_authentication_enabled").cloned())
        .is_none_or(|value| !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "no"));
    Ok(HttpConfig {
        address,
        authentication_enabled,
    })
}

/// Return the current user's home directory across supported platforms.
#[must_use]
pub fn home_directory() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Return Talvik's canonical user-scoped home directory.
#[must_use]
pub fn talvik_home_directory() -> Option<std::path::PathBuf> {
    std::env::var_os("TALVIK_HOME")
        .or_else(|| std::env::var_os("MARINA_HOME"))
        .map_or_else(
            || home_directory().map(|home| home.join(".marina")),
            |path| Some(std::path::PathBuf::from(path)),
        )
}

/// Return the user-scoped YAML configuration path.
#[must_use]
pub fn config_file() -> PathBuf {
    std::env::var_os("TALVIK_CONFIG")
        .or_else(|| std::env::var_os("MARINA_CONFIG"))
        .map_or_else(
            || {
                talvik_home_directory().map_or_else(
                    || PathBuf::from(".marina/config.yaml"),
                    |home| home.join("config.yaml"),
                )
            },
            PathBuf::from,
        )
}

/// Load the configured default model.
///
/// The built-in Gemma 4 default and its pinned vision projector are acquired
/// from Hugging Face when missing. Set `default_model` to `none` to disable
/// automatic loading.
///
/// # Errors
///
/// Returns an error when the configuration file or an environment override
/// contains invalid non-UTF-8 data.
pub fn default_model() -> Result<Option<DefaultModelConfig>, String> {
    let values = read_config_file(&config_file())?;
    let model_value = configured_value(
        "TALVIK_DEFAULT_MODEL",
        "MARINA_DEFAULT_MODEL",
        "default_model",
        &values,
    )?
    .unwrap_or_else(|| DEFAULT_MODEL_ID.to_owned());
    let Some(id) = setting_value(&model_value) else {
        return Ok(None);
    };

    let source = configured_value(
        "TALVIK_DEFAULT_MODEL_SOURCE",
        "MARINA_DEFAULT_MODEL_SOURCE",
        "default_model_source",
        &values,
    )?
    .map_or_else(
        || (id == DEFAULT_MODEL_ID).then(|| DEFAULT_MODEL_SOURCE.to_owned()),
        |value| setting_value(&value),
    );
    let checksum = configured_value(
        "TALVIK_DEFAULT_MODEL_SHA256",
        "MARINA_DEFAULT_MODEL_SHA256",
        "default_model_sha256",
        &values,
    )?
    .map_or_else(
        || (id == DEFAULT_MODEL_ID).then(|| DEFAULT_MODEL_SHA256.to_owned()),
        |value| setting_value(&value),
    );
    let vision_projector_source = configured_value(
        "TALVIK_DEFAULT_VISION_PROJECTOR_SOURCE",
        "MARINA_DEFAULT_VISION_PROJECTOR_SOURCE",
        "default_vision_projector_source",
        &values,
    )?
    .map_or_else(
        || (id == DEFAULT_MODEL_ID).then(|| DEFAULT_VISION_PROJECTOR_SOURCE.to_owned()),
        |value| setting_value(&value),
    );
    let vision_projector_checksum = configured_value(
        "TALVIK_DEFAULT_VISION_PROJECTOR_SHA256",
        "MARINA_DEFAULT_VISION_PROJECTOR_SHA256",
        "default_vision_projector_sha256",
        &values,
    )?
    .map_or_else(
        || (id == DEFAULT_MODEL_ID).then(|| DEFAULT_VISION_PROJECTOR_SHA256.to_owned()),
        |value| setting_value(&value),
    );

    let auto_bootstrap = configured_bool(
        "TALVIK_AUTO_BOOTSTRAP",
        "MARINA_AUTO_BOOTSTRAP",
        "auto_bootstrap",
        &values,
        true,
    )?;
    let auto_download = configured_bool(
        "TALVIK_AUTO_DOWNLOAD",
        "MARINA_AUTO_DOWNLOAD",
        "auto_download",
        &values,
        auto_bootstrap,
    )?;
    let auto_load = configured_bool(
        "TALVIK_AUTO_LOAD",
        "MARINA_AUTO_LOAD",
        "auto_load",
        &values,
        auto_bootstrap,
    )?;

    Ok(Some(DefaultModelConfig {
        id,
        source,
        checksum,
        vision_projector_source,
        vision_projector_checksum,
        auto_download,
        auto_load,
    }))
}

fn configured_bool(
    primary: &str,
    compatibility: &str,
    key: &str,
    values: &BTreeMap<String, String>,
    default: bool,
) -> Result<bool, String> {
    let value = configured_value(primary, compatibility, key, values)?;
    Ok(value.map_or(default, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    }))
}

fn configured_value(
    primary: &str,
    compatibility: &str,
    key: &str,
    values: &BTreeMap<String, String>,
) -> Result<Option<String>, String> {
    for name in [primary, compatibility] {
        if let Some(value) = std::env::var_os(name) {
            return value
                .into_string()
                .map(Some)
                .map_err(|_| format!("{name} must contain valid UTF-8"));
        }
    }
    Ok(values.get(key).cloned())
}

fn setting_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "none" | "disabled" | "off"
        )
    {
        None
    } else {
        Some(value.to_owned())
    }
}

/// Load the deterministic Phase 1 configuration.
///
/// # Errors
///
/// Returns a validation message if the foundation configuration is invalid.
pub fn load() -> Result<RuntimeConfig, String> {
    let mut config = RuntimeConfig::default();
    let file_values = read_config_file(&config_file())?;
    let state = std::env::var_os("TALVIK_STATE_DIR")
        .or_else(|| std::env::var_os("MARINA_STATE_DIR"))
        .map_or_else(
            || {
                file_values
                    .get("state_directory")
                    .map(|value| configured_path(value))
                    .or_else(talvik_home_directory)
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
    // Talvik store and compatible local caches. An explicit directory is an
    // isolation boundary used by tests and operators.
    config.model_directory = std::env::var_os("TALVIK_MODEL_DIRECTORY")
        .or_else(|| std::env::var_os("TALVIK_MODEL_DIR"))
        .or_else(|| std::env::var_os("MARINA_MODEL_DIRECTORY"))
        .or_else(|| std::env::var_os("MARINA_MODEL_DIR"))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            file_values
                .get("model_directory")
                .map(|value| configured_path(value))
        });
    config.registry_path = std::env::var_os("TALVIK_REGISTRY")
        .or_else(|| std::env::var_os("MARINA_REGISTRY"))
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
                return Err(format!("invalid Talvik config at line {}", line_number + 1));
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
            return Err(format!("invalid Talvik config at line {}", line_number + 1));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!(
                "empty Talvik config key at line {}",
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
    use super::{
        configured_bool, http_config, read_config_file, setting_value, DEFAULT_MODEL_ID,
        DEFAULT_MODEL_SHA256, DEFAULT_MODEL_SOURCE, DEFAULT_VISION_PROJECTOR_SHA256,
        DEFAULT_VISION_PROJECTOR_SOURCE,
    };
    use std::collections::BTreeMap;

    #[test]
    fn default_model_can_be_disabled_or_overridden() {
        assert_eq!(
            setting_value(DEFAULT_MODEL_ID),
            Some(DEFAULT_MODEL_ID.to_owned())
        );
        assert_eq!(setting_value("none"), None);
        assert_eq!(setting_value("  "), None);
        assert_eq!(
            setting_value("custom-model"),
            Some("custom-model".to_owned())
        );
    }

    #[test]
    fn default_model_is_pinned_gemma_4_with_a_vision_projector() {
        assert_eq!(DEFAULT_MODEL_ID, "gemma-4-e4b-it-q4_0");
        assert!(DEFAULT_MODEL_SOURCE.contains(
            "/resolve/a69f018d7678a2b9f5e705f8903d6fb29b9b25dc/gemma-4-E4B-it-Q4_0.gguf"
        ));
        assert_eq!(
            DEFAULT_MODEL_SHA256,
            "a555b900214b477d8880e7832e0b8925e139b0159640036b09fe472b6f2097f2"
        );
        assert!(DEFAULT_VISION_PROJECTOR_SOURCE.contains(
            "/resolve/a69f018d7678a2b9f5e705f8903d6fb29b9b25dc/mmproj-gemma-4-E4B-it-Q8_0.gguf"
        ));
        assert_eq!(
            DEFAULT_VISION_PROJECTOR_SHA256,
            "197f49a93027f9843772bd24a6a9e0be2a32a788de5a3def330e9c585d86edd1"
        );
    }

    #[test]
    fn bootstrap_flags_default_to_enabled_and_accept_false_values() {
        let values = BTreeMap::new();
        assert!(configured_bool(
            "TALVIK_AUTO_BOOTSTRAP",
            "MARINA_AUTO_BOOTSTRAP",
            "auto_bootstrap",
            &values,
            true,
        )
        .expect("default bootstrap flag"));

        let values = BTreeMap::from([("auto_download".to_owned(), "false".to_owned())]);
        assert!(!configured_bool(
            "TALVIK_AUTO_DOWNLOAD",
            "MARINA_AUTO_DOWNLOAD",
            "auto_download",
            &values,
            true,
        )
        .expect("configured download flag"));
    }

    #[test]
    fn reads_small_yaml_key_value_config() {
        let path = std::env::temp_dir().join(format!(
            "talvik-config-{}-{}.yaml",
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
        let previous = std::env::var_os("TALVIK_HTTP_ADDR");
        let previous_override = std::env::var_os("TALVIK_HTTP_ALLOW_NON_LOOPBACK");
        std::env::set_var("TALVIK_HTTP_ADDR", "0.0.0.0:11434");
        std::env::remove_var("TALVIK_HTTP_ALLOW_NON_LOOPBACK");
        assert!(http_config().is_err());
        match previous {
            Some(value) => std::env::set_var("TALVIK_HTTP_ADDR", value),
            None => std::env::remove_var("TALVIK_HTTP_ADDR"),
        }
        match previous_override {
            Some(value) => std::env::set_var("TALVIK_HTTP_ALLOW_NON_LOOPBACK", value),
            None => std::env::remove_var("TALVIK_HTTP_ALLOW_NON_LOOPBACK"),
        }
    }

    #[test]
    fn reads_nested_listener_schema_and_flattens_listener_fields() {
        let path = std::env::temp_dir().join(format!(
            "talvik-nested-config-{}-{}.yaml",
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
