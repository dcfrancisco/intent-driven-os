//! Small, dependency-free control and diagnostic utility for the Marina OS base.
//!
//! This binary deliberately does not link to Talvik or the OID runtime. The
//! operating-system foundation must remain usable when inference is absent.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_CONFIG: &str = "/etc/marinaos/marinaos.conf";
const DEFAULT_STATE: &str = "/var/lib/marinaos";
const DEFAULT_READY: &str = "/run/marinaos/ready.json";
const DEFAULT_SOCKET: &str = "/run/marinaos/marinaos.sock";

#[derive(Clone, Debug, Eq, PartialEq)]
struct Config {
    state_directory: PathBuf,
    readiness_file: PathBuf,
    socket_path: PathBuf,
    talvik_optional: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            state_directory: PathBuf::from(DEFAULT_STATE),
            readiness_file: PathBuf::from(DEFAULT_READY),
            socket_path: PathBuf::from(DEFAULT_SOCKET),
            talvik_optional: true,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("marinaosctl: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let json = take_flag(&mut args, "--json");
    let config_path = take_option(&mut args, "--config").map_or_else(
        || {
            env::var_os("MARINAOS_CONFIG")
                .map_or_else(|| PathBuf::from(DEFAULT_CONFIG), PathBuf::from)
        },
        PathBuf::from,
    );
    let command = args.first().map_or("status", String::as_str);
    if command == "--help" || command == "help" {
        print_help();
        return Ok(());
    }
    if command == "version" || command == "--version" {
        println!("marinaosctl 0.1.0");
        return Ok(());
    }
    let config = load_config(&config_path)?;
    match command {
        "status" => query(&config, "status", json),
        "doctor" => query(&config, "doctor", json),
        "hardware" => query(&config, "hardware", json),
        "daemon" | "clear-ready" => Err(
            "marinaosctl no longer owns the daemon lifecycle; use systemctl restart marinaosd.service"
                .to_owned(),
        ),
        other => Err(format!("unknown command {other}; use marinaosctl help")),
    }
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|arg| arg == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn take_option(args: &mut Vec<String>, option: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == option)?;
    args.remove(index);
    if index >= args.len() {
        return None;
    }
    Some(args.remove(index))
}

fn load_config(path: &Path) -> Result<Config, String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "cannot read config {}: {error}; run marinaosctl doctor --config {}",
            path.display(),
            path.display()
        )
    })?;
    parse_config(&contents).map_err(|error| format!("invalid config {}: {error}", path.display()))
}

fn parse_config(contents: &str) -> Result<Config, String> {
    let mut config = Config::default();
    for (line_number, raw_line) in contents.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {} must use key=value", line_number + 1))?;
        let key = key.trim();
        let value = value.trim().trim_matches('"');
        if value.is_empty() {
            return Err(format!("line {} has an empty value", line_number + 1));
        }
        match key {
            "state_directory" => config.state_directory = PathBuf::from(value),
            "readiness_file" => config.readiness_file = PathBuf::from(value),
            "socket_path" => config.socket_path = PathBuf::from(value),
            "talvik_optional" => {
                config.talvik_optional = value.parse().map_err(|_| {
                    format!(
                        "line {} talvik_optional must be true or false",
                        line_number + 1
                    )
                })?;
            }
            _ => return Err(format!("line {} has unknown key {key}", line_number + 1)),
        }
    }
    Ok(config)
}

fn query(config: &Config, method: &str, json: bool) -> Result<(), String> {
    let request = format!("{{\"version\":1,\"method\":\"{method}\"}}\n");
    let mut stream = UnixStream::connect(&config.socket_path).map_err(|error| {
        format!(
            "marinaosd is unavailable at {}; start marinaosd.service and retry ({error})",
            config.socket_path.display()
        )
    })?;
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("cannot send request to marinaosd: {error}"))?;
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|error| format!("cannot read marinaosd response: {error}"))?;
    if response.contains("\"error\"") {
        return Err(response.trim().to_owned());
    }
    if json {
        print!("{response}");
    } else {
        println!("MarinaOS daemon response: {}", response.trim());
    }
    Ok(())
}

#[allow(dead_code)]
fn status(config: &Config, json: bool) -> Result<(), String> {
    let ready = config.readiness_file.is_file();
    let talvik = talvik_status();
    if json {
        println!(
            "{{\"foundation\":\"{}\",\"talvik\":\"{}\",\"talvik_optional\":{},\"state_directory\":\"{}\"}}",
            if ready { "ready" } else { "not_ready" },
            talvik,
            config.talvik_optional,
            json_escape(&config.state_directory.display().to_string())
        );
    } else {
        println!(
            "MarinaOS foundation: {}",
            if ready { "ready" } else { "not ready" }
        );
        println!("Talvik: {talvik} (optional)");
        println!("State: {}", config.state_directory.display());
    }
    if ready {
        Ok(())
    } else {
        Err("foundation is not ready; inspect journalctl -u marinaos-foundation or run marinaosctl doctor".to_owned())
    }
}

