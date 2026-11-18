use crate::web::http::HttpResponse;

pub static INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Agent Host Steward — 监控大屏</title>
  <link rel="stylesheet" href="/app.css">
</head>
<body>
  <div id="app">
    <header class="header">
      <div class="logo">
        <span class="status-dot online"></span>
        <h1>Agent Host Steward <span>v0.3.2</span></h1>
      </div>
      <div class="header-metrics" id="header-metrics">
        <span class="metric-item">主机: <strong id="m-host">-</strong></span>
        <span class="metric-item">CPU: <strong id="m-cpu">-</strong> 核</span>
        <span class="metric-item">内存: <strong id="m-mem">-</strong> MB</span>
        <span class="metric-item">活跃锁: <strong id="m-locks" class="badge-lock">0</strong></span>
      </div>
    </header>

    <main class="main-layout">
      <section class="panel topology-panel">
        <div class="panel-header">
          <h2>🌐 宿主机事实拓扑与依赖图谱</h2>
          <div class="panel-actions">
            <button id="btn-refresh-topology" class="btn">刷新拓扑</button>
          </div>
        </div>
        <div class="topology-container" id="topology-view">
          <canvas id="topology-canvas"></canvas>
          <div class="topology-legend" id="topology-legend">
            <span class="legend-item"><span class="dot dot-process"></span> 进程</span>
            <span class="legend-item"><span class="dot dot-port"></span> 端口</span>
            <span class="legend-item"><span class="dot dot-service"></span> 服务</span>
            <span class="legend-item"><span class="dot dot-file"></span> 核心资产</span>
          </div>
        </div>
      </section>

      <aside class="sidebar-layout">
        <section class="panel locks-panel">
          <div class="panel-header">
            <h2>🔒 活跃多 Agent 协作锁</h2>
            <span id="locks-count-badge" class="badge">0</span>
          </div>
          <div class="locks-list" id="locks-list">
            <div class="empty-state">暂无活跃协作锁</div>
          </div>
        </section>

        <section class="panel events-panel">
          <div class="panel-header">
            <h2>⚡ 实时事件流 (SSE)</h2>
            <span id="sse-status" class="status-indicator">连接中...</span>
          </div>
          <div class="event-stream" id="event-stream">
            <div class="event-item info">[系统] Web 监控控制台已初始化</div>
          </div>
        </section>
      </aside>
    </main>
  </div>
  <script src="/app.js"></script>
</body>
</html>"##;

