<div align="center">

# ✦ ANIX Pulse ✦

**Нативный Rust + Tauri v2 HUD, аудит сетевых сокетов, мониторинг VRAM и Kill-Switch для ИИ-агентов, MCP-серверов и локальных LLM.**

[![CI](https://github.com/an1zy/anix-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/an1zy/anix-pulse/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-backend-orange.svg?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri v2](https://img.shields.io/badge/Tauri_v2-native_HUD-24C8D8.svg?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app)
[![Slop Score: 0%](https://img.shields.io/badge/slop_score-0%25_clean-brightgreen.svg?style=flat-square)](https://github.com/an1zy/slopcheck)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg?style=flat-square)](LICENSE)

[English](README.md) · **Русский**

<br/>

<img src="assets/hud-preview.svg" alt="ANIX Pulse HUD" width="880" />

</div>

---

## Зачем нужен ANIX Pulse?

Когда на машине одновременно крутятся Claude Code, Codex, Cursor, несколько MCP-серверов и Ollama / llama.cpp, появляются три проблемы:
1. Забытые модели и агенты втихую забивают всю VRAM и гигабайты RAM.
2. Сторонние MCP-серверы открывают левые исходящие TCP-соединения.
3. Зациклившийся агент жжёт облачные токены, когда можно было переключиться на локальную модель или стоп-кран.

**ANIX Pulse** живёт в трее без Electron-блоата и даёт мгновенный тёмно-студийный HUD-контроль над всем AI-зоопарком.

---

## Возможности

- **Живой радар агентов, MCP и LLM (`radar.rs`)**: автоматически находит процессы ИИ-агентов, MCP-серверов и локальных движков, маскирует `--api-key`/`--token` в командной строке и показывает активные внешние TCP-соединения по PID.
- **VRAM и Hardware Fit (`hardware.rs`)**: считывает загрузку/свободную память/температуру NVIDIA GPU, системную RAM, активные модели Ollama (`/api/ps`) и подсказывает, какие кванты влезают в оставшуюся VRAM.
- **Кнопка паники и Kill-Switch (`actions.rs`)**: мгновенное завершение зависшего PID или выгрузка всех моделей Ollama из VRAM в один клик (включая меню в трее).
- **Встроенный OpenAI-совместимый прокси (`proxy.rs`)**: слушает `http://127.0.0.1:8787/v1` прямо внутри Rust-бинарника с режимами `Local-First`, `Cloud Fallback` и `Kill-Switch`.

---

## Сборка из исходников

Нужен только Rust (`stable`), без Node/npm.

```bash
git clone https://github.com/an1zy/anix-pulse.git
cd anix-pulse/src-tauri
cargo test
cargo build --release
```

---

## Лицензия

MIT © 2026 [an1zy](https://github.com/an1zy)
