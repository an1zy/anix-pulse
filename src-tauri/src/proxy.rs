use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    LocalFirst,
    CloudOnly,
    Paused,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyEvent {
    pub id: u64,
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub est_tokens: u64,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyTelemetry {
    pub running: bool,
    pub listen_addr: String,
    pub mode: RoutingMode,
    pub total_requests: u64,
    pub local_routed: u64,
    pub cloud_failovers: u64,
    pub est_tokens_total: u64,
    pub recent_events: Vec<ProxyEvent>,
}

pub struct ProxyState {
    pub running: AtomicBool,
    pub mode: Mutex<RoutingMode>,
    pub ollama_url: Mutex<String>,
    pub cloud_fallback_url: Mutex<String>,
    pub cloud_api_key: Mutex<String>,
    pub total_requests: Mutex<u64>,
    pub local_routed: Mutex<u64>,
    pub cloud_failovers: Mutex<u64>,
    pub est_tokens_total: Mutex<u64>,
    pub recent_events: Mutex<Vec<ProxyEvent>>,
    pub client: reqwest::Client,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            mode: Mutex::new(RoutingMode::LocalFirst),
            ollama_url: Mutex::new("http://127.0.0.1:11434/v1/chat/completions".to_string()),
            cloud_fallback_url: Mutex::new("https://openrouter.ai/api/v1/chat/completions".to_string()),
            cloud_api_key: Mutex::new(String::new()),
            total_requests: Mutex::new(0),
            local_routed: Mutex::new(0),
            cloud_failovers: Mutex::new(0),
            est_tokens_total: Mutex::new(0),
            recent_events: Mutex::new(Vec::new()),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .expect("http client"),
        }
    }
}

impl ProxyState {
    pub fn snapshot(&self) -> ProxyTelemetry {
        let mode = self.mode.lock().map(|m| m.clone()).unwrap_or(RoutingMode::LocalFirst);
        let total = *self.total_requests.lock().unwrap();
        let local = *self.local_routed.lock().unwrap();
        let cloud = *self.cloud_failovers.lock().unwrap();
        let tokens = *self.est_tokens_total.lock().unwrap();
        let events = self.recent_events.lock().unwrap().clone();
        ProxyTelemetry {
            running: self.running.load(Ordering::Relaxed),
            listen_addr: "http://127.0.0.1:8787/v1".to_string(),
            mode,
            total_requests: total,
            local_routed: local,
            cloud_failovers: cloud,
            est_tokens_total: tokens,
            recent_events: events,
        }
    }

    fn record_event(&self, provider: &str, model: &str, latency_ms: u64, est_tokens: u64, status: u16, is_local: bool) {
        let mut total = self.total_requests.lock().unwrap();
        *total += 1;
        let id = *total;
        if is_local {
            *self.local_routed.lock().unwrap() += 1;
        } else {
            *self.cloud_failovers.lock().unwrap() += 1;
        }
        *self.est_tokens_total.lock().unwrap() += est_tokens;
        let mut events = self.recent_events.lock().unwrap();
        events.insert(
            0,
            ProxyEvent {
                id,
                provider: provider.to_string(),
                model: model.to_string(),
                latency_ms,
                est_tokens,
                status,
            },
        );
        events.truncate(25);
    }
}

async fn health_handler(State(state): State<Arc<ProxyState>>) -> impl IntoResponse {
    let snap = state.snapshot();
    Json(serde_json::json!({
        "status": "ok",
        "mode": snap.mode,
        "total_requests": snap.total_requests
    }))
}

async fn models_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [
            {"id": "pulse-auto", "object": "model", "owned_by": "anix-pulse"},
            {"id": "qwen2.5-coder:7b", "object": "model", "owned_by": "ollama"}
        ]
    }))
}

async fn chat_completions_handler(
    State(state): State<Arc<ProxyState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mode = state.mode.lock().unwrap().clone();
    if mode == RoutingMode::Paused {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": {"message": "ANIX Pulse proxy kill-switch active"}})),
        );
    }

    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("pulse-auto")
        .to_string();
    let t0 = Instant::now();

    if mode == RoutingMode::LocalFirst {
        let url = state.ollama_url.lock().unwrap().clone();
        if let Ok(resp) = state.client.post(&url).json(&payload).send().await {
            let status = resp.status();
            if status.is_success() {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    let est = body
                        .pointer("/usage/total_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(120);
                    state.record_event("Ollama (Local)", &model, t0.elapsed().as_millis() as u64, est, status.as_u16(), true);
                    return (StatusCode::OK, Json(body));
                }
            }
        }
    }

    let cloud_key = state.cloud_api_key.lock().unwrap().clone();
    if cloud_key.is_empty() {
        state.record_event("No Upstream", &model, t0.elapsed().as_millis() as u64, 0, 502, false);
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": {"message": "Local Ollama unavailable and no cloud fallback key configured"}})),
        );
    }

    let cloud_url = state.cloud_fallback_url.lock().unwrap().clone();
    match state
        .client
        .post(&cloud_url)
        .bearer_auth(&cloud_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            let code = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = resp.json::<serde_json::Value>().await.unwrap_or_else(|_| serde_json::json!({}));
            let est = body.pointer("/usage/total_tokens").and_then(|v| v.as_u64()).unwrap_or(180);
            state.record_event("Cloud Fallback", &model, t0.elapsed().as_millis() as u64, est, code.as_u16(), false);
            (code, Json(body))
        }
        Err(err) => {
            state.record_event("Cloud Error", &model, t0.elapsed().as_millis() as u64, 0, 502, false);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": {"message": err.to_string()}})),
            )
        }
    }
}

pub fn spawn_proxy_server(state: Arc<ProxyState>) {
    tauri::async_runtime::spawn(async move {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/v1/models", get(models_handler))
            .route("/v1/chat/completions", post(chat_completions_handler))
            .with_state(Arc::clone(&state));

        if let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:8787").await {
            state.running.store(true, Ordering::Relaxed);
            let _ = axum::serve(listener, app).await;
            state.running.store(false, Ordering::Relaxed);
        }
    });
}
