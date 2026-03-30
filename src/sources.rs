use crate::metrics::{MemMetrics, Metrics, PowerMetrics, TempMetrics};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub type WithError<T> = Result<T, Box<dyn Error>>;

pub fn bootstrap_runtime_assets() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("LINMON_SKIP_BOOTSTRAP").is_none() {
            let _ = bootstrap_runtime_assets_linux();
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub machine_name: String,
    pub os_version: String,
    pub cpu_name: String,
    pub cpu_vendor: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub cpu_base_freq_mhz: u32,
    pub gpu_name: String,
    pub gpu_vendor: String,
    pub gpu_backend: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct CpuStat {
    idle: u64,
    total: u64,
}

#[derive(Debug, Default)]
struct GpuSnapshot {
    usage_percent: Option<f32>,
    freq_mhz: Option<u32>,
    temp_c: Option<f32>,
    power_w: Option<f32>,
}

#[derive(Debug)]
struct RaplState {
    energy_path: PathBuf,
    max_range_uj: u64,
    prev_energy_uj: Option<u64>,
    prev_at: Option<Instant>,
}

pub struct Sampler {
    device: DeviceInfo,
    prev_cpu: Option<CpuStat>,
    rapl: Option<RaplState>,
}

impl Sampler {
    pub fn new() -> WithError<Self> {
        Ok(Self {
            device: load_device_info()?,
            prev_cpu: read_cpu_stat().ok(),
            rapl: discover_rapl(),
        })
    }

    pub fn get_metrics(&mut self) -> WithError<Metrics> {
        let cpu_now = read_cpu_stat()?;
        let cpu_usage = calc_cpu_usage(self.prev_cpu, cpu_now);
        self.prev_cpu = Some(cpu_now);

        let mem = read_mem_metrics()?;
        let gpu = read_gpu_snapshot().unwrap_or_default();
        let cpu_temp = read_cpu_temp_c();
        let gpu_temp = normalize_value(gpu.temp_c);
        let cpu_power = read_cpu_power(self.rapl.as_mut());
        let gpu_power = normalize_value(gpu.power_w);
        let sys_power = None;
        let tracked_power = match (cpu_power, gpu_power) {
            (Some(cpu), Some(gpu)) => Some(cpu + gpu),
            (Some(cpu), None) => Some(cpu),
            (None, Some(gpu)) => Some(gpu),
            (None, None) => None,
        };

        Ok(Metrics {
            temp: TempMetrics { cpu_temp, gpu_temp },
            power: PowerMetrics {
                cpu_power,
                gpu_power,
                sys_power,
                tracked_power,
            },
            memory: mem,
            cpu_usage: (read_cpu_freq_mhz(), cpu_usage),
            gpu_usage: (
                gpu.freq_mhz.unwrap_or_default(),
                normalize_ratio(gpu.usage_percent.unwrap_or_default()),
            ),
        })
    }

    pub fn get_device_info(&self) -> &DeviceInfo {
        &self.device
    }
}

pub fn load_device_info() -> WithError<DeviceInfo> {
    let cpu_info = read_cpuinfo();
    let cpu_name = cpu_info
        .get("model name")
        .cloned()
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_vendor = cpu_info
        .get("vendor_id")
        .or_else(|| cpu_info.get("Hardware"))
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());
    let cpu_threads = count_cpu_threads();
    let cpu_cores = count_cpu_cores().unwrap_or(cpu_threads);

    let (gpu_name, gpu_vendor, gpu_backend) = probe_gpu_info();

    Ok(DeviceInfo {
        machine_name: read_hostname(),
        os_version: read_os_version(),
        cpu_name,
        cpu_vendor,
        cpu_cores,
        cpu_threads,
        cpu_base_freq_mhz: read_cpu_base_freq_mhz(),
        gpu_name,
        gpu_vendor,
        gpu_backend,
    })
}

fn read_hostname() -> String {
    read_trimmed("/etc/hostname")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "linux".to_string())
}

fn read_os_version() -> String {
    let mut pretty = None;
    if let Ok(body) = fs::read_to_string("/etc/os-release") {
        for line in body.lines() {
            if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
                pretty = Some(value.trim_matches('"').to_string());
                break;
            }
        }
    }

    pretty.unwrap_or_else(|| {
        Command::new("uname")
            .arg("-sr")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Linux".to_string())
    })
}

