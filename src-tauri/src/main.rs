#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    anix_pulse_lib::run();
}

#[cfg(test)]
mod tests {
    use anix_pulse_lib::actions::{evaluate_guards, GuardConfig};
    use anix_pulse_lib::hardware::{compute_fit_suggestions, parse_nvidia_smi_line, HardwareTelemetry};
    use anix_pulse_lib::radar::{classify_process, mask_sensitive_args, AgentCategory, TrackedProcess};

    #[test]
    fn masks_api_keys_in_cmdline() {
        let raw = "python agent.py --api-key sk-1234567890abcdef --token=ghp_abcdef123456";
        let masked = mask_sensitive_args(raw);
        assert!(!masked.contains("sk-1234567890abcdef"));
        assert!(!masked.contains("ghp_abcdef123456"));
        assert!(masked.contains("***"));
    }

    #[test]
    fn classifies_agents_mcps_and_llms() {
        assert_eq!(
            classify_process("ollama.exe", "ollama serve").unwrap().0,
            AgentCategory::Llm
        );
        assert_eq!(
            classify_process("node.exe", "node npx -y @modelcontextprotocol/server-github")
                .unwrap()
                .0,
            AgentCategory::Mcp
        );
        assert_eq!(
            classify_process("claude.exe", "claude --resume").unwrap().0,
            AgentCategory::Agent
        );
        assert!(classify_process("anix-pulse.exe", "anix-pulse").is_none());
    }

    #[test]
    fn parses_nvidia_smi_and_vram_fit() {
        let parsed =
            parse_nvidia_smi_line("NVIDIA GeForce RTX 4070, 12282, 4096, 8186, 32, 54").unwrap();
        assert_eq!(parsed.0, "NVIDIA GeForce RTX 4070");
        assert_eq!(parsed.3, 8186);

        let fits = compute_fit_suggestions(8186);
        assert!(fits.iter().any(|f| f.model_name.contains("7B") && f.fits_free_vram));
        assert!(fits.iter().any(|f| f.model_name.contains("14B") && !f.fits_free_vram));
    }

    #[test]
    fn evaluates_guard_alerts_for_ram_and_mcp_network() {
        let procs = vec![TrackedProcess {
            pid: 9999,
            name: "node.exe".to_string(),
            category: AgentCategory::Mcp,
            tool_label: "MCP: evil".to_string(),
            cpu_pct: 5.0,
            ram_mb: 5000.0,
            uptime_secs: 60,
            cmd_masked: "node mcp.js".to_string(),
            remote_endpoints: vec!["45.33.32.156:443".to_string()],
        }];
        let hw = HardwareTelemetry {
            ram_total_gb: 32.0,
            ram_used_gb: 16.0,
            ram_used_pct: 50.0,
            gpu_name: "RTX 4070".to_string(),
            vram_total_mb: 12000,
            vram_used_mb: 11500,
            vram_free_mb: 500,
            gpu_util_pct: 80,
            gpu_temp_c: 70,
            has_nvidia: true,
            fit_suggestions: vec![],
        };
        let alerts = evaluate_guards(&procs, &hw, &GuardConfig::default());
        assert_eq!(alerts.len(), 3);
    }
}
