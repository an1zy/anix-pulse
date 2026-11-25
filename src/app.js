const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const appWindow = getCurrentWindow();

document.getElementById("btn-min").addEventListener("click", () => appWindow.minimize());
document.getElementById("btn-max").addEventListener("click", () => appWindow.toggleMaximize());
document.getElementById("btn-close").addEventListener("click", () => appWindow.close());

function esc(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function catBadge(cat) {
  if (cat === "MCP") return '<span class="pill pill-amber">MCP</span>';
  if (cat === "LLM") return '<span class="pill pill-mono">LLM</span>';
  return '<span class="pill pill-mono">AGENT</span>';
}

function renderHardware(hw, ollamaModels) {
  document.getElementById("gpu-name").textContent = hw.gpu_name;
  document.getElementById("ram-detail").textContent = `${hw.ram_used_gb} / ${hw.ram_total_gb} GB (${hw.ram_used_pct}%)`;

  const ring = document.getElementById("vram-ring");
  const pctEl = document.getElementById("vram-pct");
  const vramEl = document.getElementById("vram-detail");
  const thermEl = document.getElementById("gpu-thermal");

  if (hw.has_nvidia && hw.vram_total_mb > 0) {
    const pct = Math.round((hw.vram_used_mb / hw.vram_total_mb) * 100);
    const circ = 314.16;
    ring.style.strokeDashoffset = (circ * (1 - pct / 100)).toFixed(2);
    ring.style.stroke = pct >= 90 ? "#f87171" : pct >= 70 ? "#fbbf24" : "#ffffff";
    pctEl.textContent = `${pct}%`;
    vramEl.textContent = `${(hw.vram_used_mb / 1024).toFixed(1)} used / ${(hw.vram_free_mb / 1024).toFixed(1)} GB free`;
    thermEl.textContent = `${hw.gpu_util_pct}% · ${hw.gpu_temp_c} C`;
  } else {
    ring.style.strokeDashoffset = "314.16";
    pctEl.textContent = "N/A";
    vramEl.textContent = "No NVIDIA GPU detected";
    thermEl.textContent = "CPU mode";
  }

  const chips = [];
  for (const m of ollamaModels) {
    chips.push(`<span class="fit-chip fit-ok">Loaded: ${esc(m.name)} (${m.size_vram_mb} MB)</span>`);
  }
  for (const f of hw.fit_suggestions.slice(0, 4)) {
    const cls = f.fits_free_vram ? "fit-ok" : "fit-no";
    const mark = f.fits_free_vram ? "FIT" : "OOM";
    chips.push(`<span class="fit-chip ${cls}">${mark} · ${esc(f.model_name)}</span>`);
  }
  document.getElementById("fit-chips").innerHTML = chips.join("");
}

function renderProxy(proxy) {
  const pill = document.getElementById("proxy-status");
  pill.textContent = proxy.running ? proxy.listen_addr : "Proxy stopped";
  pill.className = proxy.mode === "paused" ? "pill pill-red" : "pill pill-green";

  document.querySelectorAll(".mode-btn").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.mode === proxy.mode);
  });

  document.getElementById("p-total").textContent = proxy.total_requests;
  document.getElementById("p-local").textContent = proxy.local_routed;
  document.getElementById("p-cloud").textContent = proxy.cloud_failovers;
  document.getElementById("p-tokens").textContent = proxy.est_tokens_total.toLocaleString();

  const evHtml = proxy.recent_events.slice(0, 5).map(
    (ev) => `<div class="event-item"><span>${esc(ev.provider)} · ${esc(ev.model)}</span><span>${ev.latency_ms}ms · ${ev.status}</span></div>`
  );
  document.getElementById("proxy-events").innerHTML = evHtml.join("") || '<div class="event-item"><span>Listening on http://127.0.0.1:8787/v1</span><span>Idle</span></div>';
}

function renderRadar(processes) {
  document.getElementById("proc-count").textContent = `${processes.length} active`;
  const tbody = document.getElementById("radar-body");
  if (!processes.length) {
    tbody.innerHTML = '<tr><td colspan="7" style="color:#71717a;text-align:center;padding:24px;">No active AI coding agents, MCP servers, or local LLMs detected.</td></tr>';
    return;
  }

  tbody.innerHTML = processes
    .map((p) => {
      const net = p.remote_endpoints.length ? `${p.remote_endpoints.length} TCP (${esc(p.remote_endpoints[0])})` : "local";
      return `<tr>
        <td>${catBadge(p.category)}</td>
        <td><div><strong>${esc(p.tool_label)}</strong></div><div class="cmd-sub">${esc(p.cmd_masked || p.name)}</div></td>
        <td>${p.pid}</td>
        <td>${p.ram_mb.toFixed(0)} MB</td>
        <td>${p.cpu_pct.toFixed(1)}%</td>
        <td>${net}</td>
        <td><button class="btn-kill" data-pid="${p.pid}">Kill</button></td>
      </tr>`;
    })
    .join("");
}

function renderAlerts(alerts, auditLog) {
  const pill = document.getElementById("alert-count");
  pill.textContent = `${alerts.length} alerts`;
  pill.className = alerts.length ? "pill pill-red" : "pill pill-muted";

  const cards = alerts.map(
    (a) => `<div class="alert-card ${esc(a.level)}"><div class="alert-title">[${esc(a.level)}] ${esc(a.title)}</div><div class="alert-detail">${esc(a.detail)}</div></div>`
  );
  for (const log of auditLog.slice(0, 6)) {
    cards.push(`<div class="alert-card"><div class="alert-title">Audit</div><div class="alert-detail">${esc(log)}</div></div>`);
  }
  document.getElementById("alerts-list").innerHTML = cards.join("");
}

async function refreshSnapshot() {
  try {
    const [snap, audit] = await Promise.all([invoke("get_snapshot"), invoke("get_audit_log")]);
    renderHardware(snap.hardware, snap.ollama_models);
    renderProxy(snap.proxy);
    renderRadar(snap.processes);
    renderAlerts(snap.alerts, audit);
  } catch (err) {
    document.getElementById("gpu-name").textContent = String(err).slice(0, 32);
  }
}

document.getElementById("radar-body").addEventListener("click", async (e) => {
  const btn = e.target.closest(".btn-kill");
  if (!btn) return;
  const pid = Number(btn.dataset.pid);
  await invoke("kill_agent_process", { pid });
  await refreshSnapshot();
});

document.getElementById("btn-unload").addEventListener("click", async () => {
  await invoke("unload_vram");
  await refreshSnapshot();
});

document.querySelectorAll(".mode-btn").forEach((btn) => {
  btn.addEventListener("click", async () => {
    await invoke("set_routing_mode", { mode: btn.dataset.mode });
    await refreshSnapshot();
  });
});

document.getElementById("btn-save-cloud").addEventListener("click", async () => {
  const url = document.getElementById("cloud-url").value;
  const apiKey = document.getElementById("cloud-key").value;
  await invoke("set_cloud_fallback", { url, apiKey });
  document.getElementById("cloud-key").value = "";
});

document.getElementById("btn-save-guard").addEventListener("click", async () => {
  await invoke("update_guard_config", {
    config: {
      max_ram_mb_per_process: Number(document.getElementById("cfg-ram").value) || 4096,
      max_vram_used_pct: Number(document.getElementById("cfg-vram").value) || 92,
      alert_on_mcp_network: document.getElementById("cfg-mcp-net").checked,
    },
  });
  await refreshSnapshot();
});

refreshSnapshot();
setInterval(refreshSnapshot, 2000);
