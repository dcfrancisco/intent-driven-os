//! Minimal, dependency-free MarinaOS foundation daemon.
//!
//! The socket is intentionally read-only in this first protocol version. Its
//! filesystem ownership and mode are the local authentication boundary; no
//! request can execute a shell command or mutate host state.
#![forbid(unsafe_code)]
#![allow(clippy::doc_markdown)]

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG: &str = "/etc/marinaos/marinaos.conf";
const STATE: &str = "/var/lib/marinaos";
const READY: &str = "/run/marinaos/ready.json";
const SOCKET: &str = "/run/marinaos/marinaos.sock";

#[derive(Clone, Debug)]
struct Config {
    state: PathBuf,
    ready: PathBuf,
    socket: PathBuf,
    talvik_optional: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            state: STATE.into(),
            ready: READY.into(),
            socket: SOCKET.into(),
            talvik_optional: true,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("marinaosd: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config_path = env::args()
        .skip(1)
        .find_map(|arg| arg.strip_prefix("--config=").map(str::to_owned))
        .unwrap_or_else(|| env::var("MARINAOS_CONFIG").unwrap_or_else(|_| CONFIG.to_owned()));
    let config = load_config(Path::new(&config_path))?;
    fs::create_dir_all(&config.state).map_err(|e| format!("cannot create state directory: {e}"))?;
    if let Some(parent) = config.ready.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create runtime directory: {e}"))?;
    }
    if let Some(parent) = config.socket.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create socket directory: {e}"))?;
    }
    let _ = fs::remove_file(&config.socket);
    let listener = UnixListener::bind(&config.socket)
        .map_err(|e| format!("cannot bind {}: {e}", config.socket.display()))?;
    fs::set_permissions(&config.socket, fs::Permissions::from_mode(0o660))
        .map_err(|e| format!("cannot secure socket: {e}"))?;
    let ready = format!(
        "{{\"service\":\"marinaosd\",\"protocol\":1,\"pid\":{},\"started_unix\":{}}}\n",
        std::process::id(),
        unix_time()
    );
    let temp = config.ready.with_extension("tmp");
    fs::write(&temp, ready).map_err(|e| format!("cannot write readiness state: {e}"))?;
    fs::rename(&temp, &config.ready).map_err(|e| format!("cannot publish readiness state: {e}"))?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = config.clone();
                thread::spawn(move || handle(stream, &config));
            }
            Err(error) => eprintln!("marinaosd: socket accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle(mut stream: UnixStream, config: &Config) {
    let request = BufReader::new(&mut stream)
        .lines()
        .next()
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_default();
    let response = match request_method(&request) {
        Some("status") => format!("{{\"version\":1,\"service\":\"ready\",\"talvik\":\"optional\",\"state_directory\":\"{}\"}}\n", escape(&config.state.display().to_string())),
        Some("doctor") => format!("{{\"version\":1,\"config\":\"ok\",\"state_directory\":\"ok\",\"architecture\":\"{}\",\"offline_mode\":\"available\"}}\n", env::consts::ARCH),
        Some("hardware") => hardware_json(),
        Some("version") => "{\"version\":1,\"daemon\":\"marinaosd\"}\n".to_owned(),
        _ => "{\"version\":1,\"error\":\"unsupported_request\",\"detail\":\"read-only methods are status, doctor, hardware, and version\"}\n".to_owned(),
    };
    let _ = stream.write_all(response.as_bytes());
}

fn request_method(request: &str) -> Option<&'static str> {
    match request.trim() {
        "{\"version\":1,\"method\":\"status\"}" => Some("status"),
        "{\"version\":1,\"method\":\"doctor\"}" => Some("doctor"),
        "{\"version\":1,\"method\":\"hardware\"}" => Some("hardware"),
        "{\"version\":1,\"method\":\"version\"}" => Some("version"),
        _ => None,
    }
}

fn load_config(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    let mut config = Config::default();
    for (line, raw) in contents.lines().enumerate() {
        let raw = raw.split('#').next().unwrap_or_default().trim();
        if raw.is_empty() {
            continue;
        }
        let (key, value) = raw
            .split_once('=')
            .ok_or_else(|| format!("line {} must use key=value", line + 1))?;
        let value = value.trim().trim_matches('"');
        if value.is_empty() {
            return Err(format!("line {} has an empty value", line + 1));
        }
        match key.trim() {
            "state_directory" => config.state = value.into(),
            "readiness_file" => config.ready = value.into(),
            "socket_path" => config.socket = value.into(),
            "talvik_optional" => {
                config.talvik_optional = value.parse().map_err(|_| {
                    format!("line {} talvik_optional must be true or false", line + 1)
                })?;
            }
            other => return Err(format!("line {} has unknown key {other}", line + 1)),
        }
    }
    Ok(config)
}

fn hardware_json() -> String {
    let cpu = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|v| {
            v.lines()
                .find_map(|l| l.strip_prefix("model name\t: ").map(str::to_owned))
        })
        .unwrap_or_else(|| "unknown CPU".to_owned());
    let cores = thread::available_parallelism().map_or(1, usize::from);
    format!("{{\"version\":1,\"operating_system\":\"linux\",\"architecture\":\"{}\",\"cpu\":\"{}\",\"logical_cores\":{},\"gpu\":null,\"npu\":null}}\n", env::consts::ARCH, escape(&cpu), cores)
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::request_method;

    #[test]
    fn accepts_only_versioned_phase_one_requests() {
        assert_eq!(
            request_method("{\"version\":1,\"method\":\"status\"}"),
            Some("status")
        );
        assert_eq!(request_method("{\"method\":\"status\"}"), None);
        assert_eq!(request_method("{\"version\":1,\"method\":\"status\""), None);
    }
}
