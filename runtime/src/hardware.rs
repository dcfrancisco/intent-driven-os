//! Platform-neutral hardware discovery service.

use oid_shared::{EventBus, RuntimeEvent};
use std::collections::BTreeSet;
use std::fs;

/// Normalized hardware inventory exposed to runtime clients.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HardwareSnapshot {
    /// Operating-system identifier.
    pub operating_system: String,
    /// Target architecture identifier.
    pub architecture: String,
    /// Human-readable CPU description.
    pub cpu: String,
    /// Number of logical CPU cores.
    pub logical_cores: usize,
    /// Number of physical CPU cores when discoverable.
    pub physical_cores: Option<usize>,
    /// Installed system RAM in bytes when discoverable.
    pub installed_ram_bytes: Option<u64>,
    /// Available system RAM in bytes when discoverable.
    pub available_ram_bytes: Option<u64>,
    /// Safe, normalized SIMD capability names.
    pub simd_capabilities: Vec<String>,
    /// GPU discovery remains a placeholder in this milestone.
    pub gpu: Option<String>,
    /// NPU discovery remains a placeholder in this milestone.
    pub npu: Option<String>,
}

/// Provider interface for runtime hardware consumers.
pub trait HardwareProvider: Send + Sync {
    /// Return the current hardware snapshot.
    fn snapshot(&self) -> HardwareSnapshot;
}

/// Hardware service with platform-specific discovery isolated internally.
#[derive(Clone, Debug)]
pub struct HardwareService {
    snapshot: HardwareSnapshot,
}

impl HardwareService {
    /// Detect portable hardware facts and publish a completion event.
    #[must_use]
    pub fn detect(bus: &EventBus) -> Self {
        let service = Self {
            snapshot: detect_snapshot(),
        };
        bus.publish(&RuntimeEvent::HardwareDetected);
        service
    }

    /// Return the normalized snapshot.
    #[must_use]
    pub fn snapshot(&self) -> HardwareSnapshot {
        self.snapshot.clone()
    }
}

impl HardwareProvider for HardwareService {
    fn snapshot(&self) -> HardwareSnapshot {
        self.snapshot()
    }
}

/// Empty provider retained for lower-level runtime tests.
#[derive(Clone, Debug, Default)]
pub struct UnavailableHardware;

impl HardwareProvider for UnavailableHardware {
    fn snapshot(&self) -> HardwareSnapshot {
        HardwareSnapshot {
            operating_system: "unknown".to_owned(),
            architecture: "unknown".to_owned(),
            ..HardwareSnapshot::default()
        }
    }
}

fn detect_snapshot() -> HardwareSnapshot {
    let logical_cores = std::thread::available_parallelism().map_or(1, usize::from);
    let (cpu, physical_cores) =
        linux_cpu_details().unwrap_or_else(|| ("unknown CPU".to_owned(), None));
    let (installed_ram_bytes, available_ram_bytes) = linux_memory_details();
    HardwareSnapshot {
        operating_system: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        cpu,
        logical_cores,
        physical_cores: physical_cores.or(Some(logical_cores)),
        installed_ram_bytes,
        available_ram_bytes,
        simd_capabilities: simd_capabilities(),
        gpu: None,
        npu: None,
    }
}

fn linux_cpu_details() -> Option<(String, Option<usize>)> {
    let contents = fs::read_to_string("/proc/cpuinfo").ok()?;
    let mut cpu = None;
    let mut pairs = BTreeSet::new();
    let mut physical_id = None;
    let mut core_id = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("model name\t: ") {
            cpu.get_or_insert_with(|| value.to_owned());
        }
        if let Some(value) = line.strip_prefix("physical id\t: ") {
            physical_id = value.parse::<usize>().ok();
        }
        if let Some(value) = line.strip_prefix("core id\t\t: ") {
            core_id = value.parse::<usize>().ok();
        }
        if line.is_empty() {
            if let (Some(physical), Some(core)) = (physical_id.take(), core_id.take()) {
                pairs.insert((physical, core));
            }
        }
    }
    if let (Some(physical), Some(core)) = (physical_id, core_id) {
        pairs.insert((physical, core));
    }
    Some((
        cpu.unwrap_or_else(|| "unknown CPU".to_owned()),
        (!pairs.is_empty()).then_some(pairs.len()),
    ))
}

fn linux_memory_details() -> (Option<u64>, Option<u64>) {
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
        let bytes = value.saturating_mul(1024);
        match name {
            "MemTotal:" => total = Some(bytes),
            "MemAvailable:" => available = Some(bytes),
            _ => {}
        }
    }
    (total, available)
}

fn simd_capabilities() -> Vec<String> {
    let mut capabilities = Vec::new();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sse2") {
            capabilities.push("sse2".to_owned());
        }
        if std::is_x86_feature_detected!("avx") {
            capabilities.push("avx".to_owned());
        }
        if std::is_x86_feature_detected!("avx2") {
            capabilities.push("avx2".to_owned());
        }
    }
    capabilities
}

#[cfg(test)]
mod tests {
    use super::HardwareService;
    use oid_shared::EventBus;

    #[test]
    fn detects_portable_snapshot_without_accelerator_requirements() {
        let service = HardwareService::detect(&EventBus::new());
        let snapshot = service.snapshot();
        assert!(!snapshot.operating_system.is_empty());
        assert!(!snapshot.architecture.is_empty());
        assert!(snapshot.logical_cores > 0);
    }
}
