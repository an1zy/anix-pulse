use serde::Serialize;
use std::process::Command;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, Serialize)]
pub struct FitSuggestion {
    pub model_name: String,
    pub est_vram_gb: f64,
    pub fits_free_vram: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardwareTelemetry {
    pub ram_total_gb: f64,
    pub ram_used_gb: f64,
    pub ram_used_pct: f64,
    pub gpu_name: String,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub vram_free_mb: u64,
    pub gpu_util_pct: u64,
    pub gpu_temp_c: u64,
    pub has_nvidia: bool,
    pub fit_suggestions: Vec<FitSuggestion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadedOllamaModel {
    pub name: String,
    pub size_vram_mb: u64,
}

pub fn parse_nvidia_smi_line(line: &str) -> Option<(String, u64, u64, u64, u64, u64)> {
    let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if cols.len() < 6 {
        return None;
    }
    let name = cols[0].to_string();
    let total = cols[1].parse::<u64>().ok()?;
    let used = cols[2].parse::<u64>().ok()?;
    let free = cols[3].parse::<u64>().ok()?;
    let util = cols[4].parse::<u64>().unwrap_or(0);
    let temp = cols[5].parse::<u64>().unwrap_or(0);
    Some((name, total, used, free, util, temp))
}

pub fn compute_fit_suggestions(free_vram_mb: u64) -> Vec<FitSuggestion> {
    let free_gb = free_vram_mb as f64 / 1024.0;
    let catalog = [
        ("Qwen3-4B (Q4_K_M + 16k ctx)", 3.2),
        ("Qwen2.5-Coder-7B (Q4_K_M + 16k ctx)", 5.6),
        ("Qwen3-14B (Q4_K_M + 16k ctx)", 9.8),
        ("Gemma3-27B (Q4_K_M + 8k ctx)", 17.4),
        ("Qwen3-32B (Q4_K_M + 8k ctx)", 20.8),
    ];
    catalog
        .into_iter()
        .map(|(model_name, est_vram_gb)| FitSuggestion {
            model_name: model_name.to_string(),
            est_vram_gb,
            fits_free_vram: free_gb >= est_vram_gb,
        })
        .collect()
}

pub fn collect_hardware_telemetry() -> HardwareTelemetry {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
    );
    sys.refresh_memory();

    let total_gb = (sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0;
    let used_gb = (sys.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0;
    let used_pct = if total_gb > 0.0 {
        ((used_gb / total_gb) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let mut cmd = Command::new("nvidia-smi");
    cmd.args([
        "--query-gpu=name,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu",
        "--format=csv,noheader,nounits",
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first) = stdout.lines().next() {
                if let Some((gpu_name, total, used, free, util, temp)) = parse_nvidia_smi_line(first) {
                    return HardwareTelemetry {
                        ram_total_gb: total_gb,
                        ram_used_gb: used_gb,
                        ram_used_pct: used_pct,
                        gpu_name,
                        vram_total_mb: total,
                        vram_used_mb: used,
                        vram_free_mb: free,
                        gpu_util_pct: util,
                        gpu_temp_c: temp,
                        has_nvidia: true,
                        fit_suggestions: compute_fit_suggestions(free),
                    };
                }
            }
        }
    }

    HardwareTelemetry {
        ram_total_gb: total_gb,
        ram_used_gb: used_gb,
        ram_used_pct: used_pct,
        gpu_name: "CPU / Integrated GPU".to_string(),
        vram_total_mb: 0,
        vram_used_mb: 0,
        vram_free_mb: 0,
        gpu_util_pct: 0,
        gpu_temp_c: 0,
        has_nvidia: false,
        fit_suggestions: Vec::new(),
    }
}

pub async fn list_ollama_loaded() -> Vec<LoadedOllamaModel> {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(600))
        .build()
    else {
        return Vec::new();
    };

    let Ok(resp) = client.get("http://127.0.0.1:11434/api/ps").send().await else {
        return Vec::new();
    };
    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return Vec::new();
    };

    json.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let name = item.get("name")?.as_str()?.to_string();
                    let vram_bytes = item.get("size_vram").and_then(|v| v.as_u64()).unwrap_or(0);
                    Some(LoadedOllamaModel {
                        name,
                        size_vram_mb: vram_bytes / (1024 * 1024),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn unload_all_ollama_models() -> Result<usize, String> {
    let models = list_ollama_loaded().await;
    if models.is_empty() {
        return Ok(0);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let mut unloaded = 0usize;
    for model in models {
        let payload = serde_json::json!({
            "model": model.name,
            "keep_alive": 0
        });
        if client
            .post("http://127.0.0.1:11434/api/generate")
            .json(&payload)
            .send()
            .await
            .is_ok()
        {
            unloaded += 1;
        }
    }
    Ok(unloaded)
}
