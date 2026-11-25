pub mod actions;
pub mod hardware;
pub mod proxy;
pub mod radar;

use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};

use crate::actions::{evaluate_guards, terminate_process_by_pid, GuardAlert, GuardConfig};
use crate::hardware::{
    collect_hardware_telemetry, list_ollama_loaded, unload_all_ollama_models, HardwareTelemetry,
    LoadedOllamaModel,
};
use crate::proxy::{spawn_proxy_server, ProxyState, ProxyTelemetry, RoutingMode};
use crate::radar::{scan_ai_processes, TrackedProcess};

#[derive(Debug, Clone, Serialize)]
pub struct PulseSnapshot {
    pub processes: Vec<TrackedProcess>,
    pub hardware: HardwareTelemetry,
    pub ollama_models: Vec<LoadedOllamaModel>,
    pub proxy: ProxyTelemetry,
    pub alerts: Vec<GuardAlert>,
    pub guard_config: GuardConfig,
}

pub struct AppState {
    pub proxy: Arc<ProxyState>,
    pub guard_config: Mutex<GuardConfig>,
    pub audit_log: Mutex<Vec<String>>,
}

#[tauri::command]
async fn get_snapshot(state: State<'_, AppState>) -> Result<PulseSnapshot, String> {
    let processes = tokio::task::spawn_blocking(scan_ai_processes)
        .await
        .map_err(|e| e.to_string())?;
    let hardware = tokio::task::spawn_blocking(collect_hardware_telemetry)
        .await
        .map_err(|e| e.to_string())?;
    let ollama_models = list_ollama_loaded().await;
    let proxy = state.proxy.snapshot();
    let guard_config = state.guard_config.lock().unwrap().clone();
    let alerts = evaluate_guards(&processes, &hardware, &guard_config);

    Ok(PulseSnapshot {
        processes,
        hardware,
        ollama_models,
        proxy,
        alerts,
        guard_config,
    })
}

#[tauri::command]
async fn kill_agent_process(pid: u32, state: State<'_, AppState>) -> Result<String, String> {
    let msg = tokio::task::spawn_blocking(move || terminate_process_by_pid(pid))
        .await
        .map_err(|e| e.to_string())??;
    state.audit_log.lock().unwrap().insert(0, msg.clone());
    Ok(msg)
}

#[tauri::command]
async fn unload_vram(state: State<'_, AppState>) -> Result<String, String> {
    let count = unload_all_ollama_models().await?;
    let msg = format!("Unloaded {count} Ollama model(s) from VRAM");
    state.audit_log.lock().unwrap().insert(0, msg.clone());
    Ok(msg)
}

#[tauri::command]
fn set_routing_mode(mode: RoutingMode, state: State<'_, AppState>) -> ProxyTelemetry {
    *state.proxy.mode.lock().unwrap() = mode;
    state.proxy.snapshot()
}

#[tauri::command]
fn update_guard_config(config: GuardConfig, state: State<'_, AppState>) -> GuardConfig {
    *state.guard_config.lock().unwrap() = config.clone();
    config
}

#[tauri::command]
fn set_cloud_fallback(url: String, api_key: String, state: State<'_, AppState>) {
    if !url.trim().is_empty() {
        *state.proxy.cloud_fallback_url.lock().unwrap() = url.trim().to_string();
    }
    *state.proxy.cloud_api_key.lock().unwrap() = api_key.trim().to_string();
}

#[tauri::command]
fn get_audit_log(state: State<'_, AppState>) -> Vec<String> {
    state.audit_log.lock().unwrap().clone()
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Open ANIX Pulse HUD", true, None::<&str>)?;
    let unload_item = MenuItem::with_id(app, "unload", "Panic: Unload Ollama VRAM", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &unload_item, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("ANIX Pulse — AI Agent & VRAM Monitor")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "unload" => {
                tauri::async_runtime::spawn(async {
                    let _ = unload_all_ollama_models().await;
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(win) = tray.app_handle().get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let proxy_state = Arc::new(ProxyState::default());
    let proxy_clone = Arc::clone(&proxy_state);

    tauri::Builder::default()
        .manage(AppState {
            proxy: proxy_state,
            guard_config: Mutex::new(GuardConfig::default()),
            audit_log: Mutex::new(vec!["ANIX Pulse HUD initialized".to_string()]),
        })
        .setup(move |app| {
            spawn_proxy_server(proxy_clone);
            let _ = build_tray(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            kill_agent_process,
            unload_vram,
            set_routing_mode,
            update_guard_config,
            set_cloud_fallback,
            get_audit_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running ANIX Pulse");
}
