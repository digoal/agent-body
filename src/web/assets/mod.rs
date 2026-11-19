use crate::web::http::HttpResponse;

pub static INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Agent Host Steward 2.0 — 宿主环境全景指挥舱</title>
  <link rel="stylesheet" href="/app.css">
</head>
<body>
  <div id="app">
    <header class="topbar">
      <div class="brand">
        <span class="pulse-dot"></span>
        <div>
          <h1>Agent Host Steward <span class="ver-badge">v0.3.2 · 2.0</span></h1>
          <p class="subtitle">宿主机事实图谱 · 协作锁中枢 · Context 防爆路由器</p>
        </div>
      </div>

      <div class="kpi-strip">
        <div class="kpi-pill"><span class="k-label">主机</span><strong id="k-host">-</strong></div>
        <div class="kpi-pill"><span class="k-label">CPU / 内存</span><strong id="k-hw">-</strong></div>
        <div class="kpi-pill"><span class="k-label">事实节点/边</span><strong id="k-facts" class="c-cyan">-</strong></div>
        <div class="kpi-pill"><span class="k-label">协作锁/排队</span><strong id="k-locks" class="c-orange">-</strong></div>
        <div class="kpi-pill"><span class="k-label">SKILL / MCP</span><strong id="k-eco" class="c-green">-</strong></div>
        <div class="kpi-pill"><span class="k-label">平台</span><strong id="k-os" class="c-purple">-</strong></div>
      </div>

      <div class="topbar-actions">
        <span id="sse-badge" class="sse-pill connecting">SSE 连接中</span>
        <button id="btn-toggle-sse" class="btn btn-ghost" title="展开/收起实时事件流">⚡ 事件流</button>
        <button id="btn-global-refresh" class="btn btn-primary">↻ 全量刷新</button>
      </div>
    </header>

    <div class="workspace">
      <nav class="sidenav">
        <button class="nav-btn active" data-view="topology">
          <span class="nav-ico">🕸️</span>
          <span>事实拓扑图谱</span>
        </button>
        <button class="nav-btn" data-view="facts">
          <span class="nav-ico">📡</span>
          <span>运行时事实雷达</span>
        </button>
        <button class="nav-btn" data-view="locks">
          <span class="nav-ico">🔒</span>
          <span>协作锁与死锁</span>
        </button>
        <button class="nav-btn" data-view="ecosystem">
          <span class="nav-ico">🧭</span>
          <span>生态防爆路由</span>
        </button>
        <button class="nav-btn" data-view="governance">
          <span class="nav-ico">📐</span>
          <span>规约 · 记忆 · Mesh</span>
        </button>
      </nav>

      <main class="stage">
        <!-- 视图 1: 宿主机事实拓扑图谱 -->
        <section id="view-topology" class="view active">
          <div class="view-toolbar">
            <div class="toolbar-left">
              <input id="topo-search" class="input" type="text" placeholder="搜索节点 (端口/服务/进程/工程)..." />
              <div class="filter-chips" id="topo-filters">
                <label class="chip"><input type="checkbox" value="process" checked><span class="dot d-process"></span>进程</label>
                <label class="chip"><input type="checkbox" value="port" checked><span class="dot d-port"></span>端口</label>
                <label class="chip"><input type="checkbox" value="service" checked><span class="dot d-service"></span>服务</label>
                <label class="chip"><input type="checkbox" value="cron" checked><span class="dot d-cron"></span>Cron</label>
                <label class="chip"><input type="checkbox" value="git" checked><span class="dot d-git"></span>工程</label>
                <label class="chip"><input type="checkbox" value="file" checked><span class="dot d-file"></span>文件</label>
              </div>
            </div>
            <div class="toolbar-right">
              <span id="topo-stats" class="muted-text">加载中...</span>
              <button id="btn-topo-reset" class="btn btn-ghost">恢复全景</button>
              <button id="btn-topo-fit" class="btn btn-ghost">自适应居中</button>
            </div>
          </div>
          <div class="topology-stage">
            <canvas id="topology-canvas"></canvas>
            <div class="canvas-hint">滚轮缩放 · 拖拽平移/移点 · 点击节点查看详情与爆炸半径</div>
            <aside id="node-drawer" class="drawer hidden">
              <div class="drawer-header">
                <h3 id="drawer-title">节点详情</h3>
                <button id="btn-close-drawer" class="btn-icon">✕</button>
              </div>
              <div id="drawer-body" class="drawer-body"></div>
            </aside>
          </div>
        </section>

        <!-- 视图 2: 运行时事实雷达 -->
        <section id="view-facts" class="view">
          <div class="view-toolbar">
            <div class="subtabs" id="facts-subtabs">
              <button class="subtab active" data-sub="ports">监听端口 (<span id="cnt-ports">0</span>)</button>
              <button class="subtab" data-sub="procs">活跃进程 (<span id="cnt-procs">0</span>)</button>
              <button class="subtab" data-sub="automation">Systemd &amp; Cron (<span id="cnt-auto">0</span>)</button>
              <button class="subtab" data-sub="repos">Git 工程 (<span id="cnt-repos">0</span>)</button>
              <button class="subtab" data-sub="tools">CLI 工具能力 (<span id="cnt-tools">0</span>)</button>
            </div>
            <input id="facts-filter" class="input" type="text" placeholder="过滤端口、进程名、路径或工具..." />
          </div>
          <div id="facts-content" class="scroll-panel"></div>
        </section>

        <!-- 视图 3: 多 Agent 协作锁与死锁雷达 -->
        <section id="view-locks" class="view">
          <div class="grid-2">
            <div class="card">
              <div class="card-header">
                <h3>🔒 活跃协作锁</h3>
                <div class="card-actions">
                  <input id="lock-auth-token" class="input input-sm" type="password" placeholder="可选 Bearer Token (用于解锁)" />
                </div>
              </div>
              <div id="locks-active-list" class="card-body"></div>
            </div>
            <div class="card">
              <div class="card-header">
                <h3>⏳ 等待队列 &amp; WFG 死锁检测</h3>
                <span id="deadlock-badge" class="badge badge-ok">未检测到死锁</span>
              </div>
              <div id="locks-wait-deadlock" class="card-body"></div>
            </div>
          </div>
          <div class="card mt-12">
            <div class="card-header">
              <h3>📜 锁生命周期审计历史</h3>
            </div>
            <div id="locks-history-list" class="card-body"></div>
          </div>
        </section>

        <!-- 视图 4: 生态防爆路由器 -->
        <section id="view-ecosystem" class="view">
          <div class="card router-sandbox">
            <div class="card-header">
              <h3>🧭 Context Budget Router 防爆演练沙盒</h3>
              <span id="eco-savings-badge" class="badge badge-ok">等待路由</span>
            </div>
            <div class="sandbox-controls">
              <input id="route-intent" class="input flex-1" type="text" value="PostgreSQL 数据库调优与源码分析" placeholder="输入 Agent 任务意图 (如: 美股财报分析 / postgres / svg检查)..." />
              <label class="budget-label">Token 预算:
                <input id="route-budget" class="input input-sm" type="number" value="600" min="150" max="4000" step="50" />
              </label>
              <button id="btn-run-route" class="btn btn-primary">⚡ 模拟路由</button>
            </div>
            <div id="route-result" class="route-result"></div>
          </div>

          <div class="grid-3 mt-12">
            <div class="card">
              <div class="card-header"><h3>🔌 已纳管 MCP (<span id="cnt-mcps">0</span>)</h3></div>
              <div id="eco-mcps" class="card-body scroll-sm"></div>
            </div>
            <div class="card">
              <div class="card-header">
                <h3>📚 SKILL 目录 (<span id="cnt-skills">0</span>)</h3>
                <input id="eco-skill-filter" class="input input-sm" type="text" placeholder="搜索 SKILL..." />
              </div>
              <div id="eco-skills" class="card-body scroll-sm"></div>
            </div>
            <div class="card">
              <div class="card-header"><h3>🌐 外部服务通讯录 (<span id="cnt-svcs">0</span>)</h3></div>
              <div id="eco-services" class="card-body scroll-sm"></div>
            </div>
          </div>
        </section>

        <!-- 视图 5: 规约、记忆、变更审计与 Mesh -->
        <section id="view-governance" class="view">
          <div class="grid-2">
            <div class="card">
              <div class="card-header"><h3>📐 8 大资产放置规则 &amp; 扫描策略</h3></div>
              <div id="gov-rules" class="card-body scroll-sm"></div>
            </div>
            <div class="card">
              <div class="card-header">
                <h3>🧠 跨会话长期记忆</h3>
                <input id="gov-mem-search" class="input input-sm" type="text" placeholder="FTS 搜索记忆..." />
              </div>
              <div id="gov-memories" class="card-body scroll-sm"></div>
            </div>
          </div>
          <div class="grid-2 mt-12">
            <div class="card">
              <div class="card-header"><h3>🕒 最近宿主变更审计流</h3></div>
              <div id="gov-changes" class="card-body scroll-sm"></div>
            </div>
            <div class="card">
              <div class="card-header"><h3>🌍 跨平台探针 · Preflight · Peer Mesh</h3></div>
              <div id="gov-mesh-preflight" class="card-body scroll-sm"></div>
            </div>
          </div>
        </section>
      </main>

      <!-- 右侧可折叠 SSE 实时事件流 -->
      <aside id="sse-sidebar" class="sse-sidebar">
        <div class="drawer-header">
          <h3>⚡ 实时事件流 (SSE)</h3>
          <button id="btn-clear-sse" class="btn btn-ghost btn-xs">清空</button>
        </div>
        <div id="event-stream" class="event-stream">
          <div class="event-item info">[系统] 2.0 驾驶舱已就绪</div>
        </div>
      </aside>
    </div>
  </div>
  <script src="/app.js"></script>