fn read_cpuinfo() -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let Ok(body) = fs::read_to_string("/proc/cpuinfo") else {
        return map;
    };

    for line in body.lines() {
        if line.trim().is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    map
}

fn count_cpu_threads() -> u32 {
    let Ok(body) = fs::read_to_string("/proc/cpuinfo") else {
        return std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
    };

    let threads = body
        .lines()
        .filter(|line| line.starts_with("processor"))
        .count() as u32;

    threads.max(1)
}

fn count_cpu_cores() -> Option<u32> {
    let Ok(body) = fs::read_to_string("/proc/cpuinfo") else {
        return None;
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut physical_id = String::from("0");
    let mut core_id = String::new();

    for line in body.lines() {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "physical id" => physical_id = value.trim().to_string(),
                "core id" => core_id = value.trim().to_string(),
                _ => {}
            }
        } else if line.trim().is_empty() {
            if !core_id.is_empty() {
                seen.insert((physical_id.clone(), core_id.clone()));
            }
            physical_id = String::from("0");
            core_id.clear();
        }
    }

    if !core_id.is_empty() {
        seen.insert((physical_id, core_id));
    }

    if seen.is_empty() {
        None
    } else {
        Some(seen.len() as u32)
    }
}

fn read_cpu_base_freq_mhz() -> u32 {
    for path in [
        "/sys/devices/system/cpu/cpu0/cpufreq/base_frequency",
        "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq",
    ] {
        if let Some(value) = read_u64(Path::new(path)) {
            return (value / 1000) as u32;
        }
    }

    read_cpu_freq_mhz()
}

fn read_cpu_freq_mhz() -> u32 {
    let mut sum = 0u64;
    let mut count = 0u64;

    if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("cpu") || !name[3..].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let path = entry.path().join("cpufreq").join("scaling_cur_freq");
            if let Some(value) = read_u64(&path) {
                sum += value / 1000;
                count += 1;
            }
        }
    }

    if count > 0 {
        return (sum / count) as u32;
    }

    let Ok(body) = fs::read_to_string("/proc/cpuinfo") else {
        return 0;
    };
    let mut mhz_sum = 0f64;
    let mut mhz_count = 0f64;
    for line in body.lines() {
        if let Some((key, value)) = line.split_once(':')
            && key.trim() == "cpu MHz"
            && let Ok(parsed) = value.trim().parse::<f64>()
        {
            mhz_sum += parsed;
            mhz_count += 1.0;
        }
    }

    if mhz_count > 0.0 {
        (mhz_sum / mhz_count).round() as u32
    } else {
        0
    }
}

fn read_cpu_stat() -> WithError<CpuStat> {
    let body = fs::read_to_string("/proc/stat")?;
    let line = body.lines().next().ok_or("缺少 /proc/stat")?;
    let mut parts = line.split_whitespace();
    let _ = parts.next();
    let values: Vec<u64> = parts.filter_map(|x| x.parse::<u64>().ok()).collect();
    if values.len() < 4 {
        return Err("无法解析 /proc/stat".into());
    }

    let idle =
        values.get(3).copied().unwrap_or_default() + values.get(4).copied().unwrap_or_default();
    let total = values.iter().sum();
    Ok(CpuStat { idle, total })
}

fn calc_cpu_usage(prev: Option<CpuStat>, now: CpuStat) -> f32 {
    let Some(prev) = prev else {
        return 0.0;
    };

    let total = now.total.saturating_sub(prev.total);
    let idle = now.idle.saturating_sub(prev.idle);
    if total == 0 {
        0.0
    } else {
        (1.0 - idle as f32 / total as f32).clamp(0.0, 1.0)
    }
}

fn read_mem_metrics() -> WithError<MemMetrics> {
    let body = fs::read_to_string("/proc/meminfo")?;
    let mut mem_total = 0u64;
    let mut mem_available = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;

    for line in body.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let parsed = value
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or_default()
                * 1024;
            match key {
                "MemTotal" => mem_total = parsed,
                "MemAvailable" => mem_available = parsed,
                "SwapTotal" => swap_total = parsed,
                "SwapFree" => swap_free = parsed,
                _ => {}
            }
        }
    }

    Ok(MemMetrics {
        ram_total: mem_total,
        ram_usage: mem_total.saturating_sub(mem_available),
        swap_total,
        swap_usage: swap_total.saturating_sub(swap_free),
    })
}