pub static APP_CSS: &str = r##"* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  background-color: #0d1117;
  color: #c9d1d9;
  line-height: 1.5;
  height: 100vh;
  overflow: hidden;
}
#app { display: flex; flex-direction: column; height: 100vh; }
.header {
  height: 56px;
  background-color: #161b22;
  border-bottom: 1px solid #30363d;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
}
.logo { display: flex; align-items: center; gap: 10px; }
.logo h1 { font-size: 1.1rem; color: #58a6ff; font-weight: 600; }
.logo h1 span { font-size: 0.75rem; color: #8b949e; margin-left: 6px; }
.status-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }
.status-dot.online { background-color: #238636; box-shadow: 0 0 8px #2ea043; }
.header-metrics { display: flex; gap: 20px; font-size: 0.85rem; }
.metric-item strong { color: #f0f6fc; }
.badge-lock { color: #f0883e !important; }

.main-layout {
  display: flex;
  flex: 1;
  padding: 16px;
  gap: 16px;
  overflow: hidden;
}
.topology-panel { flex: 2; display: flex; flex-direction: column; }
.sidebar-layout { flex: 1; display: flex; flex-direction: column; gap: 16px; min-width: 380px; }
.panel {
  background-color: #161b22;
  border: 1px solid #30363d;
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.locks-panel { flex: 1; }
.events-panel { flex: 1; }
.panel-header {
  padding: 12px 16px;
  background-color: #21262d;
  border-bottom: 1px solid #30363d;
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.panel-header h2 { font-size: 0.95rem; color: #f0f6fc; }
.btn {
  background-color: #238636;
  color: #fff;
  border: none;
  padding: 4px 10px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.8rem;
}
.btn:hover { background-color: #2ea043; }

.topology-container { flex: 1; position: relative; background: radial-gradient(#1f242c 1px, transparent 1px); background-size: 20px 20px; }
#topology-canvas { width: 100%; height: 100%; display: block; }
.topology-legend {
  position: absolute;
  bottom: 12px;
  left: 12px;
  background: rgba(22, 27, 34, 0.85);
  border: 1px solid #30363d;
  border-radius: 6px;
  padding: 6px 12px;
  display: flex;
  gap: 12px;
  font-size: 0.75rem;
}
.dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; margin-right: 4px; }
.dot-process { background-color: #58a6ff; }
.dot-port { background-color: #f0883e; }
.dot-service { background-color: #a371f7; }
.dot-file { background-color: #238636; }

.locks-list, .event-stream { padding: 12px; overflow-y: auto; flex: 1; font-size: 0.82rem; }
.empty-state { color: #8b949e; text-align: center; padding: 24px 0; }
.lock-card {
  background-color: #0d1117;
  border: 1px solid #30363d;
  border-radius: 6px;
  padding: 10px;
  margin-bottom: 8px;
}
.lock-header { display: flex; justify-content: space-between; margin-bottom: 6px; }
.lock-key { font-weight: 600; color: #58a6ff; font-family: monospace; }
.lock-mode { font-size: 0.75rem; padding: 2px 6px; border-radius: 10px; background: #388bfd33; color: #58a6ff; }
.lock-info { font-size: 0.75rem; color: #8b949e; display: flex; justify-content: space-between; }

.event-item { margin-bottom: 6px; font-family: monospace; font-size: 0.78rem; border-left: 2px solid #58a6ff; padding-left: 6px; }
.event-item.lock { border-left-color: #f0883e; }
.status-indicator { font-size: 0.75rem; color: #2ea043; }
"##;

pub static APP_JS: &str = r##"// Agent Host Steward Web Console
document.addEventListener("DOMContentLoaded", () => {
  initMetrics();
  initTopology();
  initLocks();
  initSSE();
});

async function initMetrics() {
  try {
    const res = await fetch("/api/v1/overview");
    const json = await res.json();
    if (json.code === 200 && json.data) {
      const d = json.data;
      document.getElementById("m-host").innerText = d.hostname || "-";
      document.getElementById("m-cpu").innerText = d.cpu_cores || "-";
      document.getElementById("m-mem").innerText = `${d.used_memory_mb}/${d.total_memory_mb}`;
      document.getElementById("m-locks").innerText = d.active_locks_count || "0";
    }
  } catch (e) {
    console.error("Failed to fetch overview:", e);
  }
}

async function initLocks() {
  try {
    const res = await fetch("/api/v1/locks");
    const json = await res.json();
    const list = document.getElementById("locks-list");
    const badge = document.getElementById("locks-count-badge");
    if (json.code === 200 && Array.isArray(json.data)) {
      badge.innerText = json.data.length;
      if (json.data.length === 0) {
        list.innerHTML = '<div class="empty-state">暂无活跃协作锁</div>';
        return;
      }
      list.innerHTML = json.data.map(l => `
        <div class="lock-card">
          <div class="lock-header">
            <span class="lock-key">${l.resource_type}:${l.resource_id}</span>
            <span class="lock-mode">${l.scope}</span>
          </div>
          <div class="lock-info">
            <span>持有者: ${l.holder_agent}</span>
            <span>剩余: ${l.remaining_ttl_secs}s</span>
          </div>
        </div>
      `).join("");
    }
  } catch (e) {
    console.error("Failed to fetch locks:", e);
  }
}

function initSSE() {
  const status = document.getElementById("sse-status");
  const stream = document.getElementById("event-stream");
  try {
    const es = new EventSource("/api/v1/stream");
    es.onopen = () => {
      status.innerText = "在线 (实时推流)";
      status.style.color = "#2ea043";
    };
    es.onmessage = (event) => {
      const div = document.createElement("div");
      div.className = "event-item";
      div.innerText = event.data;
      stream.prepend(div);
      initMetrics();
      initLocks();
    };
    es.onerror = () => {
      status.innerText = "重连中...";
      status.style.color = "#f0883e";
    };
  } catch (e) {
    status.innerText = "不可用";
  }
}

function initTopology() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  
  function resize() {
    canvas.width = canvas.parentElement.clientWidth;
    canvas.height = canvas.parentElement.clientHeight;
    drawMockGraph(ctx, canvas.width, canvas.height);
  }
  window.addEventListener("resize", resize);
  setTimeout(resize, 100);

  document.getElementById("btn-refresh-topology")?.addEventListener("click", () => {
    resize();
  });
}

function drawMockGraph(ctx, w, h) {
  ctx.clearRect(0, 0, w, h);
  const cx = w / 2;
  const cy = h / 2;

  // 绘制中心管家节点
  ctx.beginPath();
  ctx.arc(cx, cy, 28, 0, Math.PI * 2);
  ctx.fillStyle = "#1f6feb";
  ctx.fill();
  ctx.strokeStyle = "#58a6ff";
  ctx.lineWidth = 2;
  ctx.stroke();

  ctx.fillStyle = "#fff";
  ctx.font = "12px sans-serif";
  ctx.textAlign = "center";
  ctx.fillText("Steward", cx, cy + 4);

  // 环绕子节点
  const nodes = [
    { name: ":8080 (TCP)", color: "#f0883e", x: cx - 120, y: cy - 70 },
    { name: "python3", color: "#58a6ff", x: cx - 180, y: cy + 40 },
    { name: "steward.service", color: "#a371f7", x: cx + 130, y: cy - 60 },
    { name: "shared_proj", color: "#238636", x: cx + 150, y: cy + 60 },
  ];

  nodes.forEach(n => {
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    ctx.lineTo(n.x, n.y);
    ctx.strokeStyle = "#30363d";
    ctx.lineWidth = 1.5;
    ctx.stroke();

    ctx.beginPath();
    ctx.arc(n.x, n.y, 18, 0, Math.PI * 2);
    ctx.fillStyle = n.color;
    ctx.fill();

    ctx.fillStyle = "#c9d1d9";
    ctx.font = "11px monospace";
    ctx.fillText(n.name, n.x, n.y + 30);
  });
}
"##;

pub fn serve_index() -> HttpResponse {
    HttpResponse::html(INDEX_HTML)
}

pub fn serve_css() -> HttpResponse {
    HttpResponse::css(APP_CSS)
}

pub fn serve_js() -> HttpResponse {
    HttpResponse::js(APP_JS)
}
