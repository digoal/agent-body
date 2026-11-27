// Agent Body 2.0 Web Console (Zero External Dependencies)
const state = {
  theme: "light",
  activeView: "topology",
  factsTab: "ports",
  govTab: "brief",
  factsData: null,
  ecoData: null,
  govData: null,
  topoRaw: null,
  topoDisplay: null,
  topoMode: "sphere",
  maxPathDepth: 3,
  hopLimit: 3,
  expandedDirs: new Set(),
  simNodes: [],
  simEdges: [],
  cam: { x: 0, y: 0, zoom: 1 },
  canvasW: 900,
  canvasH: 600,
  dpr: 1,
  rotMat: [1, 0, 0, 0, 1, 0, 0, 0, 1],
  rotVel: { vx: 0, vy: 0 },
  autoOrbit: false,
  sphereAnimId: null,
  inertiaAnimId: null,
  orbitAnimId: null,
  selectedNodeId: null,
  hoverNodeId: null,
  dragNode: null,
  dragMoved: false,
  panning: false,
  panStart: { x: 0, y: 0 },
  unreadSse: 0,
};

const NODE_COLORS_DARK = {
  process: "#38bdf8",
  port: "#fb923c",
  service: "#a78bfa",
  cron: "#f472b6",
  git: "#34d399",
  file: "#94a3b8",
  memory: "#fbbf24",
  domain_term: "#2dd4bf",
  knowhow: "#f59e0b",
  section: "#64748b",
};

const NODE_COLORS_LIGHT = {
  process: "#1F5D6E",
  port: "#C25E32",
  service: "#5C4C8E",
  cron: "#A84A6C",
  git: "#21735C",
  file: "#64767E",
  memory: "#A1741B",
  domain_term: "#1E6F68",
  knowhow: "#B45309",
  section: "#475569",
};

