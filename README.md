<div align="center">

# ✦ ANIX Pulse ✦

**Native Rust + Tauri v2 HUD, Live Network Audit, VRAM Monitor &amp; Kill-Switch for AI Coding Agents, MCP Servers, and Local LLMs.**

[![CI](https://github.com/an1zy/anix-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/an1zy/anix-pulse/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-backend-orange.svg?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri v2](https://img.shields.io/badge/Tauri_v2-native_HUD-24C8D8.svg?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app)
[![Slop Score: 0%](https://img.shields.io/badge/slop_score-0%25_clean-brightgreen.svg?style=flat-square)](https://github.com/an1zy/slopcheck)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg?style=flat-square)](LICENSE)

**English** · [Русский](README.ru.md)

<br/>

<img src="assets/hud-preview.svg" alt="ANIX Pulse HUD Preview" width="880" />

</div>

---

## Why ANIX Pulse?

Running Claude Code, Codex, Cursor, multiple MCP servers, and local Ollama / llama.cpp instances at the same time creates three blind spots:
1. Zombie MCP servers or models quietly pin 100% of your GPU VRAM or gigabytes of RAM.
2. Unreviewed MCP servers open unexpected outbound TCP connections with host environment access.
3. Runaway coding-agent loops burn cloud tokens when you wanted local-first routing.

**ANIX Pulse** stays in your system tray at minimal RAM footprint (no Electron bloat) and gives you an instant dark-studio control plane.

---

## Core Capabilities

- **Live AI Process &amp; TCP Radar (`radar.rs`)**: Auto-detects active coding agents (`Claude Code`, `Codex`, `OpenCode`, `Cursor`, `Antigravity`, `Windsurf`, `Aider`, `Cline`), local runtimes (`Ollama`, `llama.cpp`, `LM Studio`, `vLLM`), and MCP servers (`node`/`python`/`uvx`), masking `--api-key`/`--token` flags and surfacing live outbound TCP sockets per PID.
- **Hardware &amp; VRAM Fit Monitor (`hardware.rs`)**: Reads NVIDIA VRAM utilization/free headroom/temperature via `nvidia-smi`, system RAM via `sysinfo`, lists active Ollama `/api/ps` models, and shows real-time model fit indicators (`FIT` / `OOM`).
- **1-Click Panic &amp; Kill-Switch (`actions.rs`)**: Terminate runaway agent/MCP PIDs directly from the HUD, or hit **Panic: Unload VRAM** (from window or tray menu) to unload all loaded Ollama models immediately.
- **Built-In OpenAI-Compatible Routing Proxy (`proxy.rs`)**: Binds `http://127.0.0.1:8787/v1` inside Rust with `Local-First`, `Cloud Fallback`, and instant `Kill-Switch` modes plus request/latency/token telemetry.
- **Guard Rules &amp; Audit Log**: Real-time alerts when any MCP opens external TCP sockets, any process exceeds RAM caps, or VRAM hits saturation.

---

## Build from Source

Requires Rust (`stable`) and system WebView2 (pre-installed on Windows 10/11). No Node/npm build toolchain needed.

```bash
git clone https://github.com/an1zy/anix-pulse.git
cd anix-pulse/src-tauri
cargo test
cargo build --release
```

---

## License

MIT © 2026 [an1zy](https://github.com/an1zy)
