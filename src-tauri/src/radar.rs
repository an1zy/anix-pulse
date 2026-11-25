use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AgentCategory {
    Agent,
    Mcp,
    Llm,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackedProcess {
    pub pid: u32,
    pub name: String,
    pub category: AgentCategory,
    pub tool_label: String,
    pub cpu_pct: f32,
    pub ram_mb: f64,
    pub uptime_secs: u64,
    pub cmd_masked: String,
    pub remote_endpoints: Vec<String>,
}

const AGENT_MATCHES: &[(&str, &str)] = &[
    ("claude", "Claude Code"),
    ("codex", "OpenAI Codex"),
    ("opencode", "OpenCode"),
    ("cursor", "Cursor"),
    ("antigravity", "Antigravity"),
    ("windsurf", "Windsurf"),
    ("aider", "Aider"),
    ("cline", "Cline"),
    ("gemini-cli", "Gemini CLI"),
    ("anixcode", "ANIX Code"),
];

const LLM_MATCHES: &[(&str, &str)] = &[
    ("ollama", "Ollama"),
    ("llama-server", "llama.cpp server"),
    ("llama-cli", "llama.cpp CLI"),
    ("lmstudio", "LM Studio"),
    ("vllm", "vLLM"),
    ("koboldcpp", "KoboldCpp"),
];

const MCP_KEYWORDS: &[&str] = &[
    "modelcontextprotocol",
    "fastmcp",
    "playwright-mcp",
    "chrome-devtools-mcp",
    "context7",
    "mcp-server",
    "mcp_server",
    "@mcp/",
    "mcp.py",
    "mcp.js",
    "--mcp",
];

pub fn mask_sensitive_args(cmd: &str) -> String {
    let secret_flags = ["--api-key", "--token", "--secret", "--key", "-k"];
    let mut masked_tokens: Vec<String> = Vec::new();
    let mut hide_next = false;

    for raw in cmd.split_whitespace() {
        if hide_next {
            masked_tokens.push("***".to_string());
            hide_next = false;
            continue;
        }
        let lower = raw.to_ascii_lowercase();
        if secret_flags.iter().any(|f| lower == *f) {
            masked_tokens.push(raw.to_string());
            hide_next = true;
            continue;
        }
        if let Some((k, _)) = raw.split_once('=') {
            let kl = k.to_ascii_lowercase();
            if kl.contains("key") || kl.contains("token") || kl.contains("secret") {
                masked_tokens.push(format!("{k}=***"));
                continue;
            }
        }
        if raw.starts_with("sk-") || raw.starts_with("ghp_") || raw.starts_with("xox") {
            masked_tokens.push("***".to_string());
        } else {
            masked_tokens.push(raw.to_string());
        }
    }
    let joined = masked_tokens.join(" ");
    if joined.len() > 160 {
        format!("{}...", &joined[..157])
    } else {
        joined
    }
}

pub fn classify_process(name: &str, cmd_line: &str) -> Option<(AgentCategory, String)> {
    let lower_name = name.to_ascii_lowercase();
    let lower_cmd = cmd_line.to_ascii_lowercase();

    if lower_name.contains("anix-pulse") {
        return None;
    }

    for (needle, label) in LLM_MATCHES {
        if lower_name.contains(needle) || lower_cmd.contains(needle) {
            return Some((AgentCategory::Llm, (*label).to_string()));
        }
    }

    if MCP_KEYWORDS.iter().any(|k| lower_cmd.contains(k) || lower_name.contains(k))
        || lower_cmd.split_whitespace().any(|tok| tok == "mcp")
    {
        let label = format!("MCP: {}", name.trim_end_matches(".exe"));
        return Some((AgentCategory::Mcp, label));
    }

    for (needle, label) in AGENT_MATCHES {
        if lower_name.contains(needle) || lower_cmd.contains(needle) {
            return Some((AgentCategory::Agent, (*label).to_string()));
        }
    }
    None
}

pub fn collect_tcp_endpoints_by_pid() -> HashMap<u32, Vec<String>> {
    let mut map: HashMap<u32, HashSet<String>> = HashMap::new();
    let mut cmd = Command::new("netstat");
    #[cfg(target_os = "windows")]
    {
        cmd.args(["-ano", "-p", "TCP"]);
        cmd.creation_flags(0x08000000);
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd.args(["-tunap"]);
    }

    let Ok(out) = cmd.output() else {
        return HashMap::new();
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 || !parts[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        let remote = parts[2];
        let state = parts[3];
        let Ok(pid) = parts[4].parse::<u32>() else {
            continue;
        };
        if state.eq_ignore_ascii_case("ESTABLISHED")
            && !remote.starts_with("127.0.0.1:")
            && !remote.starts_with("[::1]:")
            && !remote.starts_with("0.0.0.0:")
        {
            map.entry(pid).or_default().insert(remote.to_string());
        }
    }

    map.into_iter()
        .map(|(pid, set)| {
            let mut vec: Vec<String> = set.into_iter().collect();
            vec.sort();
            (pid, vec)
        })
        .collect()
}

pub fn scan_ai_processes() -> Vec<TrackedProcess> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let tcp_map = collect_tcp_endpoints_by_pid();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let self_pid = std::process::id();
    let mut out: Vec<TrackedProcess> = Vec::new();

    for (pid, proc_) in sys.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 == self_pid || *pid == Pid::from_u32(0) {
            continue;
        }
        let name = proc_.name().to_string_lossy().to_string();
        let cmd_raw = proc_
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");

        let Some((category, tool_label)) = classify_process(&name, &cmd_raw) else {
            continue;
        };

        let start_t = proc_.start_time();
        let uptime_secs = now.saturating_sub(start_t);
        out.push(TrackedProcess {
            pid: pid_u32,
            name,
            category,
            tool_label,
            cpu_pct: (proc_.cpu_usage() * 10.0).round() / 10.0,
            ram_mb: ((proc_.memory() as f64 / (1024.0 * 1024.0)) * 10.0).round() / 10.0,
            uptime_secs,
            cmd_masked: mask_sensitive_args(&cmd_raw),
            remote_endpoints: tcp_map.get(&pid_u32).cloned().unwrap_or_default(),
        });
    }

    out.sort_by(|a, b| {
        b.ram_mb
            .partial_cmp(&a.ram_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}
