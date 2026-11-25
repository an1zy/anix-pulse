use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

use crate::hardware::HardwareTelemetry;
use crate::radar::{AgentCategory, TrackedProcess};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardConfig {
    pub max_ram_mb_per_process: f64,
    pub alert_on_mcp_network: bool,
    pub max_vram_used_pct: f64,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            max_ram_mb_per_process: 4096.0,
            alert_on_mcp_network: true,
            max_vram_used_pct: 92.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GuardAlert {
    pub level: String,
    pub pid: Option<u32>,
    pub title: String,
    pub detail: String,
}

pub fn evaluate_guards(
    processes: &[TrackedProcess],
    hardware: &HardwareTelemetry,
    config: &GuardConfig,
) -> Vec<GuardAlert> {
    let mut alerts: Vec<GuardAlert> = Vec::new();

    for p in processes {
        if p.ram_mb > config.max_ram_mb_per_process {
            alerts.push(GuardAlert {
                level: "WARN".to_string(),
                pid: Some(p.pid),
                title: format!("{} exceeded RAM cap", p.tool_label),
                detail: format!("{:.0} MB > {:.0} MB limit (PID {})", p.ram_mb, config.max_ram_mb_per_process, p.pid),
            });
        }
        if config.alert_on_mcp_network && p.category == AgentCategory::Mcp && !p.remote_endpoints.is_empty() {
            alerts.push(GuardAlert {
                level: "SECURITY".to_string(),
                pid: Some(p.pid),
                title: format!("{} external TCP activity", p.tool_label),
                detail: format!("Connected to {}", p.remote_endpoints.join(", ")),
            });
        }
    }

    if hardware.has_nvidia && hardware.vram_total_mb > 0 {
        let pct = (hardware.vram_used_mb as f64 / hardware.vram_total_mb as f64) * 100.0;
        if pct >= config.max_vram_used_pct {
            alerts.push(GuardAlert {
                level: "CRITICAL".to_string(),
                pid: None,
                title: "GPU VRAM near saturation".to_string(),
                detail: format!("{:.1}% used ({} / {} MB)", pct, hardware.vram_used_mb, hardware.vram_total_mb),
            });
        }
    }
    alerts
}

pub fn terminate_process_by_pid(pid: u32) -> Result<String, String> {
    if pid <= 4 || pid == std::process::id() {
        return Err(format!("Refusing to terminate protected PID {pid}"));
    }
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let target = Pid::from_u32(pid);
    let Some(proc_) = sys.process(target) else {
        return Err(format!("Process PID {pid} not found"));
    };
    let name = proc_.name().to_string_lossy().to_string();
    if proc_.kill() {
        Ok(format!("Terminated {name} (PID {pid})"))
    } else {
        Err(format!("Failed to kill {name} (PID {pid})"))
    }
}
