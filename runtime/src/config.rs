//! Runtime configuration boundary.

use oid_shared::RuntimeConfig;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

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
    for (line_number, line) in contents.lines().enumerate() {
        let line = line.split_once('#').map_or(line, |(value, _)| value).trim();
        if line.is_empty() {
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
        values.insert(key.to_owned(), value.to_owned());
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::read_config_file;

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
}