fn read_gpu_snapshot() -> WithError<GpuSnapshot> {
    let Some(nvidia_smi) = find_nvidia_smi() else {
        return Ok(GpuSnapshot::default());
    };

    let output = Command::new(nvidia_smi)
        .args([
            "--query-gpu=utilization.gpu,clocks.current.graphics,temperature.gpu,power.draw",
            "--format=csv,noheader,nounits",
        ])
        .output()?;

    if !output.status.success() {
        return Ok(GpuSnapshot::default());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let Some(line) = body.lines().next() else {
        return Ok(GpuSnapshot::default());
    };

    let parts: Vec<_> = line.split(',').map(|x| x.trim()).collect();
    Ok(GpuSnapshot {
        usage_percent: parts.first().and_then(|x| x.parse::<f32>().ok()),
        freq_mhz: parts.get(1).and_then(|x| x.parse::<u32>().ok()),
        temp_c: parts.get(2).and_then(|x| x.parse::<f32>().ok()),
        power_w: parts.get(3).and_then(|x| x.parse::<f32>().ok()),
    })
}

fn probe_gpu_info() -> (String, String, String) {
    if let Some(nvidia_smi) = find_nvidia_smi() {
        if let Ok(output) = Command::new(&nvidia_smi)
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .output()
            && output.status.success()
        {
            let name = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "NVIDIA GPU".to_string());
            return (
                name,
                "NVIDIA".to_string(),
                nvidia_smi.to_string_lossy().into_owned(),
            );
        }
    }

    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("card") || !name[4..].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let vendor = read_trimmed(entry.path().join("device/vendor"));
            let vendor_name = match vendor.as_deref() {
                Some("0x10de") => "NVIDIA",
                Some("0x8086") => "Intel",
                Some("0x1002") | Some("0x1022") => "AMD",
                _ => "Unknown",
            };
            return (
                format!("{vendor_name} GPU"),
                vendor_name.to_string(),
                "sysfs".to_string(),
            );
        }
    }

    (
        "Unknown GPU".to_string(),
        "Unknown".to_string(),
        "none".to_string(),
    )
}

fn find_nvidia_smi() -> Option<PathBuf> {
    [
        "nvidia-smi",
        "/usr/lib/wsl/lib/nvidia-smi",
        "/mnt/c/Windows/System32/nvidia-smi.exe",
    ]
    .into_iter()
    .find_map(|candidate| {
        let path = PathBuf::from(candidate);
        if path.is_absolute() {
            path.exists().then_some(path)
        } else {
            Command::new("sh")
                .args(["-lc", &format!("command -v {candidate}")])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .map(|_| path)
        }
    })
}

fn read_cpu_temp_c() -> Option<f32> {
    let mut best: Option<(u8, f32)> = None;

    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let dir = entry.path();
            let name = read_trimmed(dir.join("name"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            for idx in 1..=10 {
                let input = dir.join(format!("temp{idx}_input"));
                if !input.exists() {
                    continue;
                }

                let Some(raw) = read_u64(&input) else {
                    continue;
                };
                let raw = raw as f32 / 1000.0;
                let label = read_trimmed(dir.join(format!("temp{idx}_label")))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let score = temp_score(&name, &label);
                if score == 0 {
                    continue;
                }

                match best {
                    Some((best_score, _)) if best_score >= score => {}
                    _ => best = Some((score, raw)),
                }
            }
        }
    }

    if best.is_none() {
        if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let dir = entry.path();
                let zone_type = read_trimmed(dir.join("type"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let score = if zone_type.contains("x86_pkg_temp") {
                    100
                } else if zone_type.contains("cpu") || zone_type.contains("package") {
                    80
                } else {
                    0
                };
                if score == 0 {
                    continue;
                }
                let Some(raw) = read_u64(&dir.join("temp")) else {
                    continue;
                };
                let raw = raw as f32 / 1000.0;
                match best {
                    Some((best_score, _)) if best_score >= score => {}
                    _ => best = Some((score, raw)),
                }
            }
        }
    }

    best.and_then(|(_, value)| normalize_value(Some(value)))
}

fn temp_score(name: &str, label: &str) -> u8 {
    if label.contains("package id 0") || label.contains("cpu package") {
        return 100;
    }
    if label.contains("tdie") {
        return 95;
    }
    if label.contains("tctl") {
        return 90;
    }
    if name.contains("coretemp") || name.contains("k10temp") {
        return 80;
    }
    if label.contains("cpu") || label.contains("core") {
        return 60;
    }
    0
}

