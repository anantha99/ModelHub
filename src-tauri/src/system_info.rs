use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
use serde_json::Value;
use sysinfo::{Disk, Disks, System};

use crate::models::{CpuInfo, DiskInfo, GpuInfo, MemoryInfo, SystemInfo};

pub fn collect_system_info(hf_cache_path: Option<String>) -> SystemInfo {
    let mut system = System::new_all();
    system.refresh_all();

    let disks = Disks::new_with_refreshed_list();
    let hf_cache_disk = hf_cache_path
        .as_deref()
        .and_then(|path| select_disk_for_hf_cache(&disks, path));

    SystemInfo {
        cpu: collect_cpu(&system),
        memory: MemoryInfo {
            total_bytes: system.total_memory(),
            available_bytes: system.available_memory(),
        },
        gpus: collect_gpus(),
        hf_cache_disk,
        collected_at: timestamp_now(),
    }
}

fn collect_cpu(system: &System) -> Option<CpuInfo> {
    let logical_cores = system.cpus().len();
    let name = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|name| !name.is_empty());

    if logical_cores == 0 && name.is_none() {
        return None;
    }

    Some(CpuInfo {
        name: name.unwrap_or_else(|| "CPU unavailable".to_string()),
        physical_cores: system.physical_core_count(),
        logical_cores,
    })
}

fn select_disk_for_hf_cache(disks: &Disks, hf_cache_path: &str) -> Option<DiskInfo> {
    disks
        .list()
        .iter()
        .filter_map(|disk| {
            let mount_point = disk.mount_point().to_string_lossy();
            score_mount_match(hf_cache_path, &mount_point).map(|score| (score, disk))
        })
        .max_by_key(|(score, _disk)| *score)
        .map(|(_score, disk)| disk_info_from_sysinfo(disk))
}

fn disk_info_from_sysinfo(disk: &Disk) -> DiskInfo {
    let name = disk.name().to_string_lossy().trim().to_string();

    DiskInfo {
        name: if name.is_empty() { None } else { Some(name) },
        mount_point: disk.mount_point().to_string_lossy().to_string(),
        total_bytes: disk.total_space(),
        available_bytes: disk.available_space(),
    }
}

#[cfg(target_os = "windows")]
fn collect_gpus() -> Vec<GpuInfo> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json -Compress",
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() || output.stdout.is_empty() {
        return Vec::new();
    }

    let json = String::from_utf8_lossy(&output.stdout);
    let json = json.trim().trim_start_matches('\u{feff}');

    serde_json::from_str::<Value>(json)
        .map(parse_gpu_json)
        .unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn collect_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn parse_gpu_json(value: Value) -> Vec<GpuInfo> {
    let mut gpus: Vec<GpuInfo> = match value {
        Value::Array(items) => items.iter().filter_map(gpu_from_json).collect(),
        Value::Object(_) => gpu_from_json(&value).into_iter().collect(),
        _ => Vec::new(),
    };

    gpus.sort_by_key(|gpu| std::cmp::Reverse(gpu_sort_key(gpu)));
    gpus
}

#[cfg(target_os = "windows")]
fn gpu_from_json(value: &Value) -> Option<GpuInfo> {
    let name = value.get("Name")?.as_str()?.trim().to_string();

    if name.is_empty() || is_placeholder_gpu(&name) {
        return None;
    }

    Some(GpuInfo {
        name,
        memory_bytes: value
            .get("AdapterRAM")
            .and_then(json_u64)
            .filter(|bytes| *bytes > 0),
    })
}

#[cfg(target_os = "windows")]
fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
}

#[cfg(target_os = "windows")]
fn is_placeholder_gpu(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("microsoft basic display")
        || normalized.contains("microsoft remote display")
}

#[cfg(target_os = "windows")]
fn gpu_sort_key(gpu: &GpuInfo) -> (u8, u64) {
    let normalized_name = gpu.name.to_ascii_lowercase();
    let priority = if normalized_name.contains("intel") {
        1
    } else {
        2
    };

    (priority, gpu.memory_bytes.unwrap_or(0))
}

fn score_mount_match(path: &str, mount_point: &str) -> Option<usize> {
    let normalized_path = normalize_path_for_compare(path);
    let normalized_mount = normalize_path_for_compare(mount_point);

    if normalized_path.is_empty() || normalized_mount.is_empty() {
        return None;
    }

    if normalized_path == normalized_mount {
        return Some(normalized_mount.len());
    }

    let mount_prefix = if normalized_mount.ends_with('\\') {
        normalized_mount.clone()
    } else {
        format!("{normalized_mount}\\")
    };

    normalized_path
        .starts_with(&mount_prefix)
        .then_some(normalized_mount.len())
}

fn normalize_path_for_compare(path: &str) -> String {
    let mut normalized = path.trim().replace('/', "\\").to_ascii_lowercase();

    while normalized.ends_with('\\') && !normalized.ends_with(":\\") && normalized.len() > 1 {
        normalized.pop();
    }

    normalized
}

fn timestamp_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::score_mount_match;

    #[test]
    fn matches_drive_root_for_nested_hf_cache_path() {
        assert_eq!(
            score_mount_match(r"C:\Users\person\.cache\huggingface\hub", r"C:\"),
            Some(3)
        );
    }

    #[test]
    fn prefers_specific_mount_without_prefix_bleed() {
        assert_eq!(score_mount_match(r"C:\models2\hub", r"C:\models"), None);
        assert_eq!(score_mount_match(r"C:\models\hub", r"C:\models"), Some(9));
    }

    #[test]
    fn does_not_match_other_drives() {
        assert_eq!(score_mount_match(r"D:\models\hub", r"C:\"), None);
    }
}