#[allow(dead_code)]
fn doctor(config: &Config, json: bool) -> Result<(), String> {
    let mut checks = BTreeMap::new();
    checks.insert("config", "ok");
    let state_ok = fs::create_dir_all(&config.state_directory).is_ok();
    checks.insert("state_directory", if state_ok { "ok" } else { "failed" });
    let arch_ok = env::consts::ARCH == "x86_64";
    checks.insert(
        "architecture",
        if arch_ok { "supported" } else { "unsupported" },
    );
    checks.insert("offline_mode", "available");
    if json {
        let fields = checks
            .iter()
            .map(|(key, value)| format!("\"{key}\":\"{value}\""));
        println!("{{{}}}", fields.collect::<Vec<_>>().join(","));
    } else {
        for (key, value) in &checks {
            println!("{key}: {value}");
        }
        println!("talvik: {} (optional)", talvik_status());
    }
    if state_ok && arch_ok {
        Ok(())
    } else {
        Err(
            "doctor found one or more blocking foundation checks; see the failing check above"
                .to_owned(),
        )
    }
}

#[allow(dead_code)]
fn hardware(json: bool) {
    let cpu = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|value| {
            value
                .lines()
                .find_map(|line| line.strip_prefix("model name\t: ").map(str::to_owned))
        })
        .unwrap_or_else(|| "unknown CPU".to_owned());
    let logical_cores = thread::available_parallelism().map_or(1, usize::from);
    let (memory_total, memory_available) = linux_memory();
    if json {
        println!(
            "{{\"operating_system\":\"linux\",\"architecture\":\"{}\",\"cpu\":\"{}\",\"logical_cores\":{},\"memory_total_bytes\":{},\"memory_available_bytes\":{},\"gpu\":null,\"npu\":null}}",
            env::consts::ARCH,
            json_escape(&cpu),
            logical_cores,
            memory_total.map_or_else(|| "null".to_owned(), |v| v.to_string()),
            memory_available.map_or_else(|| "null".to_owned(), |v| v.to_string())
        );
    } else {
        println!("OS: linux");
        println!("Architecture: {}", env::consts::ARCH);
        println!("CPU: {cpu}");
        println!("Logical cores: {logical_cores}");
        println!("Memory total: {}", display_bytes(memory_total));
        println!("Memory available: {}", display_bytes(memory_available));
        println!("GPU: not qualified");
        println!("NPU: not qualified");
    }
}

#[allow(dead_code)]
fn daemon(_config: &Config) -> Result<(), String> {
    Err("daemon lifecycle is owned by marinaosd.service".to_owned())
}

#[allow(dead_code)]
fn clear_ready(_config: &Config) -> Result<(), String> {
    Err("readiness lifecycle is owned by marinaosd.service".to_owned())
}

#[allow(dead_code)]
fn talvik_status() -> &'static str {
    match Command::new("systemctl")
        .args(["is-active", "--quiet", "talvikd.service"])
        .status()
    {
        Ok(status) if status.success() => "running",
        Ok(_) => "not running",
        Err(_) => "unavailable",
    }
}

#[allow(dead_code)]
fn linux_memory() -> (Option<u64>, Option<u64>) {
    let Ok(contents) = fs::read_to_string("/proc/meminfo") else {
        return (None, None);
    };
    let mut total = None;
    let mut available = None;
    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let Some(value) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        match name {
            "MemTotal:" => total = Some(value.saturating_mul(1024)),
            "MemAvailable:" => available = Some(value.saturating_mul(1024)),
            _ => {}
        }
    }
    (total, available)
}

#[allow(dead_code)]
fn display_bytes(value: Option<u64>) -> String {
    value.map_or_else(|| "unknown".to_owned(), |bytes| format!("{bytes} bytes"))
}

#[allow(dead_code)]
fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[allow(dead_code)]
fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn print_help() {
    println!(
        "MarinaOS Linux foundation control\n\nUsage: marinaosctl [--config PATH] [--json] <command>\n\nCommands:\n  status       Show foundation daemon status\n  doctor       Query foundation diagnostics\n  hardware     Query hardware capabilities\n  daemon       Deprecated; use systemctl for marinaosd.service\n  clear-ready  Deprecated; readiness is owned by marinaosd"
    );
}

#[cfg(test)]
mod tests {
    use super::{parse_config, Config};

    #[test]
    fn parses_valid_config() {
        let config = parse_config(
            "state_directory=/tmp/marina\nreadiness_file=/tmp/marina.ready\ntalvik_optional=true\n",
        )
        .expect("config");
        assert_eq!(config.state_directory, std::path::Path::new("/tmp/marina"));
        assert!(config.talvik_optional);
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(parse_config("network=true\n").is_err());
    }

    #[test]
    fn defaults_are_safe() {
        assert!(Config::default().talvik_optional);
    }
}