fn discover_rapl() -> Option<RaplState> {
    let Ok(entries) = fs::read_dir("/sys/class/powercap") else {
        return None;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("intel-rapl:") || name.matches(':').count() != 1 {
            continue;
        }

        let domain = read_trimmed(entry.path().join("name")).unwrap_or_default();
        if !domain.to_ascii_lowercase().contains("package") {
            continue;
        }

        let energy_path = entry.path().join("energy_uj");
        let max_range_uj = read_u64(&entry.path().join("max_energy_range_uj")).unwrap_or(0);
        if energy_path.exists() {
            return Some(RaplState {
                energy_path,
                max_range_uj,
                prev_energy_uj: None,
                prev_at: None,
            });
        }
    }

    None
}

fn read_cpu_power(state: Option<&mut RaplState>) -> Option<f32> {
    let state = state?;
    let now = Instant::now();
    let energy_uj = read_u64(&state.energy_path)?;

    let power = match (state.prev_energy_uj, state.prev_at) {
        (Some(prev_energy), Some(prev_at)) => {
            let delta_uj = if energy_uj >= prev_energy {
                energy_uj - prev_energy
            } else if state.max_range_uj > 0 {
                state.max_range_uj.saturating_sub(prev_energy) + energy_uj
            } else {
                0
            };

            let elapsed = now.duration_since(prev_at).as_secs_f64();
            if elapsed > 0.0 && delta_uj > 0 {
                Some((delta_uj as f64 / 1_000_000.0 / elapsed) as f32)
            } else {
                None
            }
        }
        _ => None,
    };

    state.prev_energy_uj = Some(energy_uj);
    state.prev_at = Some(now);
    normalize_value(power)
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path).and_then(|s| s.parse::<u64>().ok())
}

fn normalize_value(value: Option<f32>) -> Option<f32> {
    value.filter(|x| x.is_finite() && *x > 0.0)
}

fn normalize_ratio(value: f32) -> f32 {
    (value / 100.0).clamp(0.0, 1.0)
}

#[cfg(target_os = "linux")]
fn bootstrap_runtime_assets_linux() -> WithError<()> {
    let Some(stable_dir) = linmon_bin_dir() else {
        return Ok(());
    };

    if let Ok(current_exe) = std::env::current_exe() {
        if is_system_managed_exe(&current_exe) {
            return Ok(());
        }

        fs::create_dir_all(&stable_dir)?;
        let stable_exe = stable_dir.join("linmon");
        if current_exe != stable_exe {
            copy_file_if_needed(&current_exe, &stable_exe)?;
        }
        ensure_shell_path_contains(&stable_dir)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linmon_bin_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local").join("bin"))
}

#[cfg(target_os = "linux")]
fn copy_file_if_needed(source: &Path, target: &Path) -> WithError<()> {
    let same_content = fs::read(source)
        .ok()
        .zip(fs::read(target).ok())
        .map(|(src, dst)| src == dst)
        .unwrap_or(false);

    if same_content {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_shell_path_contains(_dir: &Path) -> WithError<()> {
    let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) else {
        return Ok(());
    };

    let line = "export PATH=\"$HOME/.local/bin:$PATH\"";
    let candidates = [
        home.join(".profile"),
        home.join(".bash_profile"),
        home.join(".zprofile"),
    ];

    let mut touched = false;
    for path in &candidates {
        if path.exists() {
            ensure_profile_line(path, &line)?;
            touched = true;
        }
    }

    if !touched {
        ensure_profile_line(&home.join(".profile"), &line)?;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_profile_line(path: &Path, line: &str) -> WithError<()> {
    let mut body = fs::read_to_string(path).unwrap_or_default();
    if body.lines().any(|existing| existing.trim() == line)
        || body.contains("$HOME/.local/bin")
        || body.contains("/.local/bin")
    {
        return Ok(());
    }

    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str(line);
    body.push('\n');
    fs::write(path, body)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn is_system_managed_exe(path: &Path) -> bool {
    [
        "/usr/bin",
        "/usr/local/bin",
        "/usr/sbin",
        "/bin",
        "/sbin",
        "/snap/bin",
    ]
    .iter()
    .any(|prefix| path.starts_with(Path::new(prefix)))
}