function esc(str) {
  if (str === null || str === undefined) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function rawJsonDrawer(data, title) {
  return `<details class="raw-json-details"><summary>${esc(title || "查看原始数据 (JSON)")}</summary><pre class="md-pre">${esc(JSON.stringify(data, null, 2))}</pre></details>`;
}

function initTheme() {
  let saved = "light";
  try {
    const v = localStorage.getItem("agent_body_ui_theme");
    if (v === "light" || v === "dark") saved = v;
  } catch (_) {}
  applyTheme(saved, false);
}

function applyTheme(theme, persist) {
  const next = theme === "dark" ? "dark" : "light";
  state.theme = next;
  document.documentElement.setAttribute("data-theme", next);
  if (persist !== false) {
    try { localStorage.setItem("agent_body_ui_theme", next); } catch (_) {}
  }
  const btn = document.getElementById("btn-theme-toggle");
  if (btn) {
    const label = btn.querySelector(".btn-label");
    if (label) {
      label.innerText = next === "light" ? "深空" : "蓝图";
      const svg = btn.querySelector("svg");
      if (svg) {
        svg.innerHTML = next === "light"
          ? '<path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/>'
          : '<circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>';
      }
    } else {
      btn.innerText = next === "light" ? "深空" : "蓝图";
    }
  }
  if (state.simNodes.length > 0) {
    drawTopology();
  }
}

function toggleTheme() {
  applyTheme(state.theme === "light" ? "dark" : "light", true);
}

function getStoredAuthToken() {
  try {
    return (localStorage.getItem("agent_body_auth_token") || "").trim();
  } catch (_) {
    return "";
  }
}

function updateAuthTokenButton() {
  const btn = document.getElementById("btn-auth-token");
  if (!btn) return;
  const has = getStoredAuthToken().length > 0;
  btn.classList.toggle("is-authed", has);
  const label = btn.querySelector(".btn-label");
  if (label) {
    label.innerText = has ? "已鉴权" : "Token";
  } else {
    btn.innerText = has ? "已鉴权" : "Token";
  }
  btn.title = has
    ? "已配置 Web Bearer Token (点击修改或留空清除)"
    : "配置 [web].auth_token 用于写操作与 GC 鉴权";
}

function promptAuthToken() {
  const current = getStoredAuthToken();
  const hint = current ? `当前已配置 (${current.slice(0, 8)}...)，输入新 Token 替换或留空清除：` : "请输入 ~/.agent-body/config.toml 中配置的 [web].auth_token：";
  const next = window.prompt(hint, "");
  if (next === null) return;
  const trimmed = next.trim();
  try {
    if (trimmed) localStorage.setItem("agent_body_auth_token", trimmed);
    else localStorage.removeItem("agent_body_auth_token");
  } catch (_) {}
  updateAuthTokenButton();
}

async function apiGet(url) {
  const headers = {};
  const tok = getStoredAuthToken();
  if (tok) headers["Authorization"] = `Bearer ${tok}`;
  const res = await fetch(url, { headers });
  const json = await res.json();
  const errMsg = json.message || json.error || "API Error";
  if (res.status === 401 || json.code === 401) {
    throw new Error(`${errMsg}（请点击右上角「🔑 Token」填入 ~/.agent-body/config.toml 中的 [web].auth_token）`);
  }
  if (res.status >= 400 || (json.code && json.code !== 200)) throw new Error(errMsg);
  return json.data;
}

async function apiPost(url, body, authToken) {
  const headers = { "Content-Type": "application/json" };
  const tok = (authToken || getStoredAuthToken() || "").trim();
  if (tok) headers["Authorization"] = `Bearer ${tok}`;
  const res = await fetch(url, { method: "POST", headers, body: JSON.stringify(body || {}) });
  const json = await res.json();
  const errMsg = json.message || json.error || "API Error";
  if (res.status === 401 || json.code === 401) {
    throw new Error(`${errMsg}（请点击右上角「🔑 Token」填入 ~/.agent-body/config.toml 中的 [web].auth_token）`);
  }
  if (res.status >= 400 || (json.code && json.code !== 200)) throw new Error(errMsg);
  return json.data;
}

function showToast(msg, isError) {
  let el = document.getElementById("global-toast-banner");
  if (!el) {
    el = document.createElement("div");
    el.id = "global-toast-banner";
    el.className = "global-toast";
    document.body.appendChild(el);
  }
  const err = Boolean(isError) || /失败|错误|Unauthorized|401|403/.test(String(msg));
  el.classList.toggle("is-error", err);
  el.innerText = String(msg);
  el.style.display = "block";
  requestAnimationFrame(() => el.classList.add("is-visible"));
  clearTimeout(window.__toastTimer);
  window.__toastTimer = setTimeout(() => {
    el.classList.remove("is-visible");
    setTimeout(() => { el.style.display = "none"; }, 220);
  }, 4500);
}

function setPerfActionBanner(msg, isError) {
  const banner = document.getElementById("body-perf-action-banner");
  if (!banner) return;
  const now = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  banner.style.display = "block";
  banner.style.background = isError ? "var(--orange-soft)" : "var(--accent-soft, rgba(47,122,102,0.18))";
  banner.style.border = isError ? "1px solid var(--orange)" : "1px solid var(--green)";
  banner.style.color = isError ? "var(--orange)" : "var(--green)";
  banner.innerText = `[${now}] ${msg}`;
}

function mirrorBlackScreen(cliCmd, mcpTool, mcpArgs) {
  const cliEl = document.getElementById("mirror-cli");
  const mcpEl = document.getElementById("mirror-mcp");
  if (cliEl) cliEl.innerText = cliCmd;
  if (mcpEl) mcpEl.innerText = JSON.stringify({ name: mcpTool, arguments: mcpArgs || {} });
}

document.addEventListener("DOMContentLoaded", () => {
  initTheme();
  bindNavigation();
  bindTopologyControls();
  bindOtherControls();
  resizeCanvas();
  initSSE();
  refreshAll();
  loadParityBadge();
});

function switchView(view) {
  state.activeView = view;
  document.querySelectorAll(".nav-btn").forEach((b) => b.classList.toggle("active", b.dataset.view === view));
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${view}`));
  if (view === "topology") resizeCanvas();
  if (view === "facts" && !state.factsData) loadFacts();
  if (view === "locks") loadLocks();
  if (view === "ecosystem" && !state.ecoData) loadEcosystem();
  if (view === "governance" && !state.govData) loadGovernance();
  if (view === "demo") loadDemoTheater();
  if (view === "benchmark") loadBenchmarkWorkbench();
  if (view === "audit") loadAuditWorkbench();
  if (view === "body") loadBodyWorkbench();
}

function bindNavigation() {
  document.querySelectorAll(".nav-btn").forEach((btn) => {
    btn.addEventListener("click", () => switchView(btn.dataset.view));
  });

  document.querySelectorAll("#facts-subtabs .subtab").forEach((btn) => {
    btn.addEventListener("click", () => {
      state.factsTab = btn.dataset.sub;
      document.querySelectorAll("#facts-subtabs .subtab").forEach((b) => b.classList.toggle("active", b === btn));
      renderFactsTab();
    });
  });

  document.querySelectorAll("#gov-subtabs .subtab").forEach((btn) => {
    btn.addEventListener("click", () => {
      state.govTab = btn.dataset.sub;
      document.querySelectorAll("#gov-subtabs .subtab").forEach((b) => b.classList.toggle("active", b === btn));
      ["brief", "memory", "analytics", "gc", "mesh"].forEach((p) => {
        document.getElementById(`gov-panel-${p}`)?.classList.toggle("hidden", p !== state.govTab);
      });
      if (state.govTab === "analytics") loadAnalytics();
      if (state.govTab === "mesh") loadParityMatrix();
    });
  });
}

function bindOtherControls() {
  updateAuthTokenButton();
  document.getElementById("btn-auth-token")?.addEventListener("click", promptAuthToken);
  document.getElementById("btn-theme-toggle")?.addEventListener("click", toggleTheme);
  document.getElementById("btn-global-refresh")?.addEventListener("click", refreshAll);
  document.getElementById("btn-toggle-sse")?.addEventListener("click", () => {
    const sb = document.getElementById("sse-sidebar");
    sb?.classList.toggle("collapsed");
    if (sb && !sb.classList.contains("collapsed")) {
      state.unreadSse = 0;
      const ub = document.getElementById("sse-unread-badge");
      if (ub) { ub.classList.add("hidden"); ub.innerText = "0"; }
    }
  });
  document.getElementById("btn-close-sse")?.addEventListener("click", () => {
    document.getElementById("sse-sidebar")?.classList.add("collapsed");
  });
  document.getElementById("btn-toggle-mirror")?.addEventListener("click", () => {
    document.getElementById("mirror-bar")?.classList.toggle("collapsed");
  });
  document.getElementById("parity-badge")?.addEventListener("click", () => {
    switchView("governance");
    document.querySelector('#gov-subtabs .subtab[data-sub="mesh"]')?.click();
  });
  document.getElementById("btn-copy-cli")?.addEventListener("click", (e) => {
    const txt = document.getElementById("mirror-cli")?.innerText || "";
    navigator.clipboard?.writeText(txt);
    const b = e.currentTarget;
    if (b) { const old = b.innerText; b.innerText = "✅"; setTimeout(() => { b.innerText = old; }, 1000); }
  });
  document.getElementById("btn-copy-mcp")?.addEventListener("click", (e) => {
    const txt = document.getElementById("mirror-mcp")?.innerText || "";
    navigator.clipboard?.writeText(txt);
    const b = e.currentTarget;
    if (b) { const old = b.innerText; b.innerText = "✅"; setTimeout(() => { b.innerText = old; }, 1000); }
  });
  document.getElementById("btn-clear-sse")?.addEventListener("click", () => {
    const el = document.getElementById("event-stream");
    if (el) el.innerHTML = "";
  });
  document.getElementById("facts-filter")?.addEventListener("input", renderFactsTab);
  document.getElementById("eco-skill-filter")?.addEventListener("input", renderEcosystemLists);
  document.getElementById("btn-run-route")?.addEventListener("click", runContextRoute);
  document.getElementById("btn-fq-run")?.addEventListener("click", runFactQuery);
  document.getElementById("btn-pc-run")?.addEventListener("click", runPortCheck);
  document.getElementById("btn-links-audit")?.addEventListener("click", runLinksAuditModal);
  document.getElementById("btn-links-mermaid")?.addEventListener("click", runLinksMermaidModal);
  document.getElementById("btn-lock-acquire")?.addEventListener("click", () => runLockAction("acquire"));
  document.getElementById("btn-lock-wait")?.addEventListener("click", () => runLockAction("wait"));
  document.getElementById("btn-hook-pre")?.addEventListener("click", () => runHookTest("pre"));
  document.getElementById("btn-hook-post")?.addEventListener("click", () => runHookTest("post"));
  document.getElementById("btn-hook-gen")?.addEventListener("click", runHookGenerate);
  document.getElementById("btn-brief-run")?.addEventListener("click", runBriefAssemble);
  document.getElementById("btn-place-suggest")?.addEventListener("click", runPlaceSuggest);
  document.getElementById("btn-place-learn")?.addEventListener("click", runPlaceLearn);
  document.getElementById("btn-mem-add")?.addEventListener("click", runMemoryAdd);
  document.getElementById("btn-mem-search")?.addEventListener("click", runMemorySearch);
  document.getElementById("btn-mem-audit")?.addEventListener("click", runMemoryAudit);
  document.getElementById("btn-ana-refresh")?.addEventListener("click", loadAnalytics);
  document.getElementById("btn-ana-export")?.addEventListener("click", runAnalyticsExport);
  document.getElementById("btn-gc-preview")?.addEventListener("click", runGcPreview);
  document.getElementById("btn-session-close")?.addEventListener("click", runSessionClosePreview);
  document.getElementById("btn-mesh-ping")?.addEventListener("click", () => runMeshQuery("ping"));
  document.getElementById("btn-mesh-port")?.addEventListener("click", () => runMeshQuery("port"));
  document.getElementById("btn-mesh-graph")?.addEventListener("click", () => runMeshQuery("graph"));
  document.getElementById("btn-mesh-dialogue-inbox")?.addEventListener("click", () => runMeshQuery("dialogue_inbox"));
  document.getElementById("btn-mesh-transfer-inbox")?.addEventListener("click", () => runMeshQuery("transfer_inbox"));
  document.getElementById("btn-demo-cruise")?.addEventListener("click", toggleDemoCruise);
  document.getElementById("btn-demo-presenter")?.addEventListener("click", () => document.body.classList.toggle("presenter-mode"));
  document.getElementById("btn-demo-reset")?.addEventListener("click", resetDemoSandbox);
  document.getElementById("btn-close-drawer")?.addEventListener("click", () => {
    document.getElementById("node-drawer")?.classList.add("hidden");
    state.selectedNodeId = null;
    drawTopology();
  });
}

async function loadParityBadge() {
  try {
    const m = await apiGet("/api/v1/parity/matrix");
    const el = document.getElementById("parity-badge");
    if (el) el.innerText = `✅ ${m.synced_count}/${m.total_mcp_tools} 对齐`;
  } catch (_) {}
}

async function refreshAll() {
  await loadOverview();
  if (state.activeView === "topology") await loadTopology();
  if (state.activeView === "facts") await loadFacts();
  if (state.activeView === "locks") await loadLocks();
  if (state.activeView === "ecosystem") await loadEcosystem();
  if (state.activeView === "governance") await loadGovernance();
}

async function loadOverview() {
  try {
    const d = await apiGet("/api/v1/overview");
    document.getElementById("k-host").innerText = d.hostname || "-";
    document.getElementById("k-hw").innerText = `${d.cpu_cores}C · ${d.used_memory_mb}/${d.total_memory_mb}MB`;
    document.getElementById("k-facts").innerText = `${d.fact_nodes_count ?? 0} 节点 / ${d.fact_edges_count ?? 0} 边`;
    document.getElementById("k-locks").innerText = `${d.active_locks_count ?? 0} 锁 / ${d.wait_queue_count ?? 0} 排队`;
    document.getElementById("k-eco").innerText = `${d.skills_count ?? 0} SKILL / ${d.mcp_servers_count ?? 0} MCP`;
    document.getElementById("k-os").innerText = d.os_platform || "linux";
  } catch (e) {
    console.error("overview err:", e);
  }
}