</body>
</html>"##;

pub static APP_CSS: &str = r##":root {
  --bg: #090d16;
  --panel: rgba(17, 24, 39, 0.88);
  --panel-2: #111827;
  --border: #1f2937;
  --border-hi: #374151;
  --text: #e5e7eb;
  --muted: #9ca3af;
  --blue: #38bdf8;
  --orange: #fb923c;
  --purple: #a78bfa;
  --pink: #f472b6;
  --green: #34d399;
  --red: #f87171;
  --yellow: #fbbf24;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
  background: radial-gradient(circle at 20% 0%, #0f172a 0%, var(--bg) 65%);
  color: var(--text);
  height: 100vh;
  overflow: hidden;
  font-size: 13px;
}
#app { display: flex; flex-direction: column; height: 100vh; }

/* Topbar */
.topbar {
  min-height: 60px;
  background: rgba(15, 23, 42, 0.92);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 18px;
  gap: 14px;
  flex-wrap: wrap;
}
.brand { display: flex; align-items: center; gap: 12px; }
.brand h1 { font-size: 15px; font-weight: 700; color: #f8fafc; letter-spacing: 0.2px; }
.ver-badge {
  font-size: 11px; font-weight: 500; color: var(--blue);
  background: rgba(56, 189, 248, 0.12); border: 1px solid rgba(56, 189, 248, 0.3);
  padding: 1px 7px; border-radius: 999px; margin-left: 6px;
}
.subtitle { font-size: 11px; color: var(--muted); }
.pulse-dot {
  width: 10px; height: 10px; border-radius: 50%; background: var(--green);
  box-shadow: 0 0 10px var(--green); flex-shrink: 0;
}

.kpi-strip { display: flex; gap: 8px; flex-wrap: wrap; }
.kpi-pill {
  background: var(--panel-2); border: 1px solid var(--border);
  border-radius: 8px; padding: 4px 10px; display: flex; flex-direction: column;
}
.k-label { font-size: 10px; color: var(--muted); }
.kpi-pill strong { font-size: 12px; color: #f9fafb; font-family: ui-monospace, monospace; }
.c-cyan { color: var(--blue) !important; }
.c-orange { color: var(--orange) !important; }
.c-green { color: var(--green) !important; }
.c-purple { color: var(--purple) !important; }

.topbar-actions { display: flex; align-items: center; gap: 8px; }
.sse-pill {
  font-size: 11px; padding: 3px 9px; border-radius: 999px;
  background: rgba(52, 211, 153, 0.12); color: var(--green); border: 1px solid rgba(52, 211, 153, 0.3);
}
.sse-pill.connecting { background: rgba(251, 146, 60, 0.12); color: var(--orange); border-color: rgba(251, 146, 60, 0.3); }

/* Buttons & Inputs */
.btn {
  border: 1px solid var(--border-hi); background: var(--panel-2); color: var(--text);
  padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 12px; transition: 0.15s;
}
.btn:hover { border-color: var(--blue); color: #fff; }
.btn-primary { background: #0284c7; border-color: #0ea5e9; color: #fff; }
.btn-primary:hover { background: #0ea5e9; }
.btn-danger { background: rgba(248, 113, 113, 0.15); border-color: var(--red); color: var(--red); }
.btn-danger:hover { background: var(--red); color: #090d16; }
.btn-ghost { background: transparent; }
.btn-xs { padding: 3px 8px; font-size: 11px; }
.btn-icon { background: none; border: none; color: var(--muted); cursor: pointer; font-size: 15px; }
.btn-icon:hover { color: #fff; }

.input {
  background: #0b1120; border: 1px solid var(--border-hi); color: var(--text);
  padding: 6px 10px; border-radius: 6px; font-size: 12px; outline: none;
}
.input:focus { border-color: var(--blue); }
.input-sm { padding: 4px 8px; font-size: 11px; width: 160px; }
.flex-1 { flex: 1; min-width: 200px; }

/* Workspace */
.workspace { display: flex; flex: 1; overflow: hidden; }
.sidenav {
  width: 184px; background: rgba(15, 23, 42, 0.72); border-right: 1px solid var(--border);
  padding: 12px 10px; display: flex; flex-direction: column; gap: 6px; flex-shrink: 0;
}
.nav-btn {
  display: flex; align-items: center; gap: 10px; padding: 10px 12px;
  border-radius: 8px; border: 1px solid transparent; background: transparent;
  color: var(--muted); cursor: pointer; text-align: left; font-size: 13px; transition: 0.15s;
}
.nav-btn:hover { background: rgba(255, 255, 255, 0.04); color: #fff; }
.nav-btn.active {
  background: rgba(56, 189, 248, 0.12); border-color: rgba(56, 189, 248, 0.35);
  color: #f8fafc; font-weight: 600;
}
.nav-ico { font-size: 15px; }

.stage { flex: 1; overflow: hidden; display: flex; flex-direction: column; padding: 14px; }
.view { display: none; flex-direction: column; height: 100%; overflow: hidden; }
.view.active { display: flex; }

.view-toolbar {
  display: flex; align-items: center; justify-content: space-between;
  gap: 10px; margin-bottom: 12px; flex-wrap: wrap;
}
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.filter-chips { display: flex; gap: 6px; flex-wrap: wrap; }
.chip {
  display: flex; align-items: center; gap: 5px; font-size: 11px;
  background: var(--panel); border: 1px solid var(--border);
  padding: 4px 8px; border-radius: 999px; cursor: pointer;
}
.dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.d-process { background: var(--blue); }
.d-port { background: var(--orange); }
.d-service { background: var(--purple); }
.d-cron { background: var(--pink); }
.d-git { background: var(--green); }
.d-file { background: #94a3b8; }

/* Topology */
.topology-stage {
  flex: 1; position: relative; border: 1px solid var(--border); border-radius: 10px;
  background: radial-gradient(#1e293b 1px, transparent 1px), #070b14;
  background-size: 22px 22px; overflow: hidden;
}
#topology-canvas { width: 100%; height: 100%; display: block; cursor: grab; }
#topology-canvas:active { cursor: grabbing; }
.canvas-hint {
  position: absolute; bottom: 10px; left: 12px; font-size: 11px; color: var(--muted);
  background: rgba(9, 13, 22, 0.75); padding: 4px 10px; border-radius: 6px; border: 1px solid var(--border);
}

.drawer {
  position: absolute; top: 10px; right: 10px; bottom: 10px; width: 340px;
  background: rgba(15, 23, 42, 0.96); border: 1px solid var(--border-hi);
  border-radius: 10px; display: flex; flex-direction: column;
  box-shadow: 0 12px 30px rgba(0,0,0,0.5); z-index: 10;
}
.drawer.hidden { display: none; }
.drawer-header {
  padding: 10px 14px; border-bottom: 1px solid var(--border);
  display: flex; align-items: center; justify-content: space-between;
}
.drawer-header h3 { font-size: 13px; color: #f8fafc; }
.drawer-body { padding: 12px; overflow-y: auto; flex: 1; font-size: 12px; }

/* Cards & Grids */
.scroll-panel { flex: 1; overflow-y: auto; padding-right: 4px; }
.grid-2 { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.grid-3 { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
.mt-12 { margin-top: 12px; }
.card {
  background: var(--panel); border: 1px solid var(--border); border-radius: 10px;
  display: flex; flex-direction: column; overflow: hidden;
}
.card-header {
  padding: 10px 14px; background: rgba(30, 41, 59, 0.55); border-bottom: 1px solid var(--border);
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
}
.card-header h3 { font-size: 13px; font-weight: 600; color: #f1f5f9; }
.card-body { padding: 12px; overflow-y: auto; max-height: 360px; }
.scroll-sm { max-height: 320px; overflow-y: auto; }

/* Subtabs & Tables */
.subtabs { display: flex; gap: 6px; flex-wrap: wrap; }
.subtab {
  background: var(--panel); border: 1px solid var(--border); color: var(--muted);
  padding: 6px 11px; border-radius: 6px; cursor: pointer; font-size: 12px;
}
.subtab.active { background: rgba(56, 189, 248, 0.15); border-color: var(--blue); color: #fff; }
.data-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.data-table th, .data-table td {
  text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--border);
  vertical-align: top; word-break: break-word;
}
.data-table th { color: var(--muted); font-weight: 600; background: rgba(15, 23, 42, 0.65); position: sticky; top: 0; }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; }

/* Badges & Items */
.badge {
  font-size: 11px; padding: 2px 8px; border-radius: 999px; font-weight: 600;
  background: rgba(56, 189, 248, 0.14); color: var(--blue);
}
.badge-ok { background: rgba(52, 211, 153, 0.14); color: var(--green); }
.badge-warn { background: rgba(251, 146, 60, 0.16); color: var(--orange); }
.badge-err { background: rgba(248, 113, 113, 0.18); color: var(--red); }
.muted-text { color: var(--muted); font-size: 12px; }
.empty { color: var(--muted); text-align: center; padding: 20px; }

.item-box {
  background: rgba(9, 13, 22, 0.72); border: 1px solid var(--border);
  border-radius: 8px; padding: 10px; margin-bottom: 8px;
}
.item-top { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-bottom: 4px; }
.ttl-bar { height: 4px; background: #1e293b; border-radius: 4px; overflow: hidden; margin-top: 6px; }
.ttl-fill { height: 100%; background: linear-gradient(90deg, var(--green), var(--blue)); }

/* Router Sandbox */
.sandbox-controls { display: flex; gap: 10px; padding: 12px; align-items: center; flex-wrap: wrap; }
.budget-label { font-size: 12px; color: var(--muted); display: flex; align-items: center; gap: 6px; }
.route-result { padding: 0 12px 12px; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }

/* SSE Sidebar */
.sse-sidebar {
  width: 280px; background: rgba(15, 23, 42, 0.85); border-left: 1px solid var(--border);
  display: flex; flex-direction: column; flex-shrink: 0;
}
.sse-sidebar.collapsed { display: none; }
.event-stream { flex: 1; overflow-y: auto; padding: 10px; font-family: ui-monospace, monospace; font-size: 11px; }
.event-item {
  border-left: 2px solid var(--blue); padding: 4px 8px; margin-bottom: 6px;
  background: rgba(9, 13, 22, 0.55); border-radius: 0 4px 4px 0; word-break: break-all;
}
.event-item.warn { border-left-color: var(--orange); }
.event-item.ok { border-left-color: var(--green); }

@media (max-width: 1100px) {
  .grid-2, .grid-3, .route-result { grid-template-columns: 1fr; }
  .sse-sidebar { display: none; }
  .sidenav { width: 62px; padding: 8px 6px; }
  .nav-btn span:last-child { display: none; }
}
"##;

pub static APP_JS: &str = r##"// Agent Host Steward 2.0 Web Console (Zero External Dependencies)
const state = {
  activeView: "topology",
  factsTab: "ports",
  factsData: null,
  ecoData: null,
  govData: null,
  topoRaw: null,
  topoDisplay: null,
  simNodes: [],
  simEdges: [],
  cam: { x: 0, y: 0, zoom: 1 },
  selectedNodeId: null,
  hoverNodeId: null,
  dragNode: null,
  panning: false,
  panStart: { x: 0, y: 0 },
};

const NODE_COLORS = {
  process: "#38bdf8",
  port: "#fb923c",
  service: "#a78bfa",
  cron: "#f472b6",
  git: "#34d399",
  file: "#94a3b8",
  memory: "#fbbf24",
};

function esc(str) {
  if (str === null || str === undefined) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function apiGet(url) {
  const res = await fetch(url);
  const json = await res.json();
  if (json.code !== 200) throw new Error(json.message || "API Error");
  return json.data;
}

document.addEventListener("DOMContentLoaded", () => {
  bindNavigation();
  bindTopologyControls();
  bindOtherControls();
  initSSE();
  refreshAll();
});

function bindNavigation() {
  document.querySelectorAll(".nav-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const view = btn.dataset.view;
      state.activeView = view;
      document.querySelectorAll(".nav-btn").forEach((b) => b.classList.toggle("active", b === btn));
      document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${view}`));
      if (view === "topology") resizeCanvas();
      if (view === "facts" && !state.factsData) loadFacts();
      if (view === "locks") loadLocks();
      if (view === "ecosystem" && !state.ecoData) loadEcosystem();
      if (view === "governance" && !state.govData) loadGovernance();
    });
  });

  document.querySelectorAll("#facts-subtabs .subtab").forEach((btn) => {
    btn.addEventListener("click", () => {
      state.factsTab = btn.dataset.sub;
      document.querySelectorAll("#facts-subtabs .subtab").forEach((b) => b.classList.toggle("active", b === btn));
      renderFactsTab();
    });
  });
}

function bindOtherControls() {
  document.getElementById("btn-global-refresh")?.addEventListener("click", refreshAll);
  document.getElementById("btn-toggle-sse")?.addEventListener("click", () => {
    document.getElementById("sse-sidebar")?.classList.toggle("collapsed");
    if (state.activeView === "topology") setTimeout(resizeCanvas, 50);
  });
  document.getElementById("btn-clear-sse")?.addEventListener("click", () => {
    const el = document.getElementById("event-stream");
    if (el) el.innerHTML = "";
  });
  document.getElementById("facts-filter")?.addEventListener("input", renderFactsTab);
  document.getElementById("eco-skill-filter")?.addEventListener("input", renderEcosystemLists);
  document.getElementById("btn-run-route")?.addEventListener("click", runContextRoute);
  document.getElementById("gov-mem-search")?.addEventListener("change", () => loadGovernance());
  document.getElementById("btn-close-drawer")?.addEventListener("click", () => {
    document.getElementById("node-drawer")?.classList.add("hidden");
    state.selectedNodeId = null;
    drawTopology();
  });
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

/* ─── 1. 宿主机事实拓扑交互图谱 ─────────────────────────────────── */
function bindTopologyControls() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas) return;

  window.addEventListener("resize", () => {
    if (state.activeView === "topology") resizeCanvas();
  });

  document.getElementById("topo-search")?.addEventListener("input", drawTopology);
  document.querySelectorAll("#topo-filters input").forEach((cb) => cb.addEventListener("change", applyTopoFilter));
  document.getElementById("btn-topo-reset")?.addEventListener("click", () => {
    if (state.topoRaw) {
      state.topoDisplay = state.topoRaw;
      applyTopoFilter();
    } else {
      loadTopology();
    }
  });
  document.getElementById("btn-topo-fit")?.addEventListener("click", fitCamera);

  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.12 : 0.89;
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;
    const wx = (mx - state.cam.x) / state.cam.zoom;
    const wy = (my - state.cam.y) / state.cam.zoom;
    state.cam.zoom = Math.max(0.18, Math.min(3.5, state.cam.zoom * factor));
    state.cam.x = mx - wx * state.cam.zoom;
    state.cam.y = my - wy * state.cam.zoom;
    drawTopology();
  }, { passive: false });

  canvas.addEventListener("mousedown", (e) => {
    const pos = canvasToWorld(e, canvas);
    const hit = findNodeAt(pos.x, pos.y);
    if (hit) {
      state.dragNode = hit;
      state.selectedNodeId = hit.id;
      showNodeDrawer(hit);
      drawTopology();
    } else {
      state.panning = true;
      state.panStart = { x: e.clientX - state.cam.x, y: e.clientY - state.cam.y };
    }
  });

  window.addEventListener("mousemove", (e) => {
    if (state.dragNode) {
      const pos = canvasToWorld(e, canvas);
      state.dragNode.x = pos.x;
      state.dragNode.y = pos.y;
      drawTopology();
    } else if (state.panning) {
      state.cam.x = e.clientX - state.panStart.x;
      state.cam.y = e.clientY - state.panStart.y;
      drawTopology();
    } else if (state.activeView === "topology") {
      const pos = canvasToWorld(e, canvas);
      const hit = findNodeAt(pos.x, pos.y);
      const nextHover = hit ? hit.id : null;
      if (nextHover !== state.hoverNodeId) {
        state.hoverNodeId = nextHover;
        canvas.style.cursor = hit ? "pointer" : "grab";
        drawTopology();
      }
    }
  });

  window.addEventListener("mouseup", () => {
    state.dragNode = null;
    state.panning = false;
  });
}

function canvasToWorld(e, canvas) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: (e.clientX - rect.left - state.cam.x) / state.cam.zoom,
    y: (e.clientY - rect.top - state.cam.y) / state.cam.zoom,
  };
}

function findNodeAt(wx, wy) {
  for (let i = state.simNodes.length - 1; i >= 0; i--) {
    const n = state.simNodes[i];
    const r = n.radius || 12;
    const dx = wx - n.x;
    const dy = wy - n.y;
    if (dx * dx + dy * dy <= (r + 4) * (r + 4)) return n;
  }
  return null;
}

function resizeCanvas() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas || !canvas.parentElement) return;
  canvas.width = canvas.parentElement.clientWidth || 800;
  canvas.height = canvas.parentElement.clientHeight || 500;
  if (!state.topoRaw) {
    loadTopology();
  } else {
    drawTopology();
  }
}

async function loadTopology() {
  try {
    const graph = await apiGet("/api/v1/topology");
    state.topoRaw = graph;
    state.topoDisplay = graph;
    applyTopoFilter();
  } catch (e) {
    console.error("topology err:", e);
  }
}

function applyTopoFilter() {
  if (!state.topoDisplay) return;
  const enabled = new Set(
    Array.from(document.querySelectorAll("#topo-filters input:checked")).map((c) => c.value)
  );
  const rawNodes = (state.topoDisplay.nodes || []).filter((n) => {
    const nt = typeof n.node_type === "string" ? n.node_type : "file";
    return enabled.has(nt) || (nt !== "process" && nt !== "port" && nt !== "service" && nt !== "cron" && nt !== "git");
  });
  const idSet = new Set(rawNodes.map((n) => n.id));
  const rawEdges = (state.topoDisplay.edges || []).filter((e) => idSet.has(e.source) && idSet.has(e.target));

  document.getElementById("topo-stats").innerText =
    `显示 ${rawNodes.length} 节点 · ${rawEdges.length} 条真实连边 (全图 ${state.topoDisplay.summary?.total_nodes ?? 0} 节点)`;

  layoutGraph(rawNodes, rawEdges);
  fitCamera();
  drawTopology();
}

function layoutGraph(rawNodes, rawEdges) {
  const deg = new Map();
  rawEdges.forEach((e) => {
    deg.set(e.source, (deg.get(e.source) || 0) + 1);
    deg.set(e.target, (deg.get(e.target) || 0) + 1);
  });

  const nCount = rawNodes.length || 1;
  const simNodes = rawNodes.map((n, idx) => {
    const d = deg.get(n.id) || 0;
    const angle = (idx / nCount) * Math.PI * 2;
    const ring = d > 0 ? 180 + (idx % 5) * 45 : 380 + (idx % 4) * 55;
    const nt = typeof n.node_type === "string" ? n.node_type : "file";
    return {
      ...n,
      nt,
      degree: d,
      radius: Math.min(20, 9 + Math.sqrt(d) * 2.5),
      x: Math.cos(angle) * ring,
      y: Math.sin(angle) * ring,
      vx: 0,
      vy: 0,
    };
  });

  const nodeMap = new Map(simNodes.map((n) => [n.id, n]));
  const simEdges = rawEdges
    .map((e) => ({ ...e, sNode: nodeMap.get(e.source), tNode: nodeMap.get(e.target) }))
    .filter((e) => e.sNode && e.tNode);

  // 轻量级力导向迭代（仅对前 180 节点做全对斥力，控制耗时 <15ms）
  const iters = simNodes.length > 160 ? 35 : 65;
  for (let step = 0; step < iters; step++) {
    for (let i = 0; i < simNodes.length; i++) {
      const a = simNodes[i];
      a.vx *= 0.6;
      a.vy *= 0.6;
      // 向心引力
      a.vx -= a.x * 0.004;
      a.vy -= a.y * 0.004;
      const limitJ = Math.min(simNodes.length, i + 45);
      for (let j = i + 1; j < limitJ; j++) {
        const b = simNodes[j];
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dist2 = dx * dx + dy * dy + 1;
        if (dist2 < 18000) {
          const f = 900 / dist2;
          a.vx += dx * f;
          a.vy += dy * f;
          b.vx -= dx * f;
          b.vy -= dy * f;
        }
      }
    }
    for (const e of simEdges) {
      const dx = e.tNode.x - e.sNode.x;
      const dy = e.tNode.y - e.sNode.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const pull = (dist - 110) * 0.04;
      const fx = (dx / dist) * pull;
      const fy = (dy / dist) * pull;
      e.sNode.vx += fx;
      e.sNode.vy += fy;
      e.tNode.vx -= fx;
      e.tNode.vy -= fy;
    }
    for (const n of simNodes) {
      n.x += Math.max(-25, Math.min(25, n.vx));
      n.y += Math.max(-25, Math.min(25, n.vy));
    }
  }

  state.simNodes = simNodes;
  state.simEdges = simEdges;
}

function fitCamera() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas || state.simNodes.length === 0) return;
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  state.simNodes.forEach((n) => {
    if (n.x < minX) minX = n.x;
    if (n.x > maxX) maxX = n.x;
    if (n.y < minY) minY = n.y;
    if (n.y > maxY) maxY = n.y;
  });
  const spanX = Math.max(200, maxX - minX + 120);
  const spanY = Math.max(200, maxY - minY + 120);
  const zoom = Math.min(1.2, Math.min(canvas.width / spanX, canvas.height / spanY));
  state.cam.zoom = zoom;
  state.cam.x = canvas.width / 2 - ((minX + maxX) / 2) * zoom;
  state.cam.y = canvas.height / 2 - ((minY + maxY) / 2) * zoom;
  drawTopology();
}

function drawTopology() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const w = canvas.width, h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const kw = (document.getElementById("topo-search")?.value || "").trim().toLowerCase();
  const focusId = state.hoverNodeId || state.selectedNodeId;

  ctx.save();
  ctx.translate(state.cam.x, state.cam.y);
  ctx.scale(state.cam.zoom, state.cam.zoom);

  // 连边
  for (const e of state.simEdges) {
    const isFocus = focusId && (e.source === focusId || e.target === focusId);
    ctx.beginPath();
    ctx.moveTo(e.sNode.x, e.sNode.y);
    ctx.lineTo(e.tNode.x, e.tNode.y);
    ctx.strokeStyle = isFocus ? "#38bdf8" : "rgba(148, 163, 184, 0.28)";
    ctx.lineWidth = isFocus ? 2.2 : 1.1;
    ctx.stroke();

    if (isFocus || state.cam.zoom > 0.85) {
      const mx = (e.sNode.x + e.tNode.x) / 2;
      const my = (e.sNode.y + e.tNode.y) / 2;
      ctx.fillStyle = isFocus ? "#7dd3fc" : "rgba(148, 163, 184, 0.65)";
      ctx.font = "9px monospace";
      ctx.textAlign = "center";
      ctx.fillText(e.relation, mx, my - 3);
    }
  }

  // 节点
  for (const n of state.simNodes) {
    const matched = !kw || n.label.toLowerCase().includes(kw) || n.id.toLowerCase().includes(kw);
    const isSelected = n.id === state.selectedNodeId || n.id === state.hoverNodeId;
    const color = NODE_COLORS[n.nt] || "#fbbf24";

    ctx.globalAlpha = matched ? 1 : 0.22;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();

    if (isSelected || (kw && matched)) {
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 2.5;
      ctx.stroke();
    }

    if (state.cam.zoom > 0.45 || n.degree > 0 || isSelected || (kw && matched)) {
      ctx.fillStyle = "#e2e8f0";
      ctx.font = isSelected ? "bold 11px sans-serif" : "10px monospace";
      ctx.textAlign = "center";
      ctx.fillText(n.label.slice(0, 26), n.x, n.y + n.radius + 12);
    }
  }
  ctx.globalAlpha = 1;
  ctx.restore();
}

function showNodeDrawer(node) {
  const drawer = document.getElementById("node-drawer");
  const title = document.getElementById("drawer-title");
  const body = document.getElementById("drawer-body");
  if (!drawer || !body) return;
  drawer.classList.remove("hidden");
  title.innerText = `${node.label}`;
  body.innerHTML = `
    <div class="item-box">
      <div><strong>ID:</strong> <span class="mono">${esc(node.id)}</span></div>
      <div><strong>类型:</strong> <span class="badge">${esc(node.nt)}</span></div>
      <div><strong>状态:</strong> ${esc(node.status)}</div>
      <div><strong>关联边数:</strong> ${node.degree}</div>
    </div>
    <button id="btn-impact-node" class="btn btn-primary" style="width:100%;margin-bottom:10px;">
      💥 推演 3 跳爆炸半径子图
    </button>
    <div class="item-box">
      <div style="margin-bottom:6px;"><strong>物理元数据:</strong></div>
      <pre class="mono" style="white-space:pre-wrap;color:#93c5fd;">${esc(JSON.stringify(node.metadata, null, 2))}</pre>
    </div>
  `;
  document.getElementById("btn-impact-node")?.addEventListener("click", async () => {
    try {
      const impact = await apiGet(`/api/v1/topology/impact?target=${encodeURIComponent(node.id)}`);
      state.topoDisplay = impact;
      applyTopoFilter();
    } catch (e) {
      alert("爆炸半径推演失败: " + e.message);
    }
  });
}

/* ─── 2. 运行时事实雷达 ──────────────────────────────────────────── */
async function loadFacts() {
  try {
    state.factsData = await apiGet("/api/v1/facts");
    document.getElementById("cnt-ports").innerText = state.factsData.listening_ports?.length || 0;
    document.getElementById("cnt-procs").innerText = state.factsData.active_processes?.length || 0;
    document.getElementById("cnt-auto").innerText =
      (state.factsData.systemd_services?.length || 0) + (state.factsData.cron_jobs?.length || 0);
    document.getElementById("cnt-repos").innerText = state.factsData.repos?.length || 0;
    document.getElementById("cnt-tools").innerText = state.factsData.capabilities?.length || 0;
    renderFactsTab();
  } catch (e) {
    console.error("facts err:", e);
  }
}

function renderFactsTab() {
  const el = document.getElementById("facts-content");
  if (!el || !state.factsData) return;
  const q = (document.getElementById("facts-filter")?.value || "").trim().toLowerCase();
  const d = state.factsData;

  if (state.factsTab === "ports") {
    const rows = (d.listening_ports || []).filter((p) =>
      !q || String(p.port).includes(q) || (p.process_name || "").toLowerCase().includes(q) || (p.command || "").toLowerCase().includes(q)
    );
    el.innerHTML = `<table class="data-table"><thead><tr><th>端口</th><th>绑定</th><th>进程 / PID</th><th>用户</th><th>工作目录</th><th>命令</th></tr></thead><tbody>` +
      rows.map((p) => `<tr>
        <td><span class="badge badge-warn">:${p.port}/${esc(p.protocol)}</span></td>
        <td class="mono">${esc(p.bind_ip)}</td>
        <td><strong>${esc(p.process_name || "-")}</strong> <span class="muted-text">(${p.pid ?? "-"})</span></td>
        <td>${esc(p.user || "-")}</td>
        <td class="mono">${esc(p.cwd || "-")}</td>
        <td class="mono">${esc((p.command || "").slice(0, 90))}</td>
      </tr>`).join("") + `</tbody></table>`;
  } else if (state.factsTab === "procs") {
    const rows = (d.active_processes || []).filter((p) =>
      !q || p.name.toLowerCase().includes(q) || p.command.toLowerCase().includes(q)
    );
    el.innerHTML = `<table class="data-table"><thead><tr><th>PID</th><th>进程名</th><th>监听端口</th><th>归属</th><th>CPU / 内存</th><th>命令行</th></tr></thead><tbody>` +
      rows.map((p) => `<tr>
        <td class="mono">${p.pid}</td>
        <td><strong>${esc(p.name)}</strong></td>
        <td>${(p.listening_ports || []).map((pt) => `<span class="badge badge-warn">:${pt}</span>`).join(" ") || "-"}</td>
        <td>${esc(p.owner)}</td>
        <td class="mono">${Number(p.cpu_pct || 0).toFixed(1)}% · ${Math.round((p.mem_bytes || 0) / 1048576)}MB</td>
        <td class="mono">${esc(p.command.slice(0, 110))}</td>
      </tr>`).join("") + `</tbody></table>`;
  } else if (state.factsTab === "automation") {
    const svcs = (d.systemd_services || []).filter((s) => !q || s.service_name.toLowerCase().includes(q));
    const crons = (d.cron_jobs || []).filter((c) => !q || c.command.toLowerCase().includes(q));
    el.innerHTML = `<h4 style="margin-bottom:8px;">⚙️ Crontab 定时任务 (${crons.length})</h4>` +
      `<table class="data-table"><thead><tr><th>用户</th><th>表达式</th><th>目标脚本</th><th>命令</th></tr></thead><tbody>` +
      crons.map((c) => `<tr><td>${esc(c.user)}</td><td class="mono">${esc(c.schedule)}</td><td class="mono">${esc(c.target_script || "-")}</td><td class="mono">${esc(c.command)}</td></tr>`).join("") +
      `</tbody></table><h4 style="margin:14px 0 8px;">🧩 Systemd 服务 (${svcs.length})</h4>` +
      `<table class="data-table"><thead><tr><th>服务名</th><th>工作目录</th><th>ExecStart</th></tr></thead><tbody>` +
      svcs.map((s) => `<tr><td><strong>${esc(s.service_name)}</strong></td><td class="mono">${esc(s.working_directory || "-")}</td><td class="mono">${esc(s.exec_start || "-")}</td></tr>`).join("") +
      `</tbody></table>`;
  } else if (state.factsTab === "repos") {
    const rows = (d.repos || []).filter((r) => !q || r.name.toLowerCase().includes(q) || r.abs_path.toLowerCase().includes(q));
    el.innerHTML = `<table class="data-table"><thead><tr><th>工程名</th><th>分支</th><th>语言</th><th>CodeGraph</th><th>路径</th></tr></thead><tbody>` +
      rows.map((r) => `<tr>
        <td><strong>${esc(r.name)}</strong></td>
        <td class="mono">${esc(r.current_branch || "-")}</td>
        <td>${esc(r.primary_lang || "-")}</td>
        <td>${r.codegraph_ready ? '<span class="badge badge-ok">Ready</span>' : '<span class="badge">Unindexed</span>'}</td>
        <td class="mono">${esc(r.abs_path)}</td>
      </tr>`).join("") + `</tbody></table>`;
  } else {
    const rows = (d.capabilities || []).filter((c) =>
      !q || c.tool_name.toLowerCase().includes(q) || c.description.toLowerCase().includes(q) || c.category.toLowerCase().includes(q)
    );
    el.innerHTML = `<table class="data-table"><thead><tr><th>工具</th><th>分类</th><th>状态</th><th>说明</th><th>推荐调用参数</th></tr></thead><tbody>` +
      rows.map((c) => `<tr>
        <td class="mono"><strong>${esc(c.tool_name)}</strong></td>
        <td><span class="badge">${esc(c.category)}</span></td>
        <td>${c.installed ? '<span class="badge badge-ok">已安装</span>' : '<span class="badge badge-err">未安装</span>'}</td>
        <td>${esc(c.description)}</td>
        <td class="mono">${esc(c.best_practice_args || "-")}</td>
      </tr>`).join("") + `</tbody></table>`;
  }
}

/* ─── 3. 多 Agent 协作锁与死锁雷达 ──────────────────────────────── */
async function loadLocks() {
  try {
    const [locks, waits, deadlocks, history] = await Promise.all([
      apiGet("/api/v1/locks"),
      apiGet("/api/v1/locks/wait-queue"),
      apiGet("/api/v1/locks/deadlocks"),
      apiGet("/api/v1/locks/history"),
    ]);

    const activeEl = document.getElementById("locks-active-list");
    if (locks.length === 0) {
      activeEl.innerHTML = '<div class="empty">当前无活跃协作锁</div>';
    } else {
      activeEl.innerHTML = locks.map((l) => {
        const pct = Math.max(5, Math.min(100, Math.round((l.remaining_ttl_secs / 300) * 100)));
        return `<div class="item-box">
          <div class="item-top">
            <strong class="mono">${esc(l.resource_type)}:${esc(l.resource_id)}</strong>
            <div>
              <span class="badge">${esc(l.scope)}</span>
              <button class="btn btn-danger btn-xs" onclick="releaseLock('${esc(l.resource_type)}','${esc(l.resource_id)}')">强制释放</button>
            </div>
          </div>
          <div class="muted-text">持有者: ${esc(l.holder_agent)} · 剩余 TTL: ${l.remaining_ttl_secs}s</div>
          <div class="ttl-bar"><div class="ttl-fill" style="width:${pct}%"></div></div>
        </div>`;
      }).join("");
    }

    const dlBadge = document.getElementById("deadlock-badge");
    if (deadlocks.has_deadlock) {
      dlBadge.className = "badge badge-err";
      dlBadge.innerText = `发现 ${deadlocks.cycles?.length || 1} 条死锁环路!`;
    } else {
      dlBadge.className = "badge badge-ok";
      dlBadge.innerText = "WFG 无环路";
    }

    const wdEl = document.getElementById("locks-wait-deadlock");
    let wdHtml = "";
    if (deadlocks.has_deadlock && deadlocks.cycles?.length) {
      wdHtml += deadlocks.cycles.map((c) => `<div class="item-box" style="border-color:var(--red);">
        <strong style="color:var(--red);">死锁环路:</strong> ${esc((c.agents || []).join(" → "))}
        <div class="muted-text">涉及资源: ${esc((c.resources || []).join(", "))}</div>
        <div class="muted-text">建议: ${esc(c.suggestion || "")}</div>
      </div>`).join("");
    }
    if (waits.length === 0) {
      wdHtml += '<div class="empty">等待队列为空</div>';
    } else {
      wdHtml += waits.map((w) => `<div class="item-box">
        <div class="item-top"><strong class="mono">${esc(w.resource_type)}:${esc(w.resource_id)}</strong><span class="badge badge-warn">P${w.priority} · ${esc(w.status)}</span></div>
        <div class="muted-text">等待 Agent: ${esc(w.waiting_agent)}</div>
      </div>`).join("");
    }
    wdEl.innerHTML = wdHtml;

    const histEl = document.getElementById("locks-history-list");
    if (history.length === 0) {
      histEl.innerHTML = '<div class="empty">暂无锁审计历史记录</div>';
    } else {
      histEl.innerHTML = `<table class="data-table"><thead><tr><th>时间</th><th>事件</th><th>目标</th><th>说明</th></tr></thead><tbody>` +
        history.map((h) => `<tr><td class="mono">${esc(h.created_at || "-")}</td><td><span class="badge">${esc(h.change_type)}</span></td><td class="mono">${esc(h.target)}</td><td>${esc(h.description || "")}</td></tr>`).join("") +
        `</tbody></table>`;
    }
  } catch (e) {
    console.error("locks err:", e);
  }
}

window.releaseLock = async function (resourceType, resourceId) {
  const token = (document.getElementById("lock-auth-token")?.value || "").trim();
  const headers = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;
  try {
    const res = await fetch("/api/v1/locks/release", {
      method: "POST",
      headers,
      body: JSON.stringify({ resource_type: resourceType, resource_id: resourceId, agent_id: "admin_web" }),
    });
    const json = await res.json();
    if (json.code !== 200) throw new Error(json.message);
    loadLocks();
    loadOverview();
  } catch (e) {
    alert("释放失败: " + e.message);
  }
};

/* ─── 4. 生态防爆路由器 ──────────────────────────────────────────── */
async function loadEcosystem() {
  try {
    state.ecoData = await apiGet("/api/v1/ecosystem");
    document.getElementById("cnt-mcps").innerText = state.ecoData.summary?.total_mcps || 0;
    document.getElementById("cnt-skills").innerText = state.ecoData.summary?.total_skills || 0;
    document.getElementById("cnt-svcs").innerText = state.ecoData.summary?.total_services || 0;
    renderEcosystemLists();
    runContextRoute();
  } catch (e) {
    console.error("eco err:", e);
  }
}

function renderEcosystemLists() {
  if (!state.ecoData) return;
  const d = state.ecoData;
  const kw = (document.getElementById("eco-skill-filter")?.value || "").trim().toLowerCase();

  document.getElementById("eco-mcps").innerHTML = (d.mcps || []).map((m) => `<div class="item-box">
    <div class="item-top"><strong>${esc(m.server_name)}</strong><span class="badge">${esc(m.client_source)} · ${esc(m.transport)}</span></div>
    <div class="mono muted-text">${esc(m.command_or_url)}</div>
  </div>`).join("") || '<div class="empty">暂无 MCP 记录</div>';

  const skills = (d.skills || []).filter((s) =>
    !kw || s.skill_name.toLowerCase().includes(kw) || s.description.toLowerCase().includes(kw)
  );
  document.getElementById("eco-skills").innerHTML = skills.slice(0, 80).map((s) => `<div class="item-box">
    <div class="item-top"><strong>${esc(s.skill_name)}</strong><span class="badge">~${s.est_tokens} tok</span></div>
    <div class="muted-text">${esc(s.description.slice(0, 100))}</div>
  </div>`).join("") || '<div class="empty">无匹配 SKILL</div>';

  document.getElementById("eco-services").innerHTML = (d.services || []).map((s) => `<div class="item-box">
    <div class="item-top"><strong>${esc(s.service_name)}</strong><span class="badge ${s.status === "healthy" ? "badge-ok" : ""}">${esc(s.status)}</span></div>
    <div class="mono muted-text">${esc(s.endpoint)}</div>
    <div class="muted-text">鉴权环境变量: ${esc(s.auth_env_var || "无需")}</div>
  </div>`).join("") || '<div class="empty">暂无外部服务</div>';
}

async function runContextRoute() {
  const intent = document.getElementById("route-intent")?.value || "";
  const budget = document.getElementById("route-budget")?.value || "600";
  const badge = document.getElementById("eco-savings-badge");
  const out = document.getElementById("route-result");
  try {
    const plan = await apiGet(`/api/v1/ecosystem/route?intent=${encodeURIComponent(intent)}&budget=${encodeURIComponent(budget)}`);
    badge.innerText = `消耗 ${plan.route_tokens_used} tok · 节省 ${Number(plan.saved_pct || 0).toFixed(1)}% (全量 ${plan.total_skills_full_tokens} tok)`;
    out.innerHTML = `
      <div class="item-box"><strong>命中 SKILL (${plan.skills.length})</strong>${plan.skills.map((s) => `<div class="mt-12"><span class="badge badge-ok">${esc(s.skill_name)}</span><div class="muted-text">${esc(s.summary)}</div></div>`).join("") || '<div class="muted-text">无</div>'}</div>
      <div class="item-box"><strong>推荐 MCP (${plan.mcps.length})</strong>${plan.mcps.map((m) => `<div class="mt-12"><span class="badge">${esc(m.server_name)}</span><div class="mono muted-text">${esc(m.transport)} · ${esc(m.client_source)}</div></div>`).join("") || '<div class="muted-text">无</div>'}</div>
      <div class="item-box"><strong>外部服务 (${plan.services.length})</strong>${plan.services.map((s) => `<div class="mt-12"><span class="badge badge-warn">${esc(s.service_name)}</span><div class="mono muted-text">${esc(s.endpoint)}</div></div>`).join("") || '<div class="muted-text">无</div>'}</div>
    `;
  } catch (e) {
    badge.innerText = "路由失败";
    out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

/* ─── 5. 规约、记忆、变更审计与 Mesh ──────────────────────────────── */
async function loadGovernance() {
  try {
    const q = document.getElementById("gov-mem-search")?.value || "";
    const [gov, mesh] = await Promise.all([
      apiGet(`/api/v1/governance?q=${encodeURIComponent(q)}`),
      apiGet("/api/v1/mesh/status"),
    ]);
    state.govData = gov;

    document.getElementById("gov-rules").innerHTML = (gov.placement_rules || []).map((r) => `<div class="item-box">
      <div class="item-top"><strong>${esc(r.rule_name)}</strong><span class="badge">P${r.priority} · ${esc(r.pattern_type)}</span></div>
      <div class="mono" style="color:var(--green);">${esc(r.target_base_dir)}</div>
      <div class="muted-text">${esc(r.description || "")}</div>
    </div>`).join("");

    document.getElementById("gov-memories").innerHTML = (gov.memories || []).map((m) => `<div class="item-box">
      <div class="item-top"><span class="badge">${esc(m.memory_type)}</span><span class="muted-text">权重 ${m.weight}</span></div>
      <div>${esc(m.summary || m.content.slice(0, 140))}</div>
    </div>`).join("") || '<div class="empty">暂无长期记忆</div>';

    document.getElementById("gov-changes").innerHTML = (gov.recent_changes || []).slice(0, 40).map((c) => `<div class="item-box">
      <div class="item-top"><span class="badge">${esc(c.change_type)}</span><span class="mono muted-text">${esc(c.created_at || "")}</span></div>
      <div class="mono">${esc(c.target)}</div>
    </div>`).join("") || '<div class="empty">暂无变更记录</div>';

    const pf = gov.preflight || {};
    document.getElementById("gov-mesh-preflight").innerHTML = `
      <div class="item-box">
        <div class="item-top"><strong>Peer Mesh 联邦节点</strong><span class="badge ${mesh.mesh_enabled ? "badge-ok" : ""}">${mesh.mesh_enabled ? "已启用" : "单机模式"}</span></div>
        <div class="mono muted-text">Node: ${esc(mesh.node_id)} · OS: ${esc(mesh.platform_os)} · Peers: ${(mesh.peers || []).length}</div>
      </div>
      <div class="item-box">
        <div class="item-top"><strong>Preflight 运行环境预检</strong><span class="badge ${pf.passed_required ? "badge-ok" : "badge-err"}">${pf.passed_required ? "PASS" : "FAIL"}</span></div>
        ${(pf.items || []).map((it) => `<div class="muted-text" style="margin-top:4px;">• <strong>${esc(it.name)}</strong> [${esc(it.status)}]: ${esc(it.message)}</div>`).join("")}
      </div>
    `;
  } catch (e) {
    console.error("gov err:", e);
  }
}

/* ─── SSE 实时事件流 ─────────────────────────────────────────────── */
function initSSE() {
  const badge = document.getElementById("sse-badge");
  const stream = document.getElementById("event-stream");
  try {
    const es = new EventSource("/api/v1/stream");
    es.onopen = () => {
      badge.className = "sse-pill";
      badge.innerText = "SSE 在线";
    };
    es.onmessage = (event) => {
      const div = document.createElement("div");
      div.className = "event-item ok";
      div.innerText = event.data;
      stream.prepend(div);
      while (stream.children.length > 80) stream.removeChild(stream.lastChild);
      loadOverview();
      if (state.activeView === "locks") loadLocks();
    };
    es.onerror = () => {
      badge.className = "sse-pill connecting";
      badge.innerText = "SSE 重连中";
    };
  } catch (e) {
    badge.innerText = "SSE 不可用";
  }
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
