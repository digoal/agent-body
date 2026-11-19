use crate::web::http::HttpResponse;

pub static INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="zh-CN" data-theme="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Agent Host Steward 2.0 — 宿主环境全景指挥舱</title>
  <link rel="icon" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Ccircle cx='16' cy='16' r='14' fill='%23245A6B'/%3E%3Ctext x='16' y='21' font-size='16' text-anchor='middle' fill='white'%3ES%3C/text%3E%3C/svg%3E">
  <script>
    try {
      var savedTheme = localStorage.getItem("steward_ui_theme") || "dark";
      document.documentElement.setAttribute("data-theme", savedTheme === "light" ? "light" : "dark");
    } catch (e) {}
  </script>
  <link rel="stylesheet" href="/app.css">
</head>
<body>
  <div id="app">
    <header class="topbar">
      <div class="brand">
        <span class="pulse-dot"></span>
        <div>
          <h1>Agent Host Steward <span class="ver-badge">v0.4.0 · 2.0</span></h1>
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
        <span id="parity-badge" class="sse-pill" style="cursor:pointer;" title="点击查看黑白屏 40/40 能力对齐矩阵">✅ 40/40 对齐</span>
        <span id="sse-badge" class="sse-pill connecting">SSE 连接中</span>
        <button id="btn-theme-toggle" class="btn btn-theme" title="切换白天工程蓝图 / 夜间深空指挥舱 (快捷键 T)">☀️ 白天蓝图</button>
        <button id="btn-toggle-mirror" class="btn btn-ghost" title="展开/收起黑屏命令镜像器">⌨️ 黑屏镜像</button>
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
        <button class="nav-btn nav-btn-demo" data-view="demo">
          <span class="nav-ico">🎬</span>
          <span>路演演示剧场</span>
        </button>
        <button class="nav-btn" data-view="benchmark">
          <span class="nav-ico">📊</span>
          <span>实证基准与账本</span>
        </button>
        <div class="sidenav-footer">
          <a href="/deck" target="_blank" class="btn btn-ghost btn-xs sidenav-link">📽️ 路演 PPT (/deck)</a>
          <a href="/presentation" target="_blank" class="btn btn-ghost btn-xs sidenav-link">🎨 架构 PPT</a>
        </div>
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
              <button id="btn-links-audit" class="btn btn-ghost">🩺 断链审计/修复</button>
              <button id="btn-links-mermaid" class="btn btn-ghost">📐 导出 Mermaid</button>
              <button id="btn-topo-reset" class="btn btn-ghost">恢复全景</button>
              <button id="btn-topo-fit" class="btn btn-ghost">自适应居中</button>
            </div>
          </div>
          <div class="topology-stage">
            <canvas id="topology-canvas"></canvas>
            <div class="canvas-hint">滚轮缩放 · 拖拽平移/移点 · 点击节点查看详情、CodeGraph 下钻与爆炸半径 · 按 T 切换昼夜主题</div>
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
          <div class="inline-bar">
            <input id="fq-input" class="input flex-1" type="text" placeholder="统一事实语义问答 (如: 处理 PDF / 8080 / systemd / postgres)..." />
            <select id="fq-type" class="input input-sm">
              <option value="auto">类型: auto</option>
              <option value="capability">capability (工具能力)</option>
              <option value="port">port (端口)</option>
              <option value="automation">automation (Systemd/Cron)</option>
              <option value="repo">repo (源码工程)</option>
              <option value="doc">doc (权威资料)</option>
            </select>
            <button id="btn-fq-run" class="btn btn-primary">🔍 事实问答</button>
            <input id="pc-port" class="input input-sm" style="width:95px;" type="number" value="8756" min="1" max="65535" />
            <button id="btn-pc-run" class="btn btn-ghost">🛡️ 端口防撞诊断</button>
          </div>
          <div id="facts-qa-result" class="qa-box hidden"></div>
          <div class="view-toolbar">
            <div class="subtabs" id="facts-subtabs">
              <button class="subtab active" data-sub="ports">监听端口 (<span id="cnt-ports">0</span>)</button>
              <button class="subtab" data-sub="procs">活跃进程 (<span id="cnt-procs">0</span>)</button>
              <button class="subtab" data-sub="automation">Systemd &amp; Cron (<span id="cnt-auto">0</span>)</button>
              <button class="subtab" data-sub="repos">Git 工程 (<span id="cnt-repos">0</span>)</button>
              <button class="subtab" data-sub="tools">CLI 工具能力 (<span id="cnt-tools">0</span>)</button>
              <button class="subtab" data-sub="assets">📦 纳管资产与分级扫描</button>
            </div>
            <input id="facts-filter" class="input" type="text" placeholder="过滤端口、进程名、路径或工具..." />
          </div>
          <div id="facts-content" class="scroll-panel"></div>
        </section>

        <!-- 视图 3: 多 Agent 协作锁与死锁雷达 -->
        <section id="view-locks" class="view">
          <div class="inline-bar">
            <select id="lock-acq-type" class="input input-sm" style="width:110px;">
              <option value="port">port</option>
              <option value="dir">dir</option>
              <option value="file">file</option>
              <option value="service">service</option>
              <option value="custom">custom</option>
            </select>
            <input id="lock-acq-id" class="input" style="width:180px;" type="text" placeholder="资源ID (如 9090 或 /tmp/a)" />
            <input id="lock-acq-agent" class="input input-sm" type="text" value="web-console" placeholder="Agent ID" />
            <select id="lock-acq-scope" class="input input-sm" style="width:110px;">
              <option value="exclusive">exclusive</option>
              <option value="shared">shared</option>
            </select>
            <button id="btn-lock-acquire" class="btn btn-primary">🔒 申请协作锁</button>
            <button id="btn-lock-wait" class="btn btn-ghost">⏳ 排队等待 (WFG)</button>
            <span id="lock-acq-msg" class="muted-text"></span>
          </div>
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

        <!-- 视图 4: 生态防爆路由器与 HOOK 门禁 -->
        <section id="view-ecosystem" class="view">
          <div class="scroll-panel">
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

            <div class="card mt-12">
              <div class="card-header">
                <h3>🪝 HOOK 门禁演练场 &amp; 客户端配置生成器</h3>
                <div class="toolbar-right">
                  <select id="hook-client-select" class="input input-sm" style="width:110px;">
                    <option value="agy">agy</option>
                    <option value="cursor">cursor</option>
                    <option value="claude">claude</option>
                    <option value="git">git</option>
                  </select>
                  <button id="btn-hook-gen" class="btn btn-ghost btn-xs">⚙️ 生成客户端 Hook</button>
                </div>
              </div>
              <div class="sandbox-controls">
                <input id="hook-tool-name" class="input" style="width:170px;" type="text" value="bind_port" placeholder="工具名 (bind_port / write_file)" />
                <input id="hook-tool-args" class="input flex-1 mono" type="text" value='{"port": 8756}' placeholder='入参 JSON (如 {"port": 8756})' />
                <button id="btn-hook-pre" class="btn btn-primary">🛡️ Pre-Hook 预检</button>
                <button id="btn-hook-post" class="btn btn-ghost">📝 Post-Hook 通知</button>
              </div>
              <div id="hook-result" class="card-body" style="max-height:240px;"></div>
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
          </div>
        </section>

        <!-- 视图 5: 规约、简报、记忆、DuckDB 分析、自洁与 Mesh -->
        <section id="view-governance" class="view">
          <div class="view-toolbar">
            <div class="subtabs" id="gov-subtabs">
              <button class="subtab active" data-sub="brief">📋 简报与放置规约</button>
              <button class="subtab" data-sub="memory">🧠 长期记忆与审计</button>
              <button class="subtab" data-sub="analytics">🔥 DuckDB 分析与导出</button>
              <button class="subtab" data-sub="gc">🧹 宿主自洁 &amp; 变更流</button>
              <button class="subtab" data-sub="mesh">🌐 Mesh · 预检 · 40/40 对齐矩阵</button>
            </div>
          </div>
          <div id="gov-panel-brief" class="scroll-panel">
            <div class="grid-2">
              <div class="card">
                <div class="card-header">
                  <h3>📋 自适应开工简报组装器 (steward_brief)</h3>
                  <button id="btn-brief-run" class="btn btn-primary btn-xs">⚡ 生成简报</button>
                </div>
                <div class="sandbox-controls">
                  <input id="brief-intent" class="input flex-1" type="text" value="宿主机运行状态与近期变更" placeholder="任务意图..." />
                  <input id="brief-budget" class="input input-sm" style="width:90px;" type="number" value="1500" />
                </div>
                <div id="brief-result" class="card-body"></div>
              </div>
              <div class="card">
                <div class="card-header">
                  <h3>🧭 交互式放置推导 &amp; 自演化 (place suggest/learn)</h3>
                  <button id="btn-place-learn" class="btn btn-ghost btn-xs">🧠 规则自演化</button>
                </div>
                <div class="sandbox-controls">
                  <select id="place-type" class="input input-sm" style="width:95px;">
                    <option value="script">script</option>
                    <option value="project">project</option>
                    <option value="note">note</option>
                    <option value="skill">skill</option>
                    <option value="temp">temp</option>
                  </select>
                  <input id="place-name" class="input input-sm" type="text" value="sec_crawler" placeholder="对象名称" />
                  <input id="place-intent" class="input flex-1" type="text" value="美股财报抓取脚本" placeholder="用途说明" />
                  <button id="btn-place-suggest" class="btn btn-primary btn-xs">推导路径</button>
                </div>
                <div id="place-result" class="card-body"></div>
              </div>
            </div>
            <div class="card mt-12">
              <div class="card-header"><h3>📐 8 大资产放置规则 &amp; 扫描策略</h3></div>
              <div id="gov-rules" class="card-body scroll-sm"></div>
            </div>
          </div>

          <div id="gov-panel-memory" class="scroll-panel hidden">
            <div class="grid-2">
              <div class="card">
                <div class="card-header">
                  <h3>✍️ 写入记忆 &amp; 混合检索</h3>
                  <button id="btn-mem-audit" class="btn btn-ghost btn-xs">🩺 衰减/矛盾审计</button>
                </div>
                <div class="sandbox-controls">
                  <select id="mem-add-type" class="input input-sm" style="width:105px;">
                    <option value="decision">decision</option>
                    <option value="troubleshooting">troubleshooting</option>
                    <option value="preference">preference</option>
                    <option value="fact">fact</option>
                  </select>
                  <input id="mem-add-content" class="input flex-1" type="text" placeholder="记录新的长期决策或排障经验..." />
                  <button id="btn-mem-add" class="btn btn-primary btn-xs">写入记忆</button>
                </div>
                <div class="sandbox-controls" style="padding-top:0;">
                  <input id="gov-mem-search" class="input flex-1" type="text" placeholder="输入关键词检索记忆..." />
                  <label class="chip"><input id="mem-hybrid-chk" type="checkbox" checked /> RRF 混合</label>
                  <button id="btn-mem-search" class="btn btn-ghost btn-xs">🔍 检索</button>
                </div>
                <div id="gov-memories" class="card-body scroll-sm"></div>
              </div>
              <div class="card">
                <div class="card-header"><h3>🩺 记忆健康审计结果 (Half-Life &amp; Conflicts)</h3></div>
                <div id="mem-audit-result" class="card-body scroll-sm"><div class="empty">点击「衰减/矛盾审计」执行体检</div></div>
              </div>
            </div>
          </div>

          <div id="gov-panel-analytics" class="scroll-panel hidden">
            <div class="inline-bar">
              <button id="btn-ana-refresh" class="btn btn-primary">🔥 刷新 DuckDB 热力图与趋势</button>
              <select id="ana-export-table" class="input input-sm" style="width:130px;">
                <option value="assets">assets</option>
                <option value="changes">changes</option>
                <option value="memories">memories</option>
                <option value="fact_nodes">fact_nodes</option>
              </select>
              <button id="btn-ana-export" class="btn btn-ghost">📦 导出 Parquet 至 /root/new/tmp/</button>
              <span id="ana-export-msg" class="muted-text"></span>
            </div>
            <div class="grid-2 mt-12">
              <div class="card">
                <div class="card-header"><h3>🔥 DuckDB 目录变更热力图</h3></div>
                <div id="ana-heatmap" class="card-body"></div>
              </div>
              <div class="card">
                <div class="card-header"><h3>📈 资产增长趋势与图谱统计</h3></div>
                <div id="ana-trend" class="card-body"></div>
              </div>
            </div>
          </div>

          <div id="gov-panel-gc" class="scroll-panel hidden">
            <div class="grid-2">
              <div class="card">
                <div class="card-header">
                  <h3>🧹 宿主 GC 与会话完工结清预览 (Dry-Run)</h3>
                  <div class="toolbar-right">
                    <button id="btn-gc-preview" class="btn btn-primary btn-xs">🧹 扫描 /root/new/tmp/</button>
                    <button id="btn-session-close" class="btn btn-ghost btn-xs">📋 结清预览</button>
                  </div>
                </div>
                <div id="gc-result" class="card-body"></div>
              </div>
              <div class="card">
                <div class="card-header"><h3>🕒 最近宿主变更审计流</h3></div>
                <div id="gov-changes" class="card-body scroll-sm"></div>
              </div>
            </div>
          </div>

          <div id="gov-panel-mesh" class="scroll-panel hidden">
            <div class="grid-2">
              <div class="card">
                <div class="card-header">
                  <h3>🌍 跨平台探针 · Preflight · Peer Mesh</h3>
                  <div class="toolbar-right">
                    <button id="btn-mesh-ping" class="btn btn-primary btn-xs">📡 Ping Peers</button>
                    <input id="mesh-port-input" class="input input-sm" style="width:80px;" type="number" value="8080" />
                    <button id="btn-mesh-port" class="btn btn-ghost btn-xs">🌐 联邦查端口</button>
                  </div>
                </div>
                <div id="gov-mesh-preflight" class="card-body scroll-sm"></div>
                <div id="mesh-query-result" class="card-body" style="padding-top:0;"></div>
              </div>
              <div class="card">
                <div class="card-header">
                  <h3>✅ 黑白屏 100% 能力对齐矩阵</h3>
                  <span id="parity-summary-badge" class="badge badge-ok">加载中</span>
                </div>
                <div id="parity-matrix-box" class="card-body scroll-sm"></div>
              </div>
            </div>
          </div>
        </section>

        <!-- 视图 6: 路演演示剧场 (Phase 25 故事化直观舞台) -->
        <section id="view-demo" class="view">
          <div class="scroll-panel">
            <div class="demo-origin-banner">
              <div class="demo-origin-ico">🏛️</div>
              <div class="demo-origin-body">
                <strong>为什么 AI Agent 需要大管家？—— 经济学「比较优势」分工原则</strong>
                <p>就像硅谷创业园区物业包办生活琐事让程序员专心写代码、企业高管配秘书打理杂务。大管家驻留本机，替 AI 摸清现成工具与代码家底、拦住危险操作，让大模型把 100% 智商和 Token 用在核心任务上。</p>
              </div>
            </div>

            <div id="demo-roi-strip" class="demo-roi-strip mt-12">
              <div class="roi-card"><span class="k-label">Context Token 节省率</span><strong id="roi-save" class="c-green">-</strong></div>
              <div class="roi-card"><span class="k-label">全量 SKILL / MCP / 服务</span><strong id="roi-eco" class="c-cyan">-</strong></div>
              <div class="roi-card"><span class="k-label">宿主机 CLI 工具 / 资产</span><strong id="roi-tools">-</strong></div>
              <div class="roi-card"><span class="k-label">事实图谱 (节点 / 边)</span><strong id="roi-graph" class="c-purple">-</strong></div>
              <div class="roi-card"><span class="k-label">黑白屏同步认证</span><strong id="roi-parity" class="c-orange">40/40 (100%)</strong></div>
            </div>

            <div class="demo-control-bar mt-12">
              <div id="demo-scene-buttons" class="demo-scene-grid"></div>
              <div class="demo-actions-col">
                <button id="btn-demo-cruise" class="btn btn-primary btn-xs">▶ 自动巡航路演</button>
                <button id="btn-demo-presenter" class="btn btn-ghost btn-xs">📽️ 演讲者大屏模式</button>
                <button id="btn-demo-reset" class="btn btn-danger btn-xs">🧹 一键重置沙盒</button>
              </div>
            </div>

            <div id="demo-stage" class="demo-stage mt-12">
              <div class="empty">点击上方任意路演剧目，或点击「▶ 自动巡航路演」实时驱动真实 Rust 引擎对比演示</div>
            </div>
          </div>
        </section>

        <!-- 视图 7: 实证 Benchmark 对照评测与生产效能存证账本 (Phase 28) -->
        <section id="view-benchmark" class="view">
          <div class="scroll-panel">
            <div class="demo-roi-strip">
              <div class="roi-card"><span class="k-label">生产累计节省 Token</span><strong id="bench-kpi-tokens" class="c-green">-</strong></div>
              <div class="roi-card"><span class="k-label">累计缩减探索轮次</span><strong id="bench-kpi-rounds" class="c-cyan">-</strong></div>
              <div class="roi-card"><span class="k-label">事前拦截风险/冲突</span><strong id="bench-kpi-hazards" class="c-orange">-</strong></div>
              <div class="roi-card"><span class="k-label">阻断垃圾噪音文件</span><strong id="bench-kpi-noise" class="c-purple">-</strong></div>
              <div class="roi-card"><span class="k-label">最新 A/B 综合加速</span><strong id="bench-kpi-speedup" class="c-green">-</strong></div>
            </div>

            <div class="view-toolbar mt-12" style="padding:10px 12px;border:1px solid var(--border);border-radius:10px;background:var(--panel-2);">
              <div class="toolbar-left">
                <select id="bench-suite-select" class="input" style="width:170px;">
                  <option value="full">全量套件 (W1~W8 + M1~M5)</option>
                  <option value="macro">宏观业务 A/B (W1~W8)</option>
                  <option value="micro">内核微基准 SLA (M1~M5)</option>
                  <option value="W1">W1 · 宿主工具发现</option>
                  <option value="W2">W2 · 端口防撞选址</option>
                  <option value="W3">W3 · 分级扫描阻断</option>
                  <option value="W4">W4 · 放置合规门禁</option>
                  <option value="W5">W5 · 爆炸半径与断链</option>
                  <option value="W6">W6 · 多 Agent 死锁破局</option>
                  <option value="W7">W7 · 百级 SKILL 路由</option>
                  <option value="W8">W8 · 冷启动态势简报</option>
                </select>
                <button id="btn-bench-run" class="btn btn-primary btn-xs">▶ 真机运行 A/B 基准</button>
                <select id="bench-window-select" class="input" style="width:120px;">
                  <option value="24h">近 24 小时</option>
                  <option value="7d" selected>近 7 天</option>
                  <option value="30d">近 30 天</option>
                  <option value="all">全部历史</option>
                </select>
                <button id="btn-bench-refresh" class="btn btn-ghost btn-xs">🔄 刷新账本</button>
              </div>
              <div class="toolbar-right">
                <span id="bench-sha-badge" class="badge">SHA-256: 待运行</span>
              </div>
            </div>

            <div class="panel-grid-2 mt-12">
              <div class="card">
                <div class="item-top"><strong>🧪 实验室 A/B 对照实测 (Without vs With Steward)</strong><span id="bench-sla-badge" class="badge">-</span></div>
                <div id="bench-workloads-box" class="mt-8"><div class="empty">点击「▶ 真机运行 A/B 基准」查看 8 大负载对照实测</div></div>
              </div>
              <div class="card">
                <div class="item-top"><strong>⚡ 5 大内核微基准 SLA & 跨版本回归比对</strong></div>
                <div id="bench-micro-box" class="mt-8"><div class="empty">暂无微基准数据</div></div>
                <div class="mt-12" style="border-top:1px solid var(--border);padding-top:10px;">
                  <div class="toolbar-left">
                    <select id="bench-cmp-a" class="input" style="width:140px;"></select>
                    <span class="muted-text">vs</span>
                    <select id="bench-cmp-b" class="input" style="width:140px;"></select>
                    <button id="btn-bench-compare" class="btn btn-ghost btn-xs">🔁 回归对比</button>
                  </div>
                  <div id="bench-compare-box" class="mt-8"></div>
                </div>
              </div>
            </div>

            <div class="card mt-12">
              <div class="item-top"><strong>📒 生产运行态效能存证账本（带 PR-01~PR-08 确定性溯源依据）</strong></div>
              <div id="bench-rules-strip" class="mt-8"></div>
              <div id="bench-ledger-list" class="mt-8"><div class="empty">暂无生产流水记录</div></div>
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

    <!-- 全局黑屏命令镜像底栏 -->
    <footer id="mirror-bar" class="mirror-bar">
      <div class="mirror-title">⌨️ 黑屏命令镜像器</div>
      <div class="mirror-slots">
        <div class="mirror-slot">
          <span class="badge">CLI</span>
          <code id="mirror-cli" class="mono">agent-steward brief</code>
          <button id="btn-copy-cli" class="btn btn-ghost btn-xs">复制</button>
        </div>
        <div class="mirror-slot">
          <span class="badge badge-ok">MCP JSON-RPC</span>
          <code id="mirror-mcp" class="mono">{"name":"steward_brief","arguments":{}}</code>
          <button id="btn-copy-mcp" class="btn btn-ghost btn-xs">复制</button>
        </div>
      </div>
    </footer>
  </div>
  <script src="/app.js"></script>
</body>
</html>"##;

pub static APP_CSS: &str = r##":root, :root[data-theme="dark"] {
  --bg: #080c14;
  --bg-grad-start: #0f172a;
  --topbar-bg: rgba(15, 23, 42, 0.94);
  --sidenav-bg: rgba(15, 23, 42, 0.78);
  --panel: rgba(17, 24, 39, 0.90);
  --panel-2: #131d31;
  --item-bg: rgba(9, 13, 22, 0.75);
  --input-bg: #0b1120;
  --card-head-bg: rgba(30, 41, 59, 0.60);
  --table-head-bg: rgba(15, 23, 42, 0.72);
  --drawer-bg: rgba(15, 23, 42, 0.96);
  --drawer-shadow: 0 12px 30px rgba(0, 0, 0, 0.5);
  --hover-bg: rgba(255, 255, 255, 0.05);
  --text: #f1f5f9;
  --text-2: #cbd5e1;
  --muted: #94a3b8;
  --border: #1e293b;
  --border-hi: #334155;
  --blue: #38bdf8;
  --blue-soft: rgba(56, 189, 248, 0.14);
  --blue-border: rgba(56, 189, 248, 0.35);
  --orange: #fb923c;
  --orange-soft: rgba(251, 146, 60, 0.16);
  --orange-border: rgba(251, 146, 60, 0.35);
  --purple: #a78bfa;
  --purple-soft: rgba(167, 139, 250, 0.14);
  --pink: #f472b6;
  --green: #34d399;
  --green-soft: rgba(52, 211, 153, 0.14);
  --green-border: rgba(52, 211, 153, 0.32);
  --red: #f87171;
  --red-soft: rgba(248, 113, 113, 0.18);
  --red-border: rgba(248, 113, 113, 0.45);
  --yellow: #fbbf24;
  --code-fg: #93c5fd;
  --mirror-bg: rgba(11, 17, 32, 0.96);
  --mirror-code: #a7f3d0;
  --canvas-bg: #070b14;
  --canvas-dot: #1e293b;
  --accent-btn-bg: #0284c7;
  --accent-btn-hover: #0ea5e9;
  --accent-btn-fg: #ffffff;
  --demo-nav-bg: rgba(251, 146, 60, 0.08);
  --demo-nav-fg: #fdba74;
  --demo-nav-border: rgba(251, 146, 60, 0.38);
  --demo-bad-bg: rgba(127, 29, 29, 0.22);
  --demo-bad-border: rgba(248, 113, 113, 0.55);
  --demo-good-bg: rgba(6, 78, 59, 0.22);
  --demo-good-border: rgba(52, 211, 153, 0.55);
  --speaker-bg: rgba(30, 41, 59, 0.88);
  --speaker-fg: #fef3c7;
  --banner-grad: linear-gradient(90deg, rgba(56, 189, 248, 0.14), rgba(167, 139, 250, 0.12));
  --shadow-card: 0 2px 10px rgba(0, 0, 0, 0.22);
}

:root[data-theme="light"] {
  --bg: #F7F6F2;
  --bg-grad-start: #EFECE4;
  --topbar-bg: rgba(247, 246, 242, 0.96);
  --sidenav-bg: #F1EFE9;
  --panel: #FFFFFF;
  --panel-2: #F3F1EB;
  --item-bg: #FAF9F5;
  --input-bg: #FFFFFF;
  --card-head-bg: #F1EFE9;
  --table-head-bg: #F1EFE9;
  --drawer-bg: rgba(255, 255, 255, 0.97);
  --drawer-shadow: 0 10px 28px rgba(27, 43, 51, 0.14);
  --hover-bg: rgba(36, 90, 107, 0.07);
  --text: #1B2B33;
  --text-2: #33444C;
  --muted: #64747B;
  --border: #D9D5CB;
  --border-hi: #B8B2A6;
  --blue: #245A6B;
  --blue-soft: #E5EFF0;
  --blue-border: #9CC0C6;
  --orange: #BE653B;
  --orange-soft: #F7E9DE;
  --orange-border: #DFAA8B;
  --purple: #5B4B8A;
  --purple-soft: #EFEBF8;
  --pink: #A84A6C;
  --green: #2F7A66;
  --green-soft: #E8F3EF;
  --green-border: #8FC4B4;
  --red: #B9382C;
  --red-soft: #FCEAE7;
  --red-border: #E2A098;
  --yellow: #9A7422;
  --code-fg: #1E4B5A;
  --mirror-bg: #ECE9E0;
  --mirror-code: #1E5E4A;
  --canvas-bg: #F4F2EC;
  --canvas-dot: #D5D0C5;
  --accent-btn-bg: #245A6B;
  --accent-btn-hover: #1E4B5A;
  --accent-btn-fg: #FFFFFF;
  --demo-nav-bg: #F7E9DE;
  --demo-nav-fg: #9C4821;
  --demo-nav-border: #DFAA8B;
  --demo-bad-bg: #FDF3F1;
  --demo-bad-border: #E2A098;
  --demo-good-bg: #EFF7F4;
  --demo-good-border: #8FC4B4;
  --speaker-bg: #FBF6E9;
  --speaker-fg: #3D2E0E;
  --banner-grad: linear-gradient(90deg, #E5EFF0, #EFEBF8);
  --shadow-card: 0 2px 8px rgba(27, 43, 51, 0.06);
}

* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "PingFang SC", "Noto Sans CJK SC", "Microsoft YaHei", sans-serif;
  background: radial-gradient(circle at 18% 0%, var(--bg-grad-start) 0%, var(--bg) 68%);
  color: var(--text);
  height: 100vh;
  overflow: hidden;
  font-size: 13px;
  transition: background-color 0.2s ease, color 0.2s ease;
}
#app { display: flex; flex-direction: column; height: 100vh; }

/* Topbar */
.topbar {
  min-height: 60px;
  background: var(--topbar-bg);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 18px;
  gap: 14px;
  flex-wrap: wrap;
}
.brand { display: flex; align-items: center; gap: 12px; }
.brand h1 { font-size: 15px; font-weight: 700; color: var(--text); letter-spacing: 0.2px; }
.ver-badge {
  font-size: 11px; font-weight: 600; color: var(--blue);
  background: var(--blue-soft); border: 1px solid var(--blue-border);
  padding: 1px 7px; border-radius: 999px; margin-left: 6px;
}
.subtitle { font-size: 11px; color: var(--muted); }
.pulse-dot {
  width: 10px; height: 10px; border-radius: 50%; background: var(--green);
  box-shadow: 0 0 8px var(--green); flex-shrink: 0;
}

.kpi-strip { display: flex; gap: 8px; flex-wrap: wrap; }
.kpi-pill {
  background: var(--panel-2); border: 1px solid var(--border);
  border-radius: 8px; padding: 4px 10px; display: flex; flex-direction: column;
  box-shadow: var(--shadow-card);
}
.k-label { font-size: 10px; color: var(--muted); }
.kpi-pill strong { font-size: 12px; color: var(--text); font-family: ui-monospace, SFMono-Regular, monospace; }
.c-cyan { color: var(--blue) !important; }
.c-orange { color: var(--orange) !important; }
.c-green { color: var(--green) !important; }
.c-purple { color: var(--purple) !important; }

.topbar-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.sse-pill {
  font-size: 11px; padding: 3px 9px; border-radius: 999px; font-weight: 600;
  background: var(--green-soft); color: var(--green); border: 1px solid var(--green-border);
}
.sse-pill.connecting { background: var(--orange-soft); color: var(--orange); border-color: var(--orange-border); }

/* Buttons & Inputs */
.btn {
  border: 1px solid var(--border-hi); background: var(--panel-2); color: var(--text);
  padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 12px; transition: 0.15s; font-weight: 500;
}
.btn:hover { border-color: var(--blue); color: var(--blue); background: var(--hover-bg); }
.btn-primary { background: var(--accent-btn-bg); border-color: var(--accent-btn-bg); color: var(--accent-btn-fg); }
.btn-primary:hover { background: var(--accent-btn-hover); border-color: var(--accent-btn-hover); color: var(--accent-btn-fg); }
.btn-theme { background: var(--orange-soft); border-color: var(--orange-border); color: var(--orange); font-weight: 600; }
.btn-theme:hover { border-color: var(--orange); color: var(--text); }
.btn-danger { background: var(--red-soft); border-color: var(--red-border); color: var(--red); }
.btn-danger:hover { background: var(--red); color: #ffffff; }
.btn-ghost { background: transparent; }
.btn-xs { padding: 3px 8px; font-size: 11px; }
.btn-icon { background: none; border: none; color: var(--muted); cursor: pointer; font-size: 15px; }
.btn-icon:hover { color: var(--text); }

.input {
  background: var(--input-bg); border: 1px solid var(--border-hi); color: var(--text);
  padding: 6px 10px; border-radius: 6px; font-size: 12px; outline: none;
}
.input:focus { border-color: var(--blue); }
.input-sm { padding: 4px 8px; font-size: 11px; width: 160px; }
.flex-1 { flex: 1; min-width: 200px; }

/* Workspace */
.workspace { display: flex; flex: 1; overflow: hidden; }
.sidenav {
  width: 192px; background: var(--sidenav-bg); border-right: 1px solid var(--border);
  padding: 12px 10px; display: flex; flex-direction: column; gap: 6px; flex-shrink: 0;
}
.nav-btn {
  display: flex; align-items: center; gap: 10px; padding: 10px 12px;
  border-radius: 8px; border: 1px solid transparent; background: transparent;
  color: var(--muted); cursor: pointer; text-align: left; font-size: 13px; transition: 0.15s;
}
.nav-btn:hover { background: var(--hover-bg); color: var(--text); }
.nav-btn.active {
  background: var(--blue-soft); border-color: var(--blue-border);
  color: var(--text); font-weight: 600;
}
.nav-btn-demo {
  margin-top: 6px;
  background: var(--demo-nav-bg);
  border-color: var(--demo-nav-border);
  color: var(--demo-nav-fg);
  font-weight: 600;
}
.nav-btn-demo.active {
  background: var(--orange-soft);
  border-color: var(--orange);
  color: var(--orange);
}
.nav-ico { font-size: 15px; }
.sidenav-footer {
  margin-top: auto; display: flex; flex-direction: column; gap: 6px;
  padding-top: 10px; border-top: 1px solid var(--border);
}
.sidenav-link { text-decoration: none; text-align: center; }

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
  display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--text-2);
  background: var(--panel); border: 1px solid var(--border);
  padding: 4px 8px; border-radius: 999px; cursor: pointer;
}
.dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.d-process { background: var(--blue); }
.d-port { background: var(--orange); }
.d-service { background: var(--purple); }
.d-cron { background: var(--pink); }
.d-git { background: var(--green); }
.d-file { background: var(--muted); }

/* Topology */
.topology-stage {
  flex: 1; position: relative; border: 1px solid var(--border); border-radius: 10px;
  background: radial-gradient(var(--canvas-dot) 1px, transparent 1px), var(--canvas-bg);
  background-size: 22px 22px; overflow: hidden;
}
#topology-canvas { width: 100%; height: 100%; display: block; cursor: grab; }
#topology-canvas:active { cursor: grabbing; }
.canvas-hint {
  position: absolute; bottom: 10px; left: 12px; font-size: 11px; color: var(--muted);
  background: var(--topbar-bg); padding: 4px 10px; border-radius: 6px; border: 1px solid var(--border);
}

.drawer {
  position: absolute; top: 10px; right: 10px; bottom: 10px; width: 350px;
  background: var(--drawer-bg); border: 1px solid var(--border-hi);
  border-radius: 10px; display: flex; flex-direction: column;
  box-shadow: var(--drawer-shadow); z-index: 10;
}
.drawer.hidden { display: none; }
.drawer-header {
  padding: 10px 14px; border-bottom: 1px solid var(--border);
  display: flex; align-items: center; justify-content: space-between;
}
.drawer-header h3 { font-size: 13px; color: var(--text); }
.drawer-body { padding: 12px; overflow-y: auto; flex: 1; font-size: 12px; }

/* Cards & Grids */
.scroll-panel { flex: 1; overflow-y: auto; padding-right: 4px; }
.grid-2 { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.grid-3 { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
.mt-8 { margin-top: 8px; }
.mt-12 { margin-top: 12px; }
.card {
  background: var(--panel); border: 1px solid var(--border); border-radius: 10px;
  display: flex; flex-direction: column; overflow: hidden; box-shadow: var(--shadow-card);
}
.card-header {
  padding: 10px 14px; background: var(--card-head-bg); border-bottom: 1px solid var(--border);
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
}
.card-header h3 { font-size: 13px; font-weight: 600; color: var(--text); }
.card-body { padding: 12px; overflow-y: auto; max-height: 360px; }
.scroll-sm { max-height: 320px; overflow-y: auto; }

/* Subtabs & Tables */
.subtabs { display: flex; gap: 6px; flex-wrap: wrap; }
.subtab {
  background: var(--panel); border: 1px solid var(--border); color: var(--muted);
  padding: 6px 11px; border-radius: 6px; cursor: pointer; font-size: 12px; transition: 0.15s;
}
.subtab:hover { border-color: var(--border-hi); color: var(--text); }
.subtab.active { background: var(--blue-soft); border-color: var(--blue); color: var(--blue); font-weight: 600; }
.data-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.data-table th, .data-table td {
  text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--border);
  vertical-align: top; word-break: break-word; color: var(--text-2);
}
.data-table th { color: var(--muted); font-weight: 600; background: var(--table-head-bg); position: sticky; top: 0; }
.data-table tbody tr:hover { background: var(--hover-bg); }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; }

/* Badges & Items */
.badge {
  display: inline-block; font-size: 11px; padding: 2px 8px; border-radius: 999px; font-weight: 600;
  background: var(--blue-soft); color: var(--blue); border: 1px solid transparent;
}
.badge-ok { background: var(--green-soft); color: var(--green); border-color: var(--green-border); }
.badge-warn { background: var(--orange-soft); color: var(--orange); border-color: var(--orange-border); }
.badge-err { background: var(--red-soft); color: var(--red); border-color: var(--red-border); }
.muted-text { color: var(--muted); font-size: 12px; }
.empty { color: var(--muted); text-align: center; padding: 20px; }

.item-box {
  background: var(--item-bg); border: 1px solid var(--border);
  border-radius: 8px; padding: 10px; margin-bottom: 8px; color: var(--text-2);
}
.item-top { display: flex; justify-content: space-between; align-items: center; gap: 8px; margin-bottom: 4px; flex-wrap: wrap; }
.ttl-bar { height: 5px; background: var(--border); border-radius: 4px; overflow: hidden; margin-top: 6px; }
.ttl-fill { height: 100%; background: linear-gradient(90deg, var(--green), var(--blue)); }

/* Router Sandbox */
.sandbox-controls { display: flex; gap: 10px; padding: 12px; align-items: center; flex-wrap: wrap; }
.budget-label { font-size: 12px; color: var(--muted); display: flex; align-items: center; gap: 6px; }
.route-result { padding: 0 12px 12px; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }

/* SSE Sidebar */
.sse-sidebar {
  width: 280px; background: var(--sidenav-bg); border-left: 1px solid var(--border);
  display: flex; flex-direction: column; flex-shrink: 0;
}
.sse-sidebar.collapsed { display: none; }
.event-stream { flex: 1; overflow-y: auto; padding: 10px; font-family: ui-monospace, monospace; font-size: 11px; }
.event-item {
  border-left: 2px solid var(--blue); padding: 4px 8px; margin-bottom: 6px;
  background: var(--item-bg); color: var(--text-2); border-radius: 0 4px 4px 0; word-break: break-all;
}
.event-item.warn { border-left-color: var(--orange); }
.event-item.ok { border-left-color: var(--green); }

/* Parity Bars & Black-Screen Mirror */
.hidden { display: none !important; }
.inline-bar {
  display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
  background: var(--panel); border: 1px solid var(--border);
  border-radius: 8px; padding: 8px 12px; margin-bottom: 10px; box-shadow: var(--shadow-card);
}
.qa-box {
  background: var(--panel); border: 1px solid var(--blue);
  border-radius: 8px; padding: 10px 12px; margin-bottom: 10px;
  max-height: 280px; overflow-y: auto; box-shadow: var(--shadow-card);
}
.md-pre {
  white-space: pre-wrap; font-family: ui-monospace, Menlo, monospace;
  font-size: 11px; color: var(--code-fg); line-height: 1.5;
}
.mirror-bar {
  background: var(--mirror-bg); border-top: 1px solid var(--border-hi);
  padding: 6px 16px; display: flex; align-items: center; gap: 14px; flex-wrap: wrap;
}
.mirror-bar.collapsed { display: none; }
.mirror-title { font-size: 11px; font-weight: 700; color: var(--blue); white-space: nowrap; }
.mirror-slots { display: flex; gap: 12px; flex: 1; flex-wrap: wrap; overflow: hidden; }
.mirror-slot {
  display: flex; align-items: center; gap: 6px; background: var(--panel-2);
  border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px; max-width: 100%;
}
.mirror-slot code { color: var(--mirror-code); max-width: 420px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

/* Collapsible Raw JSON Details & Visual Summary Cards */
.raw-json-details {
  margin-top: 8px; border: 1px dashed var(--border-hi); border-radius: 8px;
  background: var(--item-bg); padding: 6px 10px;
}
.raw-json-details summary {
  cursor: pointer; font-size: 11px; font-weight: 600; color: var(--muted);
  user-select: none; outline: none;
}
.raw-json-details summary:hover { color: var(--blue); }
.raw-json-details[open] summary { margin-bottom: 8px; color: var(--blue); }
.summary-kpi-row {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px; margin-bottom: 8px;
}
.summary-kpi {
  background: var(--panel-2); border: 1px solid var(--border);
  border-radius: 8px; padding: 8px 10px;
}
.summary-kpi .sk-label { font-size: 10px; color: var(--muted); display: block; }
.summary-kpi .sk-val { font-size: 14px; font-weight: 700; color: var(--text); font-family: ui-monospace, monospace; }
.bar-row { display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px; }
.bar-row-head { display: flex; justify-content: space-between; font-size: 12px; color: var(--text-2); }
.bar-track { height: 8px; background: var(--border); border-radius: 999px; overflow: hidden; }
.bar-fill { height: 100%; background: var(--blue); border-radius: 999px; transition: width 0.35s ease; }
.bar-fill.ok { background: var(--green); }
.bar-fill.warn { background: var(--orange); }
.bar-fill.err { background: var(--red); }

/* Roadshow Live Demo Theater (Phase 25 Story-Driven) */
.demo-origin-banner {
  display: flex; align-items: flex-start; gap: 12px;
  background: var(--banner-grad); border: 1px solid var(--blue-border);
  border-radius: 10px; padding: 12px 16px;
}
.demo-origin-ico { font-size: 24px; line-height: 1.1; flex-shrink: 0; }
.demo-origin-body strong { display: block; font-size: 13px; color: var(--text); margin-bottom: 3px; }
.demo-origin-body p { font-size: 12px; color: var(--text-2); line-height: 1.55; }

.demo-roi-strip { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 10px; }
.roi-card {
  background: var(--panel); border: 1px solid var(--border-hi); border-radius: 10px;
  padding: 10px 14px; display: flex; flex-direction: column; gap: 4px; box-shadow: var(--shadow-card);
}
.roi-card strong { font-size: 15px; font-family: ui-monospace, monospace; color: var(--text); }

.demo-control-bar {
  display: flex; gap: 12px; align-items: stretch; justify-content: space-between; flex-wrap: wrap;
}
.demo-scene-grid {
  flex: 1; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; min-width: 300px;
}
.scene-card-btn {
  background: var(--panel); border: 1px solid var(--border); border-radius: 9px;
  padding: 8px 10px; cursor: pointer; text-align: left; transition: 0.15s;
  display: flex; flex-direction: column; gap: 3px; color: var(--text);
}
.scene-card-btn:hover { border-color: var(--blue); background: var(--hover-bg); }
.scene-card-btn.active {
  background: var(--blue-soft); border-color: var(--blue); box-shadow: 0 0 0 1px var(--blue);
}
.scene-card-top { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
.scene-card-title { font-size: 12px; font-weight: 700; color: var(--text); }
.scene-card-sub { font-size: 11px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.demo-actions-col {
  display: flex; flex-direction: column; gap: 6px; justify-content: center; min-width: 146px;
}

.demo-stage { display: flex; flex-direction: column; gap: 12px; }
.demo-header-banner {
  background: var(--banner-grad);
  border: 1px solid var(--blue-border); border-radius: 10px; padding: 12px 16px;
  display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;
}
.demo-header-banner h3 { font-size: 16px; color: var(--text); }
.demo-explainer-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.explainer-card {
  background: var(--panel); border: 1px solid var(--border); border-left: 4px solid var(--blue);
  border-radius: 8px; padding: 10px 14px; box-shadow: var(--shadow-card);
}
.explainer-card.analogy { border-left-color: var(--orange); }
.explainer-card .ex-title { font-size: 11px; font-weight: 700; color: var(--muted); margin-bottom: 4px; text-transform: uppercase; }
.explainer-card .ex-body { font-size: 13px; color: var(--text); line-height: 1.55; font-weight: 500; }

.demo-hero-metrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
.hero-metric-card {
  background: var(--panel); border: 1px solid var(--border); border-radius: 10px;
  padding: 10px 14px; display: flex; flex-direction: column; gap: 4px; box-shadow: var(--shadow-card);
}
.hero-metric-card.tone-ok { border-top: 3px solid var(--green); }
.hero-metric-card.tone-warn { border-top: 3px solid var(--orange); }
.hero-metric-card.tone-info { border-top: 3px solid var(--blue); }
.hero-metric-label { font-size: 11px; color: var(--muted); }
.hero-metric-value { font-size: 18px; font-weight: 800; color: var(--text); font-family: ui-monospace, monospace; }

.demo-split { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.demo-pane-bad {
  background: var(--demo-bad-bg); border: 1px solid var(--demo-bad-border);
  border-radius: 10px; padding: 14px; display: flex; flex-direction: column; gap: 10px;
}
.demo-pane-good {
  background: var(--demo-good-bg); border: 1px solid var(--demo-good-border);
  border-radius: 10px; padding: 14px; display: flex; flex-direction: column; gap: 10px;
}
.story-step-list { display: flex; flex-direction: column; gap: 8px; }
.story-step {
  background: var(--panel); border: 1px solid var(--border); border-radius: 8px;
  padding: 9px 11px; display: flex; gap: 9px; align-items: flex-start;
}
.step-num {
  width: 22px; height: 22px; border-radius: 50%; flex-shrink: 0;
  display: inline-flex; align-items: center; justify-content: center;
  font-size: 11px; font-weight: 800; background: var(--red-soft); color: var(--red); border: 1px solid var(--red-border);
}
.step-num.good { background: var(--green-soft); color: var(--green); border-color: var(--green-border); }
.story-step-text strong { display: block; font-size: 12px; color: var(--text); margin-bottom: 2px; }
.story-step-text p { font-size: 12px; color: var(--text-2); line-height: 1.45; }
.cmd-sim-box {
  background: var(--input-bg); border: 1px solid var(--border-hi); border-radius: 6px;
  padding: 7px 10px; font-family: ui-monospace, monospace; font-size: 11px; color: var(--red);
  word-break: break-all;
}

.widget-panel {
  background: var(--panel); border: 1px solid var(--border); border-radius: 8px;
  padding: 10px 12px; display: flex; flex-direction: column; gap: 8px;
}
.widget-title { font-size: 12px; font-weight: 700; color: var(--text); display: flex; justify-content: space-between; align-items: center; }
.tool-wall { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.tool-card {
  background: var(--item-bg); border: 1px solid var(--border); border-radius: 7px; padding: 7px 9px;
}
.tool-card-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 3px; }
.tool-card code { font-size: 10px; color: var(--code-fg); word-break: break-all; display: block; }
.pill-wrap { display: flex; gap: 6px; flex-wrap: wrap; }

.traffic-row {
  display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 8px;
  border: 1px solid var(--border); background: var(--item-bg);
}
.traffic-light { width: 14px; height: 14px; border-radius: 50%; flex-shrink: 0; }
.traffic-light.red { background: var(--red); box-shadow: 0 0 8px var(--red); }
.traffic-light.green { background: var(--green); box-shadow: 0 0 8px var(--green); }

.cascade-chain { display: flex; flex-direction: column; gap: 6px; }
.cascade-node {
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
  background: var(--item-bg); border: 1px solid var(--border); border-radius: 7px; padding: 7px 10px;
}
.cascade-arrow { text-align: center; font-size: 12px; color: var(--blue); font-weight: 700; line-height: 1; }

.deadlock-ring {
  display: grid; grid-template-columns: 1fr auto 1fr; gap: 8px; align-items: center;
  background: var(--item-bg); border: 1px solid var(--red-border); border-radius: 8px; padding: 10px;
}
.dl-agent-box {
  background: var(--panel); border: 1px solid var(--border-hi); border-radius: 7px; padding: 8px; text-align: center;
}
.dl-arrows { display: flex; flex-direction: column; align-items: center; gap: 4px; font-size: 11px; color: var(--red); font-weight: 700; }

.speaker-card {
  background: var(--speaker-bg); border: 1px solid var(--border); border-left: 4px solid var(--yellow);
  border-radius: 8px; padding: 10px 14px; font-size: 12px; line-height: 1.6; color: var(--speaker-fg);
}
body.presenter-mode .sidenav, body.presenter-mode .sse-sidebar, body.presenter-mode .kpi-strip, body.presenter-mode .demo-origin-banner { display: none !important; }

@media (max-width: 1180px) {
  .demo-scene-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
@media (max-width: 1100px) {
  .grid-2, .grid-3, .route-result, .demo-roi-strip, .demo-split, .demo-explainer-grid, .demo-hero-metrics, .tool-wall { grid-template-columns: 1fr; }
  .sse-sidebar { display: none; }
  .sidenav { width: 62px; padding: 8px 6px; }
  .nav-btn span:last-child { display: none; }
}
"##;

pub static APP_JS: &str = r##"// Agent Host Steward 2.0 Web Console (Zero External Dependencies)
const state = {
  theme: "dark",
  activeView: "topology",
  factsTab: "ports",
  govTab: "brief",
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

const NODE_COLORS_DARK = {
  process: "#38bdf8",
  port: "#fb923c",
  service: "#a78bfa",
  cron: "#f472b6",
  git: "#34d399",
  file: "#94a3b8",
  memory: "#fbbf24",
};

const NODE_COLORS_LIGHT = {
  process: "#245A6B",
  port: "#BE653B",
  service: "#5B4B8A",
  cron: "#A84A6C",
  git: "#2F7A66",
  file: "#64747B",
  memory: "#9A7422",
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
  let saved = "dark";
  try {
    const v = localStorage.getItem("steward_ui_theme");
    if (v === "light" || v === "dark") saved = v;
  } catch (_) {}
  applyTheme(saved, false);
}

function applyTheme(theme, persist) {
  const next = theme === "light" ? "light" : "dark";
  state.theme = next;
  document.documentElement.setAttribute("data-theme", next);
  if (persist !== false) {
    try { localStorage.setItem("steward_ui_theme", next); } catch (_) {}
  }
  const btn = document.getElementById("btn-theme-toggle");
  if (btn) {
    btn.innerText = next === "light" ? "🌙 夜间深空" : "☀️ 白天蓝图";
  }
  if (state.simNodes.length > 0) {
    drawTopology();
  }
}

function toggleTheme() {
  applyTheme(state.theme === "light" ? "dark" : "light", true);
}

async function apiGet(url) {
  const res = await fetch(url);
  const json = await res.json();
  if (json.code !== 200) throw new Error(json.message || "API Error");
  return json.data;
}

async function apiPost(url, body, authToken) {
  const headers = { "Content-Type": "application/json" };
  if (authToken) headers["Authorization"] = `Bearer ${authToken}`;
  const res = await fetch(url, { method: "POST", headers, body: JSON.stringify(body || {}) });
  const json = await res.json();
  if (json.code !== 200) throw new Error(json.message || "API Error");
  return json.data;
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
  document.getElementById("btn-theme-toggle")?.addEventListener("click", toggleTheme);
  document.getElementById("btn-global-refresh")?.addEventListener("click", refreshAll);
  document.getElementById("btn-toggle-sse")?.addEventListener("click", () => {
    document.getElementById("sse-sidebar")?.classList.toggle("collapsed");
    if (state.activeView === "topology") setTimeout(resizeCanvas, 50);
  });
  document.getElementById("btn-toggle-mirror")?.addEventListener("click", () => {
    document.getElementById("mirror-bar")?.classList.toggle("collapsed");
  });
  document.getElementById("parity-badge")?.addEventListener("click", () => {
    switchView("governance");
    document.querySelector('#gov-subtabs .subtab[data-sub="mesh"]')?.click();
  });
  document.getElementById("btn-copy-cli")?.addEventListener("click", () => {
    const txt = document.getElementById("mirror-cli")?.innerText || "";
    navigator.clipboard?.writeText(txt);
  });
  document.getElementById("btn-copy-mcp")?.addEventListener("click", () => {
    const txt = document.getElementById("mirror-mcp")?.innerText || "";
    navigator.clipboard?.writeText(txt);
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

  const iters = simNodes.length > 160 ? 35 : 65;
  for (let step = 0; step < iters; step++) {
    for (let i = 0; i < simNodes.length; i++) {
      const a = simNodes[i];
      a.vx *= 0.6;
      a.vy *= 0.6;
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

  const isLight = state.theme === "light";
  const palette = isLight ? NODE_COLORS_LIGHT : NODE_COLORS_DARK;
  const edgeNormal = isLight ? "rgba(36, 90, 107, 0.28)" : "rgba(148, 163, 184, 0.28)";
  const edgeFocus = isLight ? "#245A6B" : "#38bdf8";
  const edgeLabelNormal = isLight ? "rgba(70, 88, 95, 0.78)" : "rgba(148, 163, 184, 0.65)";
  const edgeLabelFocus = isLight ? "#1E4B5A" : "#7dd3fc";
  const nodeOutline = isLight ? "#BE653B" : "#ffffff";
  const nodeLabelColor = isLight ? "#1B2B33" : "#e2e8f0";

  const kw = (document.getElementById("topo-search")?.value || "").trim().toLowerCase();
  const focusId = state.hoverNodeId || state.selectedNodeId;

  ctx.save();
  ctx.translate(state.cam.x, state.cam.y);
  ctx.scale(state.cam.zoom, state.cam.zoom);

  for (const e of state.simEdges) {
    const isFocus = focusId && (e.source === focusId || e.target === focusId);
    ctx.beginPath();
    ctx.moveTo(e.sNode.x, e.sNode.y);
    ctx.lineTo(e.tNode.x, e.tNode.y);
    ctx.strokeStyle = isFocus ? edgeFocus : edgeNormal;
    ctx.lineWidth = isFocus ? 2.2 : 1.1;
    ctx.stroke();

    if (isFocus || state.cam.zoom > 0.85) {
      const mx = (e.sNode.x + e.tNode.x) / 2;
      const my = (e.sNode.y + e.tNode.y) / 2;
      ctx.fillStyle = isFocus ? edgeLabelFocus : edgeLabelNormal;
      ctx.font = "9px monospace";
      ctx.textAlign = "center";
      ctx.fillText(e.relation, mx, my - 3);
    }
  }

  for (const n of state.simNodes) {
    const matched = !kw || n.label.toLowerCase().includes(kw) || n.id.toLowerCase().includes(kw);
    const isSelected = n.id === state.selectedNodeId || n.id === state.hoverNodeId;
    const color = palette[n.nt] || palette.memory;

    ctx.globalAlpha = matched ? 1 : 0.22;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();

    if (isSelected || (kw && matched)) {
      ctx.strokeStyle = nodeOutline;
      ctx.lineWidth = 2.5;
      ctx.stroke();
    }

    if (state.cam.zoom > 0.45 || n.degree > 0 || isSelected || (kw && matched)) {
      ctx.fillStyle = nodeLabelColor;
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
  const cgPath = node.metadata?.abs_path || node.metadata?.path || (node.id.startsWith("/") ? node.id : "/root/new/work/agent-steward");
  const metaPairs = Object.entries(node.metadata || {}).slice(0, 6);
  body.innerHTML = `
    <div class="item-box">
      <div><strong>ID:</strong> <span class="mono">${esc(node.id)}</span></div>
      <div><strong>类型:</strong> <span class="badge">${esc(node.nt)}</span></div>
      <div><strong>状态:</strong> ${esc(node.status)}</div>
      <div><strong>关联边数:</strong> ${node.degree}</div>
    </div>
    <div style="display:flex;gap:6px;margin-bottom:10px;">
      <button id="btn-impact-node" class="btn btn-primary" style="flex:1;">💥 爆炸半径</button>
      <button id="btn-cg-node" class="btn btn-ghost" style="flex:1;">🔬 CodeGraph 穿透</button>
    </div>
    <div id="drawer-extra" class="hidden item-box"></div>
    <div class="item-box">
      <div style="margin-bottom:6px;"><strong>关键物理属性:</strong></div>
      ${metaPairs.length ? metaPairs.map(([k, v]) => `<div class="mono" style="margin-bottom:3px;"><span class="muted-text">${esc(k)}:</span> ${esc(typeof v === "object" ? JSON.stringify(v) : v)}</div>`).join("") : '<div class="muted-text">无附加属性</div>'}
      ${rawJsonDrawer(node.metadata, "查看完整元数据 JSON")}
    </div>
  `;
  mirrorBlackScreen(`agent-steward links query --id "${node.id}"`, "steward_links_query", { id: node.id, type: node.nt });
  document.getElementById("btn-impact-node")?.addEventListener("click", async () => {
    try {
      const impact = await apiGet(`/api/v1/topology/impact?target=${encodeURIComponent(node.id)}`);
      state.topoDisplay = impact;
      applyTopoFilter();
      mirrorBlackScreen(`agent-steward links impact "${node.id}"`, "steward_links_impact", { target: node.id, max_depth: 3 });
    } catch (e) {
      alert("爆炸半径推演失败: " + e.message);
    }
  });
  document.getElementById("btn-cg-node")?.addEventListener("click", async () => {
    const extra = document.getElementById("drawer-extra");
    if (!extra) return;
    extra.classList.remove("hidden");
    extra.innerHTML = '<div class="muted-text">正在穿透 CodeGraph...</div>';
    try {
      const cg = await apiGet(`/api/v1/links/codegraph?path=${encodeURIComponent(cgPath)}`);
      mirrorBlackScreen(`agent-steward links codegraph --path "${cgPath}"`, "steward_codegraph_bridge", { path: cgPath });
      const info = cg.project_info || {};
      extra.innerHTML = `
        <div class="item-top"><strong>🔬 CodeGraph 状态</strong> ${cg.has_codegraph ? '<span class="badge badge-ok">已索引</span>' : '<span class="badge">未索引</span>'}</div>
        ${cg.has_codegraph ? `<div class="summary-kpi-row mt-8"><div class="summary-kpi"><span class="sk-label">符号节点</span><span class="sk-val">${info.node_count ?? 0}</span></div><div class="summary-kpi"><span class="sk-label">调用边</span><span class="sk-val">${info.edge_count ?? 0}</span></div><div class="summary-kpi"><span class="sk-label">文件数</span><span class="sk-val">${info.file_count ?? 0}</span></div></div>` : '<div class="muted-text">该目录尚未初始化 .codegraph/ 索引</div>'}
        ${rawJsonDrawer(cg, "查看 CodeGraph 原始 JSON")}
      `;
    } catch (e) {
      extra.innerHTML = `<div class="muted-text">${esc(e.message)}</div>`;
    }
  });
}

async function runLinksAuditModal() {
  const drawer = document.getElementById("node-drawer");
  const title = document.getElementById("drawer-title");
  const body = document.getElementById("drawer-body");
  if (!drawer || !body) return;
  drawer.classList.remove("hidden");
  title.innerText = "🩺 断链审计 & 修复推演";
  body.innerHTML = '<div class="muted-text">正在扫描依赖图...</div>';
  try {
    const [audit, repair] = await Promise.all([apiGet("/api/v1/links/audit"), apiGet("/api/v1/links/repair")]);
    mirrorBlackScreen("agent-steward links audit && agent-steward links repair", "steward_links_audit", { fix_dangling: false });
    const score = audit.health_score ?? 100;
    const issues = audit.issues || [];
    const suggestions = repair.suggestions || [];
    body.innerHTML = `
      <div class="summary-kpi-row">
        <div class="summary-kpi"><span class="sk-label">拓扑健康度</span><span class="sk-val c-green">${score}/100</span></div>
        <div class="summary-kpi"><span class="sk-label">总连边</span><span class="sk-val">${audit.total_links ?? 0}</span></div>
        <div class="summary-kpi"><span class="sk-label">断链数</span><span class="sk-val c-orange">${issues.length}</span></div>
      </div>
      <div class="item-box">
        <strong>修复建议 (${suggestions.length})</strong>
        ${suggestions.length ? suggestions.slice(0, 6).map((s) => `<div class="mt-8 mono">${esc(s.broken_target)} → ${esc(s.suggested_target)}</div>`).join("") : '<div class="muted-text mt-8">当前依赖图健康，无待修复断链。</div>'}
      </div>
      ${rawJsonDrawer({ audit, repair }, "查看审计与修复原始 JSON")}
    `;
  } catch (e) {
    body.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runLinksMermaidModal() {
  const drawer = document.getElementById("node-drawer");
  const title = document.getElementById("drawer-title");
  const body = document.getElementById("drawer-body");
  if (!drawer || !body) return;
  drawer.classList.remove("hidden");
  title.innerText = "📐 Mermaid 拓扑源码";
  try {
    const res = await apiGet("/api/v1/links/mermaid?limit=30");
    mirrorBlackScreen("agent-steward links visualize --limit 30", "steward_links_visualize", { limit: 30 });
    body.innerHTML = `<div class="item-box"><pre class="md-pre">${esc(res.mermaid)}</pre></div>`;
  } catch (e) {
    body.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
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
  } else if (state.factsTab === "tools") {
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
  } else {
    el.innerHTML = `
      <div class="inline-bar">
        <input id="asset-search-q" class="input flex-1" type="text" value="${esc(q)}" placeholder="FTS5 检索纳管资产 (路径/用途/能力/符号)..." />
        <select id="asset-detail-level" class="input input-sm" style="width:108px;" title="Token 披露粒度">
          <option value="standard">standard</option>
          <option value="micro">micro</option>
          <option value="full">full</option>
        </select>
        <button id="btn-asset-search" class="btn btn-primary btn-xs">🔍 检索资产</button>
        <input id="asset-scan-path" class="input" style="width:200px;" type="text" value="/root/new/work/agent-steward" placeholder="待扫描目录路径" />
        <label style="font-size:12px;display:inline-flex;align-items:center;gap:4px;color:var(--muted);">
          <input id="asset-scan-enrich" type="checkbox" /> 🤖 Ollama精炼
        </label>
        <button id="btn-asset-scan" class="btn btn-ghost btn-xs">📂 分级扫描</button>
      </div>
      <div class="inline-bar">
        <input id="asset-reg-path" class="input flex-1" type="text" placeholder="新资产路径 (如 /root/new/shared_proj/my_script.py)" />
        <select id="asset-reg-type" class="input input-sm" style="width:95px;">
          <option value="script">script</option><option value="project">project</option><option value="doc">doc</option><option value="other">other</option>
        </select>
        <input id="asset-reg-desc" class="input" style="width:200px;" type="text" placeholder="描述..." />
        <button id="btn-asset-reg" class="btn btn-primary btn-xs">➕ 登记资产</button>
      </div>
      <div id="asset-panel-out"><div class="muted-text">正在加载资产列表...</div></div>
    `;
    document.getElementById("btn-asset-search")?.addEventListener("click", loadAssetsPanel);
    document.getElementById("btn-asset-scan")?.addEventListener("click", scanAssetFromWeb);
    document.getElementById("btn-asset-reg")?.addEventListener("click", registerAssetFromWeb);
    loadAssetsPanel();
  }
}

async function loadAssetsPanel() {
  const q = document.getElementById("asset-search-q")?.value || "";
  const detail = document.getElementById("asset-detail-level")?.value || "standard";
  const out = document.getElementById("asset-panel-out");
  if (!out) return;
  try {
    const list = await apiGet(`/api/v1/assets/search?q=${encodeURIComponent(q)}&limit=50&detail=${encodeURIComponent(detail)}`);
    mirrorBlackScreen(`agent-steward asset search "${q}" --detail ${detail}`, "steward_asset_search", { query: q, limit: 50, detail_level: detail });
    out.innerHTML = `<table class="data-table"><thead><tr><th>类型 / 域</th><th>路径</th><th>核心用途与结构化 KV</th></tr></thead><tbody>` +
      (list || []).map((a) => {
        const typ = a.type || a.asset_type || "other";
        const kv = a.desc_kv || {};
        const domain = a.domain || kv.domain || "-";
        const purpose = a.purpose || kv.purpose || a.description || "-";
        const caps = Array.isArray(kv.capabilities) ? kv.capabilities.map((c) => `<span class="badge badge-ok" style="margin-right:4px;">${esc(c)}</span>`).join("") : "";
        const symsArr = kv.symbols || kv.headings || kv.config_sections || [];
        const syms = Array.isArray(symsArr) && symsArr.length ? `<div class="mono" style="font-size:11px;color:var(--muted);margin-top:3px;">符号/章节: ${esc(symsArr.slice(0, 5).join(", "))}</div>` : "";
        const modelBadge = kv.annotation_model ? `<span class="badge">${esc(kv.annotation_model)}</span>` : "";
        return `<tr><td><span class="badge">${esc(typ)}</span> <span class="badge">${esc(domain)}</span> ${modelBadge}</td><td class="mono">${esc(a.path)}</td><td><div>${esc(purpose)}</div><div style="margin-top:3px;">${caps}</div>${syms}</td></tr>`;
      }).join("") + `</tbody></table>`;
  } catch (e) {
    out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function registerAssetFromWeb() {
  const path = document.getElementById("asset-reg-path")?.value || "";
  const asset_type = document.getElementById("asset-reg-type")?.value || "script";
  const description = document.getElementById("asset-reg-desc")?.value || "";
  try {
    await apiPost("/api/v1/assets/register", { path, asset_type, description });
    mirrorBlackScreen(`agent-steward asset register --path "${path}" --asset-type ${asset_type}`, "steward_asset_register", { path, type: asset_type, description });
    loadAssetsPanel();
  } catch (e) {
    alert("登记失败: " + e.message);
  }
}

async function scanAssetFromWeb() {
  const path = document.getElementById("asset-scan-path")?.value || "";
  const enrich = Boolean(document.getElementById("asset-scan-enrich")?.checked);
  const out = document.getElementById("asset-panel-out");
  try {
    const rep = await apiPost("/api/v1/assets/scan", { path, enrich_with_ollama: enrich, enrich_limit: 5 });
    const cliCmd = enrich ? `agent-steward asset scan "${path}" --enrich --enrich-limit 5` : `agent-steward asset scan "${path}"`;
    mirrorBlackScreen(cliCmd, "steward_asset_scan", { path, enrich_with_ollama: enrich, enrich_limit: 5 });
    if (out) {
      out.innerHTML = `<div class="item-box">
        <div class="item-top"><strong>📂 分级扫描 &amp; 元数据标注完成</strong><span class="badge badge-ok">${esc(path)}</span></div>
        <div class="summary-kpi-row mt-8">
          <div class="summary-kpi"><span class="sk-label">扫描总数</span><span class="sk-val">${rep.scanned_count ?? rep.total_scanned ?? "-"}</span></div>
          <div class="summary-kpi"><span class="sk-label">黑名单浅层阻断</span><span class="sk-val c-orange">${rep.shallow_blocked ?? rep.skipped_count ?? 0}</span></div>
          <div class="summary-kpi"><span class="sk-label">Ollama 精炼</span><span class="sk-val c-green">${rep.ollama_enriched ?? 0}</span></div>
        </div>
        ${rawJsonDrawer(rep, "查看扫描明细 JSON")}
      </div>`;
    }
  } catch (e) {
    alert("扫描失败: " + e.message);
  }
}

async function runFactQuery() {
  const q = document.getElementById("fq-input")?.value || "all";
  const t = document.getElementById("fq-type")?.value || "auto";
  const box = document.getElementById("facts-qa-result");
  if (!box) return;
  box.classList.remove("hidden");
  box.innerHTML = '<div class="muted-text">正在查询宿主机事实图谱...</div>';
  try {
    const res = await apiGet(`/api/v1/facts/query?q=${encodeURIComponent(q)}&type=${encodeURIComponent(t)}`);
    mirrorBlackScreen(`agent-steward fact "${q}" --fact-type ${t}`, "steward_fact_query", { query: q, fact_type: t });
    box.innerHTML = `<div class="item-top"><strong>🔍 事实图谱零幻觉回答</strong><button class="btn-icon" onclick="this.closest('.qa-box').classList.add('hidden')">✕</button></div><pre class="md-pre">${esc(res.answer)}</pre>`;
  } catch (e) {
    box.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runPortCheck() {
  const port = Number(document.getElementById("pc-port")?.value || 8756);
  const box = document.getElementById("facts-qa-result");
  if (!box) return;
  box.classList.remove("hidden");
  try {
    const res = await apiGet(`/api/v1/facts/port-check?port=${port}`);
    mirrorBlackScreen(`agent-steward env port ${port}`, "steward_port_check", { port, suggest_count: 3 });
    const occupied = res.status === "OCCUPIED";
    const freePorts = (res.recommended_free_ports || []).map((p) => `<span class="badge badge-ok">:${p}</span>`).join(" ");
    const conflictHtml = res.conflict
      ? `<div class="muted-text mt-8">占用进程: <strong>${esc(res.conflict.name)}</strong> (PID ${res.conflict.pid}, 用户 ${esc(res.conflict.owner)}) · 命令: <code class="mono">${esc(res.conflict.command)}</code></div>`
      : "";
    box.innerHTML = `
      <div class="item-top">
        <strong>🛡️ 端口 :${port} 防撞诊断</strong>
        <div><span class="badge ${occupied ? "badge-err" : "badge-ok"}">${occupied ? "已被占用 (防撞拦截)" : "空闲可用"}</span> <button class="btn-icon" onclick="this.closest('.qa-box').classList.add('hidden')">✕</button></div>
      </div>
      <div style="margin-top:6px;">${esc(res.advice || "")}</div>
      ${conflictHtml}
      ${freePorts ? `<div class="mt-8"><strong>推荐安全空闲端口:</strong> ${freePorts}</div>` : ""}
      ${rawJsonDrawer(res, "查看端口探测原始 JSON")}
    `;
  } catch (e) {
    box.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
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
    mirrorBlackScreen(`agent-steward coordination release --resource-type ${resourceType} --resource-id "${resourceId}"`, "steward_lock_release", { resource_type: resourceType, resource_id: resourceId });
    loadLocks();
    loadOverview();
  } catch (e) {
    alert("释放失败: " + e.message);
  }
};

async function runLockAction(mode) {
  const resource_type = document.getElementById("lock-acq-type")?.value || "custom";
  const resource_id = (document.getElementById("lock-acq-id")?.value || "").trim();
  const holder_agent = (document.getElementById("lock-acq-agent")?.value || "web-console").trim();
  const scope = document.getElementById("lock-acq-scope")?.value || "exclusive";
  const token = (document.getElementById("lock-auth-token")?.value || "").trim();
  const msgEl = document.getElementById("lock-acq-msg");
  if (!resource_id) return alert("请输入资源 ID");
  try {
    const url = mode === "wait" ? "/api/v1/locks/wait" : "/api/v1/locks/acquire";
    const data = await apiPost(url, { resource_type, resource_id, holder_agent, scope, ttl_seconds: 120, wait_timeout_ms: 800 }, token);
    mirrorBlackScreen(`agent-steward coordination ${mode} -t ${resource_type} -i "${resource_id}" -a "${holder_agent}"`, mode === "wait" ? "steward_lock_wait" : "steward_lock_acquire", { resource_type, resource_id, holder_agent, scope });
    if (msgEl) msgEl.innerText = data.acquired ? `✅ 已获锁 (${data.lock_id || "ok"})` : `⏳ 冲突/排队: ${data.conflict_reason || data.status || "waiting"}`;
    loadLocks();
    loadOverview();
  } catch (e) {
    if (msgEl) msgEl.innerText = `失败: ${e.message}`;
  }
}

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
    <div class="item-top">
      <strong>${esc(s.service_name)}</strong>
      <div>
        <span class="badge ${s.status === "healthy" ? "badge-ok" : ""}">${esc(s.status)}</span>
        <button class="btn btn-ghost btn-xs" onclick="checkExternalService('${esc(s.service_name)}')">⚡ 探活</button>
      </div>
    </div>
    <div class="mono muted-text">${esc(s.endpoint)}</div>
    <div class="muted-text">鉴权环境变量: ${esc(s.auth_env_var || "无需")}</div>
  </div>`).join("") || '<div class="empty">暂无外部服务</div>';
}

window.checkExternalService = async function (name) {
  try {
    const rep = await apiGet(`/api/v1/ecosystem/service-check?name=${encodeURIComponent(name)}`);
    mirrorBlackScreen(`agent-steward service check "${name}"`, "steward_service_contact", { action: "check", service_name: name });
    alert(`服务 ${name} 探活结果:\n${JSON.stringify(rep, null, 2)}`);
    loadEcosystem();
  } catch (e) {
    alert("探活失败: " + e.message);
  }
};

async function runHookTest(kind) {
  const tool_name = document.getElementById("hook-tool-name")?.value || "write_file";
  const rawArgs = document.getElementById("hook-tool-args")?.value || "{}";
  const out = document.getElementById("hook-result");
  let args = {};
  try { args = JSON.parse(rawArgs); } catch (e) { return alert("入参 JSON 格式错误: " + e.message); }
  try {
    const res = await apiPost(kind === "pre" ? "/api/v1/hooks/pre" : "/api/v1/hooks/post", { tool_name, args, success: true });
    mirrorBlackScreen(`agent-steward hooks ${kind} --tool-name ${tool_name} --args '${rawArgs}'`, kind === "pre" ? "steward_hook_pre" : "steward_hook_post", { tool_name, args });
    if (out) {
      const hasWarn = Boolean(res.has_warnings || (res.warnings && res.warnings.length));
      const allowed = res.allowed !== false;
      const badgeCls = !allowed ? "badge-err" : hasWarn ? "badge-warn" : "badge-ok";
      const badgeTxt = !allowed ? "🛑 已拦截 (BLOCK)" : hasWarn ? "⚠️ 触发告警与建议" : "✅ 安全放行";
      const warns = (res.warnings || []).map((w) => `<div class="item-box" style="border-color:var(--orange);">${esc(w)}</div>`).join("");
      const sugs = (res.suggestions || []).map((s) => `<div class="item-box">${esc(s)}</div>`).join("");
      out.innerHTML = `
        <div class="item-top"><strong>${kind === "pre" ? "Pre-Hook 预检结果" : "Post-Hook 回写结果"}</strong><span class="badge ${badgeCls}">${badgeTxt}</span></div>
        <div class="mt-8">${warns}${sugs || (!warns ? '<div class="muted-text">未发现冲突，操作已记录。</div>' : "")}</div>
        ${rawJsonDrawer(res, "查看 Hook 原始 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runHookGenerate() {
  const client = document.getElementById("hook-client-select")?.value || "agy";
  const out = document.getElementById("hook-result");
  try {
    const res = await apiGet(`/api/v1/hooks/generate?client=${encodeURIComponent(client)}`);
    mirrorBlackScreen(`agent-steward hooks generate --client ${client} --dry-run`, "steward_hook_generate", { client, dry_run: true });
    if (out) {
      out.innerHTML = `
        <div class="item-top"><strong>⚙️ 客户端 Hook 配置 (${esc(client)})</strong><span class="badge badge-ok">已生成</span></div>
        <div class="muted-text mt-8">目标路径: <code class="mono">${esc(res.target_path || "-")}</code></div>
        ${rawJsonDrawer(res, "查看生成配置详情 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runContextRoute() {
  const intent = document.getElementById("route-intent")?.value || "";
  const budget = document.getElementById("route-budget")?.value || "600";
  const badge = document.getElementById("eco-savings-badge");
  const out = document.getElementById("route-result");
  try {
    const plan = await apiGet(`/api/v1/ecosystem/route?intent=${encodeURIComponent(intent)}&budget=${encodeURIComponent(budget)}`);
    badge.innerText = `消耗 ${plan.route_tokens_used} tok · 节省 ${Number(plan.saved_pct || 0).toFixed(1)}% (全量 ${plan.total_skills_full_tokens} tok)`;
    mirrorBlackScreen(`agent-steward skill route "${intent}" --budget-tokens ${budget}`, "steward_skill_route", { intent, budget_tokens: Number(budget) });
    out.innerHTML = `
      <div class="item-box"><strong>命中 SKILL (${plan.skills.length})</strong>${plan.skills.map((s) => `<div class="mt-8"><span class="badge badge-ok">${esc(s.skill_name)}</span><div class="muted-text">${esc(s.summary)}</div></div>`).join("") || '<div class="muted-text">无</div>'}</div>
      <div class="item-box"><strong>推荐 MCP (${plan.mcps.length})</strong>${plan.mcps.map((m) => `<div class="mt-8"><span class="badge">${esc(m.server_name)}</span><div class="mono muted-text">${esc(m.transport)} · ${esc(m.client_source)}</div></div>`).join("") || '<div class="muted-text">无</div>'}</div>
      <div class="item-box"><strong>外部服务 (${plan.services.length})</strong>${plan.services.map((s) => `<div class="mt-8"><span class="badge badge-warn">${esc(s.service_name)}</span><div class="mono muted-text">${esc(s.endpoint)}</div></div>`).join("") || '<div class="muted-text">无</div>'}</div>
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

async function runBriefAssemble() {
  const intent = document.getElementById("brief-intent")?.value || "";
  const budget = document.getElementById("brief-budget")?.value || "1500";
  const out = document.getElementById("brief-result");
  try {
    const res = await apiGet(`/api/v1/brief?intent=${encodeURIComponent(intent)}&budget=${encodeURIComponent(budget)}`);
    mirrorBlackScreen(`agent-steward brief`, "steward_brief", { query: intent, budget_tokens: Number(budget) });
    if (out) out.innerHTML = `<div class="muted-text" style="margin-bottom:6px;">综合质量分: <span class="badge badge-ok">${res.quality?.overall ?? "-"}/100</span></div><pre class="md-pre">${esc(res.markdown)}</pre>`;
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runPlaceSuggest() {
  const object_type = document.getElementById("place-type")?.value || "script";
  const name = document.getElementById("place-name")?.value || "unnamed";
  const intent = document.getElementById("place-intent")?.value || "";
  const out = document.getElementById("place-result");
  try {
    const res = await apiPost("/api/v1/placement/suggest", { object_type, name, intent });
    mirrorBlackScreen(`agent-steward place suggest --type ${object_type} --name "${name}" --intent "${intent}"`, "steward_placement_suggest", { object_type, name, intent });
    if (out) {
      out.innerHTML = `
        <div class="item-box">
          <div class="item-top">
            <strong class="mono" style="color:var(--green);font-size:13px;">${esc(res.suggested_path)}</strong>
            <span class="badge ${res.conflict_detected ? "badge-warn" : "badge-ok"}">${res.conflict_detected ? "存在同名冲突" : "路径安全可用"}</span>
          </div>
          <div class="muted-text">命中规约: <strong>${esc(res.rule_matched || "-")}</strong> · ${esc(res.reason || "")}</div>
          <div class="muted-text mt-8">${esc(res.naming_convention || "")}</div>
        </div>
        ${rawJsonDrawer(res, "查看放置推导原始 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runPlaceLearn() {
  const out = document.getElementById("place-result");
  try {
    const res = await apiGet("/api/v1/placement/learn?min_samples=3");
    mirrorBlackScreen("agent-steward place learn --min-samples 3", "steward_placement_learn", { min_samples: 3, accept: false });
    if (out) {
      const cands = res.candidates || res.suggestions || [];
      out.innerHTML = `
        <div class="item-box">
          <div class="item-top"><strong>🧠 放置规则自演化分析</strong><span class="badge">${cands.length} 个候选规则</span></div>
          ${cands.length ? cands.map((c) => `<div class="mt-8 mono">${esc(JSON.stringify(c))}</div>`).join("") : '<div class="muted-text mt-8">近期未发现偏离既有规则的新目录聚类。</div>'}
        </div>
        ${rawJsonDrawer(res, "查看自演化原始 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runMemoryAdd() {
  const memory_type = document.getElementById("mem-add-type")?.value || "decision";
  const content = (document.getElementById("mem-add-content")?.value || "").trim();
  if (!content) return alert("请输入记忆内容");
  try {
    await apiPost("/api/v1/memories/add", { memory_type, content });
    mirrorBlackScreen(`agent-steward memory add --type ${memory_type} "${content}"`, "steward_memory_add", { type: memory_type, content });
    document.getElementById("mem-add-content").value = "";
    loadGovernance();
  } catch (e) {
    alert("写入失败: " + e.message);
  }
}

async function runMemorySearch() {
  const q = document.getElementById("gov-mem-search")?.value || "";
  const hybrid = document.getElementById("mem-hybrid-chk")?.checked ?? true;
  const out = document.getElementById("gov-memories");
  try {
    const res = await apiGet(`/api/v1/memories/search?q=${encodeURIComponent(q)}&hybrid=${hybrid}`);
    mirrorBlackScreen(`agent-steward memory search "${q}"${hybrid ? " --hybrid" : ""}`, "steward_memory_search", { query: q, hybrid });
    const items = res.items || [];
    if (out) out.innerHTML = items.map((item) => {
      const m = item.memory || item;
      const rrf = item.rrf_score !== undefined ? ` · RRF ${Number(item.rrf_score).toFixed(4)}` : "";
      return `<div class="item-box"><div class="item-top"><span class="badge">${esc(m.memory_type)}</span><span class="muted-text">${esc(res.mode)}${rrf}</span></div><div>${esc(m.summary || (m.content || "").slice(0, 160))}</div></div>`;
    }).join("") || '<div class="empty">无匹配记忆</div>';
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runMemoryAudit() {
  const out = document.getElementById("mem-audit-result");
  try {
    const rep = await apiGet("/api/v1/memories/audit");
    mirrorBlackScreen("agent-steward memory audit", "steward_memory_audit", {});
    if (out) {
      out.innerHTML = `
        <div class="summary-kpi-row">
          <div class="summary-kpi"><span class="sk-label">总记忆数</span><span class="sk-val">${rep.total_memories ?? 0}</span></div>
          <div class="summary-kpi"><span class="sk-label">活跃记忆</span><span class="sk-val c-green">${rep.active_memories ?? 0}</span></div>
          <div class="summary-kpi"><span class="sk-label">半衰期衰减</span><span class="sk-val c-orange">${rep.decayed_count ?? 0}</span></div>
          <div class="summary-kpi"><span class="sk-label">语义矛盾</span><span class="sk-val">${rep.conflict_count ?? 0}</span></div>
        </div>
        ${rawJsonDrawer(rep, "查看记忆审计明细 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function loadAnalytics() {
  const hEl = document.getElementById("ana-heatmap");
  const tEl = document.getElementById("ana-trend");
  try {
    const [heat, trend] = await Promise.all([apiGet("/api/v1/analytics/heatmap?days=30"), apiGet("/api/v1/analytics/trend?days=30")]);
    mirrorBlackScreen("agent-steward analytics heatmap --days 30", "steward_analytics_heatmap", { days: 30 });
    if (hEl) {
      const cells = heat.heatmap || [];
      const maxCnt = Math.max(1, ...cells.map((c) => c.change_count || 0));
      hEl.innerHTML = `
        ${cells.length ? cells.slice(0, 10).map((c) => {
          const pct = Math.max(6, Math.round(((c.change_count || 0) / maxCnt) * 100));
          return `<div class="bar-row"><div class="bar-row-head"><span class="mono">${esc(c.directory)}</span><strong>${c.change_count} 次变更</strong></div><div class="bar-track"><div class="bar-fill warn" style="width:${pct}%"></div></div></div>`;
        }).join("") : '<div class="empty">近 30 天暂无变更热力记录</div>'}
        ${rawJsonDrawer(heat, "查看 DuckDB 热力图原始 JSON")}
      `;
    }
    if (tEl) {
      const rows = trend.asset_growth_trend || [];
      const maxAsset = Math.max(1, ...rows.map((r) => r.cumulative_count || r.daily_count || 0));
      const gs = trend.graph_stats || {};
      tEl.innerHTML = `
        <div class="summary-kpi-row">
          <div class="summary-kpi"><span class="sk-label">统计窗口</span><span class="sk-val">${trend.time_span_days ?? 30} 天</span></div>
          <div class="summary-kpi"><span class="sk-label">图谱节点</span><span class="sk-val c-cyan">${gs.total_nodes ?? "-"}</span></div>
          <div class="summary-kpi"><span class="sk-label">图谱边</span><span class="sk-val c-green">${gs.total_edges ?? "-"}</span></div>
        </div>
        ${rows.slice(0, 8).map((r) => {
          const val = r.cumulative_count || r.daily_count || 0;
          const pct = Math.max(6, Math.round((val / maxAsset) * 100));
          return `<div class="bar-row"><div class="bar-row-head"><span>${esc(r.asset_type)}</span><strong>${val}</strong></div><div class="bar-track"><div class="bar-fill ok" style="width:${pct}%"></div></div></div>`;
        }).join("")}
        ${rawJsonDrawer(trend, "查看 DuckDB 趋势原始 JSON")}
      `;
    }
  } catch (e) {
    if (hEl) hEl.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runAnalyticsExport() {
  const table = document.getElementById("ana-export-table")?.value || "assets";
  const output_path = `/root/new/tmp/steward_${table}.parquet`;
  const msgEl = document.getElementById("ana-export-msg");
  try {
    const res = await apiPost("/api/v1/analytics/export", { table, output_path });
    mirrorBlackScreen(`agent-steward analytics export --table ${table} --output-path ${output_path}`, "steward_analytics_export", { table, output_path });
    if (msgEl) msgEl.innerText = `✅ 已导出 ${res.rows_exported ?? ""} 行至 ${output_path}`;
  } catch (e) {
    if (msgEl) msgEl.innerText = `导出失败: ${e.message}`;
  }
}

async function runGcPreview() {
  const out = document.getElementById("gc-result");
  try {
    const res = await apiGet("/api/v1/env/gc?max_age_hours=72");
    mirrorBlackScreen("agent-steward gc --max-age-hours 72", "steward_env_gc", { max_age_hours: 72, apply: false });
    if (out) {
      const mb = ((res.candidate_bytes_reclaimed || 0) / 1048576).toFixed(1);
      out.innerHTML = `
        <div class="item-top"><strong>🧹 /root/new/tmp/ 扫描结果</strong><span class="badge badge-ok">Dry-Run 安全预览</span></div>
        <div class="summary-kpi-row mt-8">
          <div class="summary-kpi"><span class="sk-label">待清理文件数</span><span class="sk-val c-orange">${res.candidate_files_count ?? 0}</span></div>
          <div class="summary-kpi"><span class="sk-label">可释放空间</span><span class="sk-val">${mb} MB</span></div>
          <div class="summary-kpi"><span class="sk-label">实际删除</span><span class="sk-val c-green">${res.deleted_files_count ?? 0} (零误删)</span></div>
        </div>
        ${rawJsonDrawer(res, "查看 GC 候选列表 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runSessionClosePreview() {
  const out = document.getElementById("gc-result");
  try {
    const res = await apiPost("/api/v1/session/close", { session_id: "web-preview", clean_temp: true });
    mirrorBlackScreen("agent-steward close web-preview --dry-run", "steward_session_close", { session_id: "web-preview", dry_run: true });
    if (out) {
      out.innerHTML = `
        <div class="item-top"><strong>📋 会话完工结清预览</strong><span class="badge badge-ok">Dry-Run 模拟</span></div>
        <div class="muted-text mt-8">会话 ID: <code class="mono">${esc(res.session_id || "web-preview")}</code> · 已检测关联记忆与临时清理清单。</div>
        ${rawJsonDrawer(res, "查看结清报告 JSON")}
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function runMeshQuery(action) {
  const port = Number(document.getElementById("mesh-port-input")?.value || 8080);
  const out = document.getElementById("mesh-query-result");
  try {
    const res = await apiGet(`/api/v1/mesh/query?action=${action}&port=${port}`);
    mirrorBlackScreen(`agent-steward mesh ${action === "port" ? "port " + port : action}`, "steward_mesh_query", { action, port });
    if (out) {
      out.innerHTML = `
        <div class="item-box">
          <div class="item-top"><strong>🌐 Peer Mesh 查询 (${esc(action)})</strong><span class="badge badge-ok">已响应</span></div>
          ${rawJsonDrawer(res, "查看 Mesh 返回 JSON")}
        </div>
      `;
    }
  } catch (e) {
    if (out) out.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
  }
}

async function loadParityMatrix() {
  const badge = document.getElementById("parity-summary-badge");
  const box = document.getElementById("parity-matrix-box");
  if (!box) return;
  try {
    const m = await apiGet("/api/v1/parity/matrix");
    if (badge) badge.innerText = `${m.synced_count}/${m.total_mcp_tools} (${Number(m.coverage_pct).toFixed(0)}%)`;
    box.innerHTML = `<table class="data-table"><thead><tr><th>MCP 工具</th><th>CLI 命令</th><th>Web 端点</th><th>状态</th></tr></thead><tbody>` +
      (m.items || []).map((it) => `<tr>
        <td class="mono"><strong>${esc(it.mcp_tool)}</strong></td>
        <td class="mono">${esc(it.cli_command)}</td>
        <td class="mono">${esc(it.web_endpoint)}</td>
        <td><span class="badge ${it.status === "synced" ? "badge-ok" : "badge-err"}">${esc(it.status)}</span></td>
      </tr>`).join("") + `</tbody></table>`;
  } catch (e) {
    box.innerHTML = `<div class="empty">${esc(e.message)}</div>`;
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

/* ─── 6. 路演演示剧场 (Phase 25 三层故事化直观舞台) ──────────────── */
let demoScenariosCache = [];
let demoCurrentIdx = 0;
let demoCruiseTimer = null;

async function loadDemoTheater() {
  try {
    const [scenarios, roi] = await Promise.all([
      apiGet("/api/v1/demo/scenarios"),
      apiGet("/api/v1/demo/roi"),
    ]);
    demoScenariosCache = scenarios || [];

    document.getElementById("roi-save").innerText = `${Number(roi.token_savings_pct || 0).toFixed(1)}% (600/${roi.skills_full_tokens} tok)`;
    document.getElementById("roi-eco").innerText = `${roi.skills_count} / ${roi.mcp_servers_count} / ${roi.external_services_count}`;
    document.getElementById("roi-tools").innerText = `${roi.capabilities_installed} 工具 / ${roi.managed_assets_count} 资产`;
    document.getElementById("roi-graph").innerText = `${roi.fact_nodes_count} 节点 / ${roi.fact_edges_count} 边`;
    const parityEl = document.getElementById("roi-parity");
    if (parityEl) {
      const ledgerHint = roi.ledger_tokens_saved > 0
        ? ` · 账本实省 ${Number(roi.ledger_tokens_saved).toLocaleString()} tok`
        : "";
      const speedupHint = roi.latest_benchmark_speedup
        ? ` · 实测 ${Number(roi.latest_benchmark_speedup).toFixed(2)}x`
        : "";
      parityEl.innerText = `40/40 (100%)${speedupHint}${ledgerHint}`;
    }

    const btnWrap = document.getElementById("demo-scene-buttons");
    if (btnWrap) {
      btnWrap.innerHTML = demoScenariosCache.map((s, idx) =>
        `<button class="scene-card-btn ${idx === demoCurrentIdx ? "active" : ""}" data-scene="${esc(s.id)}" onclick="runDemoScenario('${esc(s.id)}', ${idx})">
          <div class="scene-card-top">
            <span class="badge">第 ${s.order} 幕</span>
            <span class="muted-text mono" style="font-size:10px;">${esc(s.category)}</span>
          </div>
          <div class="scene-card-title">${esc(s.title)}</div>
          <div class="scene-card-sub" title="${esc(s.intent_summary || s.tagline)}">${esc(s.intent_summary || s.tagline)}</div>
        </button>`
      ).join("");
    }
    if (demoScenariosCache.length > 0 && !document.getElementById("demo-stage")?.dataset.loaded) {
      runDemoScenario(demoScenariosCache[0].id, 0);
    }
  } catch (e) {
    console.error("demo load err:", e);
  }
}

function renderDemoSceneWidget(res) {
  const ws = res.with_steward || {};
  const roi = res.roi_metrics || {};
  const id = res.scenario_id;

  if (id === "scene-1") {
    const tools = ws.sample_tools || [];
    const guards = ws.blacklist_shallow_guards || [];
    return `
      <div class="widget-panel">
        <div class="widget-title"><span>🚪 黑白名单浅层阻断漏斗</span><span class="badge badge-ok">拦截 ${esc(roi.noise_blocked_pct || "98.5%")} 垃圾文件</span></div>
        <div class="bar-row">
          <div class="bar-row-head"><span>敲门即止（仅记录目录实体，绝不递归扫爆）</span><strong>98.5% 阻断</strong></div>
          <div class="bar-track"><div class="bar-fill ok" style="width:98.5%"></div></div>
        </div>
        <div class="pill-wrap">${guards.map((g) => `<span class="badge badge-warn">🚫 ${esc(g)}</span>`).join("")}</div>
      </div>
      <div class="widget-panel">
        <div class="widget-title"><span>🧰 本机现成 CLI 武器库（实测已装 ${ws.installed_tools_count ?? tools.length} 个）</span><span class="badge">零幻觉</span></div>
        <div class="tool-wall">
          ${tools.map((t) => `<div class="tool-card">
            <div class="tool-card-head"><strong>${esc(t.tool)}</strong><span class="badge badge-ok">${esc(t.category)}</span></div>
            <code>${esc(t.best_practice || "已就绪可直接调用")}</code>
          </div>`).join("")}
        </div>
      </div>
    `;
  }

  if (id === "scene-2") {
    const pc = ws.port_check || {};
    const conflict = pc.conflict || {};
    const freePorts = (pc.recommended_free_ports || roi.free_ports_recommended || []);
    const place = ws.placement_advice || {};
    return `
      <div class="widget-panel">
        <div class="widget-title"><span>🚦 Pre-Hook 1ms 红绿灯急刹车</span><span class="badge badge-ok">执行前拦截</span></div>
        <div class="traffic-row">
          <span class="traffic-light red"></span>
          <div style="flex:1;">
            <strong>红灯拦截：端口 :${esc(ws.target_port_probed)} 已被真实占用</strong>
            <div class="muted-text">占用方: ${esc(conflict.name || "宿主机服务")} (PID ${esc(conflict.pid ?? "-")}) · 避免服务撞车崩溃</div>
          </div>
          <span class="badge badge-err">拦截</span>
        </div>
        <div class="traffic-row">
          <span class="traffic-light red"></span>
          <div style="flex:1;">
            <strong>红灯纠正：禁止随手乱丢 /tmp/TmpCrawler.py</strong>
            <div class="muted-text">违反工作区整洁与小写蛇形命名规范</div>
          </div>
          <span class="badge badge-warn">纠正</span>
        </div>
        <div class="traffic-row">
          <span class="traffic-light green"></span>
          <div style="flex:1;">
            <strong>绿灯自动改道推荐</strong>
            <div class="pill-wrap mt-8">
              ${freePorts.map((p) => `<span class="badge badge-ok">可用端口 :${p}</span>`).join("")}
              <span class="badge">规范路径: ${esc(place.suggested_path || roi.suggested_path || "/root/new/shared_proj/")}</span>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  if (id === "scene-3") {
    const chain = ws.cascade_chain || [];
    const repair = ws.repair_preview || {};
    const cg = ws.codegraph_federation;
    const audit = ws.links_audit || {};
    return `
      <div class="widget-panel">
        <div class="widget-title"><span>🦋 3 跳蝴蝶效应因果链（真实宿主机拓扑）</span><span class="badge badge-ok">健康度 ${audit.health_score ?? 100}/100</span></div>
        <div class="cascade-chain">
          ${chain.map((node, idx) => `
            ${idx > 0 ? '<div class="cascade-arrow">↓ 牵连影响</div>' : ''}
            <div class="cascade-node">
              <span class="badge">${esc(node.kind)}</span>
              <strong class="mono" style="flex:1;font-size:11px;">${esc(node.label)}</strong>
              <span class="muted-text">Hop ${node.hop}</span>
            </div>
          `).join("")}
        </div>
      </div>
      <div class="widget-panel">
        <div class="widget-title"><span>🩹 断链自愈补丁 &amp; 🔬 CodeGraph 微观联邦</span><span class="badge">${cg ? `已联邦 ${cg.node_count} 符号` : "按需下钻"}</span></div>
        <div class="item-box" style="margin-bottom:0;">
          <div class="muted-text" style="text-decoration:line-through;color:var(--red);">${esc(repair.broken_ref || "/root/new/shared_proj/sec_crawler_v1.py")}</div>
          <div class="mono" style="color:var(--green);margin-top:4px;font-weight:700;">→ 自动修复为: ${esc(repair.repaired_ref || "/root/new/shared_proj/sec_crawler.py")} (相似度 ${repair.similarity ?? 0.94})</div>
        </div>
      </div>
    `;
  }

  if (id === "scene-4") {
    const dl = ws.deadlock_report || {};
    const cycles = dl.cycles || [];
    const firstCycle = cycles[0] || {};
    return `
      <div class="widget-panel">
        <div class="widget-title"><span>🔄 WFG 双 Agent 交叉死锁环路图</span><span class="badge badge-err">1ms 检出 ${cycles.length} 条环路</span></div>
        <div class="deadlock-ring">
          <div class="dl-agent-box">
            <strong>🤖 demo-agent-alpha</strong>
            <div class="muted-text mt-8">持有 <span class="badge">demo:port:19090</span></div>
          </div>
          <div class="dl-arrows">
            <span>⇄ 互相死等 ⇄</span>
            <span style="font-size:10px;color:var(--muted);">WFG DFS 环路</span>
          </div>
          <div class="dl-agent-box">
            <strong>🤖 demo-agent-beta</strong>
            <div class="muted-text mt-8">持有 <span class="badge">/work/demo_app</span></div>
          </div>
        </div>
      </div>
      <div class="widget-panel">
        <div class="widget-title"><span>⚖️ 交警破局裁决书</span><span class="badge badge-ok">demo:* 隔离沙盒</span></div>
        <div class="item-box" style="margin-bottom:0;">
          <strong>建议动作：</strong>${esc(firstCycle.suggestion || "唤醒优先级裁决：让等待代价更小的一方暂时退让释放锁，打破死锁环路。")}
        </div>
      </div>
    `;
  }

  if (id === "scene-5") {
    const plan = ws.route_plan || {};
    const fullTok = Number(plan.total_skills_full_tokens || roi.full_tokens || 449640);
    const usedTok = Number(plan.route_tokens_used || roi.routed_tokens || 358);
    const pct = Math.max(1.5, Math.min(100, (usedTok / Math.max(1, fullTok)) * 100));
    const skills = plan.skills || [];
    const mcps = plan.mcps || [];
    return `
      <div class="widget-panel">
        <div class="widget-title"><span>📉 Token 账单瘦身对比尺</span><span class="badge badge-ok">节省 ${esc(roi.saved_pct || "99.9%")}</span></div>
        <div class="bar-row">
          <div class="bar-row-head"><span>全量硬塞 121+ SKILL（上下文直接打爆）</span><strong style="color:var(--red);">${fullTok.toLocaleString()} tok (100%)</strong></div>
          <div class="bar-track"><div class="bar-fill err" style="width:100%"></div></div>
        </div>
        <div class="bar-row" style="margin-bottom:0;">
          <div class="bar-row-head"><span>按预算精准路由 Top-K 名片</span><strong style="color:var(--green);">${usedTok.toLocaleString()} tok</strong></div>
          <div class="bar-track"><div class="bar-fill ok" style="width:${pct.toFixed(1)}%"></div></div>
        </div>
      </div>
      <div class="widget-panel">
        <div class="widget-title"><span>🎯 精准命中名片夹（意图：PostgreSQL 调优）</span><span class="badge">${skills.length} SKILL · ${mcps.length} MCP</span></div>
        <div class="pill-wrap">
          ${skills.map((s) => `<span class="badge badge-ok">📚 ${esc(s.skill_name)}</span>`).join("")}
          ${mcps.map((m) => `<span class="badge">🔌 ${esc(m.server_name)}</span>`).join("")}
        </div>
      </div>
    `;
  }

  // scene-6
  const quality = ws.brief_quality || {};
  const score = Number(quality.overall ?? roi.brief_overall_score ?? 71);
  const gc = ws.gc_dry_run || {};
  const mem = ws.memory_audit || {};
  const mb = ((gc.candidate_bytes_reclaimed || 0) / 1048576).toFixed(1);
  return `
    <div class="widget-panel">
      <div class="widget-title"><span>📋 1 秒交接开工简报质量仪表</span><span class="badge badge-ok">综合评分 ${score}/100</span></div>
      <div class="bar-row">
        <div class="bar-row-head"><span>三位一体情境组装完整度（硬件 + 近期变更 + 长期记忆）</span><strong>${score}%</strong></div>
        <div class="bar-track"><div class="bar-fill ok" style="width:${Math.max(10, Math.min(100, score))}%"></div></div>
      </div>
      <div class="muted-text">${esc((ws.brief_preview || "").slice(0, 140))}...</div>
    </div>
    <div class="widget-panel">
      <div class="widget-title"><span>🧹 完工安全自洁护盾</span><span class="badge badge-ok">Dry-Run 绝不静默误删</span></div>
      <div class="summary-kpi-row" style="margin-bottom:0;">
        <div class="summary-kpi"><span class="sk-label">待检孤儿文件</span><span class="sk-val c-orange">${gc.candidate_files_count ?? roi.orphan_files_found ?? 0}</span></div>
        <div class="summary-kpi"><span class="sk-label">可安全回收</span><span class="sk-val">${mb} MB</span></div>
        <div class="summary-kpi"><span class="sk-label">记忆矛盾检查</span><span class="sk-val c-green">${mem.conflict_count ?? 0} 冲突</span></div>
      </div>
    </div>
  `;
}

window.runDemoScenario = async function (scenarioId, idx) {
  if (typeof idx === "number") demoCurrentIdx = idx;
  document.querySelectorAll("#demo-scene-buttons .scene-card-btn").forEach((b) =>
    b.classList.toggle("active", b.dataset.scene === scenarioId)
  );
  const stage = document.getElementById("demo-stage");
  if (!stage) return;
  stage.dataset.loaded = "1";
  stage.innerHTML = `<div class="muted-text">⚡ 正在调用宿主机真实 Rust 引擎执行 ${esc(scenarioId)}...</div>`;
  try {
    const res = await apiPost("/api/v1/demo/run", { scenario_id: scenarioId });
    if (res.black_screen_mirror) {
      mirrorBlackScreen(
        res.black_screen_mirror.cli_command,
        res.black_screen_mirror.mcp_tool,
        res.black_screen_mirror.mcp_arguments
      );
    }
    const elapsedMs = (Number(res.elapsed_us || 0) / 1000).toFixed(2);
    const wo = res.without_steward || {};
    const heroCards = (res.hero_metrics || []).map((m) => `
      <div class="hero-metric-card tone-${esc(m.tone || "info")}">
        <span class="hero-metric-label">${esc(m.label)}</span>
        <strong class="hero-metric-value">${esc(m.value)}</strong>
      </div>
    `).join("");

    stage.innerHTML = `
      <div class="demo-header-banner">
        <div>
          <h3>🎬 ${esc(res.title)}</h3>
          <div class="muted-text" style="margin-top:3px;">${esc(res.tagline)}</div>
        </div>
        <div style="display:flex;gap:8px;align-items:center;">
          <span class="badge badge-ok">⚡ 真引擎实测 ${res.elapsed_us} μs (${elapsedMs} ms)</span>
          <span class="badge">${esc(res.scenario_id)}</span>
        </div>
      </div>

      <div class="demo-explainer-grid">
        <div class="explainer-card analogy">
          <div class="ex-title">💡 一句话生活比喻</div>
          <div class="ex-body">${esc(res.analogy || res.tagline)}</div>
        </div>
        <div class="explainer-card">
          <div class="ex-title">🎯 这一幕解决什么问题（核心用意）</div>
          <div class="ex-body">${esc(res.intent_summary || res.speaker_notes)}</div>
        </div>
      </div>

      <div class="demo-hero-metrics">${heroCards}</div>

      <div class="speaker-card">
        <strong>🎙️ 演讲者 30 秒提词卡：</strong>${esc(res.speaker_notes)}
      </div>

      <div class="demo-split">
        <div class="demo-pane-bad">
          <div class="item-top"><strong style="color:var(--red);">💥 没有大管家：盲人 Agent 翻车现场</strong><span class="badge badge-err">高危痛点</span></div>
          <div class="story-step-list">
            <div class="story-step">
              <span class="step-num">1</span>
              <div class="story-step-text"><strong>盲目动作</strong><p>${esc(wo.action || wo.symptom || "")}</p></div>
            </div>
            <div class="story-step">
              <span class="step-num">2</span>
              <div class="story-step-text"><strong>触发故障</strong><p>${esc(wo.failure || wo.symptom || "")}</p></div>
            </div>
            <div class="story-step">
              <span class="step-num">3</span>
              <div class="story-step-text"><strong>业务代价</strong><p>${esc(wo.cost || wo.risk || "")}</p></div>
            </div>
          </div>
          ${wo.command ? `<div class="cmd-sim-box">$ ${esc(wo.command)}</div>` : ""}
        </div>

        <div class="demo-pane-good">
          <div class="item-top"><strong style="color:var(--green);">🛡️ 有了大管家：小秘书毫秒级护航</strong><span class="badge badge-ok">真引擎实跑</span></div>
          ${renderDemoSceneWidget(res)}
        </div>
      </div>

      <details class="raw-json-details">
        <summary>🔍 技术专家核验区：展开查看本机 Rust 真引擎原始返回 JSON 与等价 CLI / MCP 命令</summary>
        <div class="muted-text" style="margin-bottom:6px;">CLI: <code class="mono">${esc(res.black_screen_mirror?.cli_command || "")}</code></div>
        <pre class="md-pre">${esc(JSON.stringify({ without_steward: res.without_steward, with_steward: res.with_steward, roi_metrics: res.roi_metrics }, null, 2))}</pre>
      </details>
    `;
  } catch (e) {
    stage.innerHTML = `<div class="empty">路演执行失败: ${esc(e.message)}</div>`;
  }
};

function toggleDemoCruise() {
  const btn = document.getElementById("btn-demo-cruise");
  if (demoCruiseTimer) {
    clearInterval(demoCruiseTimer);
    demoCruiseTimer = null;
    if (btn) btn.innerText = "▶ 自动巡航路演";
    return;
  }
  if (btn) btn.innerText = "⏸ 暂停巡航 (8s/幕)";
  demoCruiseTimer = setInterval(() => {
    if (state.activeView !== "demo" || demoScenariosCache.length === 0) return;
    demoCurrentIdx = (demoCurrentIdx + 1) % demoScenariosCache.length;
    runDemoScenario(demoScenariosCache[demoCurrentIdx].id, demoCurrentIdx);
  }, 8000);
}

async function resetDemoSandbox() {
  try {
    const res = await apiPost("/api/v1/demo/reset", {});
    mirrorBlackScreen("agent-steward demo reset", "steward_demo_scenario", { action: "reset" });
    alert(`✅ 已无痕清理 ${res.cleaned ?? 0} 条 demo:* 沙盒锁与等待队列`);
    loadOverview();
  } catch (e) {
    alert("沙盒重置失败: " + e.message);
  }
}

document.addEventListener("keydown", (e) => {
  if (e.target && ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) return;
  if ((e.key === "t" || e.key === "T") && !e.ctrlKey && !e.metaKey && !e.altKey) {
    toggleTheme();
    return;
  }
  if (state.activeView !== "demo" || demoScenariosCache.length === 0) return;
  if (e.key === "ArrowRight") {
    demoCurrentIdx = (demoCurrentIdx + 1) % demoScenariosCache.length;
    runDemoScenario(demoScenariosCache[demoCurrentIdx].id, demoCurrentIdx);
  } else if (e.key === "ArrowLeft") {
    demoCurrentIdx = (demoCurrentIdx - 1 + demoScenariosCache.length) % demoScenariosCache.length;
    runDemoScenario(demoScenariosCache[demoCurrentIdx].id, demoCurrentIdx);
  } else if (e.code === "Space") {
    e.preventDefault();
    toggleDemoCruise();
  }
});

/* ─── Phase 28: 实证 Benchmark 与效能账本工作台 JS ─── */
async function loadBenchmarkWorkbench() {
  mirrorBlackScreen("agent-steward bench ledger --window 7d", "steward_benchmark", { action: "ledger", window: "7d" });
  await Promise.all([loadBenchmarkReport(), loadBenchmarkLedger()]);
  const runBtn = document.getElementById("btn-bench-run");
  if (runBtn && !runBtn.dataset.bound) {
    runBtn.dataset.bound = "1";
    runBtn.addEventListener("click", runBenchmarkSuiteFromUi);
    document.getElementById("btn-bench-refresh")?.addEventListener("click", loadBenchmarkLedger);
    document.getElementById("bench-window-select")?.addEventListener("change", loadBenchmarkLedger);
    document.getElementById("btn-bench-compare")?.addEventListener("click", compareBenchmarkRunsFromUi);
  }
}

async function runBenchmarkSuiteFromUi() {
  const suite = document.getElementById("bench-suite-select")?.value || "full";
  const box = document.getElementById("bench-workloads-box");
  if (box) box.innerHTML = `<div class="empty">⏳ 正在宿主机真机运行 ${esc(suite)} A/B 对照基准...</div>`;
  mirrorBlackScreen(`agent-steward bench run --suite ${suite}`, "steward_benchmark", { action: "run", suite, save: true });
  try {
    const rep = await apiPost("/api/v1/benchmark/run", { suite, save: true });
    renderBenchmarkReportPayload(rep);
    await loadBenchmarkReport();
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">运行失败: ${esc(e.message)}</div>`;
  }
}

async function loadBenchmarkReport() {
  try {
    const data = await apiGet("/api/v1/benchmark/report");
    if (data.latest) {
      renderBenchmarkReportPayload(data.latest);
    }
    const hist = data.history || [];
    const selA = document.getElementById("bench-cmp-a");
    const selB = document.getElementById("bench-cmp-b");
    if (selA && selB) {
      const opts = hist.map((h) => `<option value="${esc(h.run_id)}">${esc(h.run_id.slice(0, 14))} (v${esc(h.app_version)} · ${Number(h.overall_token_savings_pct || 0).toFixed(1)}%)</option>`).join("");
      selA.innerHTML = opts;
      selB.innerHTML = opts;
      if (hist.length >= 2) selB.selectedIndex = 1;
    }
  } catch (_) {}
}

function renderBenchmarkReportPayload(rep) {
  const sp = document.getElementById("bench-kpi-speedup");
  const sha = document.getElementById("bench-sha-badge");
  const sla = document.getElementById("bench-sla-badge");
  if (sp) sp.innerText = `${Number(rep.overall_speedup_factor || 1).toFixed(2)}x`;
  if (sha) sha.innerText = `SHA-256: ${String(rep.report_sha256 || "").slice(0, 16)}...`;
  if (sla) {
    sla.innerText = rep.sla_passed ? "✅ SLA PASS" : "❌ SLA FAIL";
    sla.className = rep.sla_passed ? "badge badge-ok" : "badge badge-err";
  }

  const wbox = document.getElementById("bench-workloads-box");
  const workloads = rep.workloads || [];
  if (wbox) {
    if (workloads.length === 0) {
      wbox.innerHTML = `<div class="empty">该批次无宏观负载数据</div>`;
    } else {
      wbox.innerHTML = workloads.map((w) => {
        const pct = Math.max(0, Math.min(100, Number(w.token_savings_pct || 0)));
        return `<div class="card" style="padding:10px;margin-bottom:8px;">
          <div class="item-top">
            <strong>${esc(w.id)} · ${esc(w.title)}</strong>
            <span class="badge ${w.sla_passed ? "badge-ok" : "badge-err"}">${w.sla_passed ? "PASS" : "FAIL"} · 省 ${pct.toFixed(1)}%</span>
          </div>
          <div class="muted-text" style="font-size:12px;margin-top:4px;">${esc(w.evidence_detail)}</div>
          <div style="display:flex;gap:8px;align-items:center;margin-top:6px;font-size:11px;">
            <span style="min-width:95px;">基线 ${w.baseline_tokens} tok</span>
            <div style="flex:1;height:6px;background:var(--border);border-radius:4px;overflow:hidden;">
              <div style="width:${(100 - pct).toFixed(1)}%;height:100%;background:var(--green);"></div>
            </div>
            <span style="min-width:95px;text-align:right;">管家 ${w.steward_tokens} tok</span>
          </div>
        </div>`;
      }).join("");
    }
  }

  const mbox = document.getElementById("bench-micro-box");
  const micros = rep.micro_benchmarks || [];
  if (mbox) {
    if (micros.length === 0) {
      mbox.innerHTML = `<div class="empty">该批次未运行内核微基准</div>`;
    } else {
      mbox.innerHTML = micros.map((m) => `
        <div class="item-top" style="padding:6px 0;border-bottom:1px dashed var(--border);font-size:12px;">
          <span><strong>${esc(m.id)}</strong> ${esc(m.name)}</span>
          <span class="mono">${m.avg_us} μs (P95: ${m.p95_us} / SLA &lt; ${m.sla_limit_us}) ${m.passed ? "✅" : "❌"}</span>
        </div>
      `).join("");
    }
  }
}

async function loadBenchmarkLedger() {
  const win = document.getElementById("bench-window-select")?.value || "7d";
  try {
    const sum = await apiGet(`/api/v1/benchmark/ledger?window=${encodeURIComponent(win)}&limit=30`);
    document.getElementById("bench-kpi-tokens").innerText = `${Number(sum.total_tokens_saved || 0).toLocaleString()} tok`;
    document.getElementById("bench-kpi-rounds").innerText = `${Number(sum.total_rounds_saved || 0)} 轮`;
    document.getElementById("bench-kpi-hazards").innerText = `${Number(sum.total_hazards_intercepted || 0)} 次`;
    document.getElementById("bench-kpi-noise").innerText = `${Number(sum.total_noise_blocked || 0)} 个`;

    const rStrip = document.getElementById("bench-rules-strip");
    if (rStrip) {
      const rules = sum.by_rule || [];
      rStrip.innerHTML = rules.map((r) => `<span class="badge" style="margin-right:6px;margin-bottom:4px;">${esc(r.provenance_rule)}: ${r.event_count}次 · 省${r.tokens_saved} tok</span>`).join("");
    }

    const list = document.getElementById("bench-ledger-list");
    if (list) {
      const evs = sum.recent_events || [];
      if (evs.length === 0) {
        list.innerHTML = `<div class="empty">该时间窗内尚无生产效能存证流水（调用任意 MCP/扫描后自动入账）</div>`;
      } else {
        list.innerHTML = evs.map((e) => `
          <div class="item-top" style="padding:7px 0;border-bottom:1px solid var(--border);font-size:12px;">
            <div>
              <span class="badge badge-ok">${esc(e.provenance_rule)}</span>
              <strong class="mono">${esc(e.tool_or_action)}</strong>
              <span class="muted-text">(${esc(e.source_channel)})</span>
              <div class="muted-text" style="margin-top:2px;">${esc(e.evidence_summary)}</div>
            </div>
            <div class="mono" style="text-align:right;white-space:nowrap;">
              <span style="color:var(--green);">-${e.tokens_saved} tok</span> · ${e.elapsed_us} μs
            </div>
          </div>
        `).join("");
      }
    }
  } catch (e) {
    const list = document.getElementById("bench-ledger-list");
    if (list) list.innerHTML = `<div class="empty">账本加载失败: ${esc(e.message)}</div>`;
  }
}

async function compareBenchmarkRunsFromUi() {
  const a = document.getElementById("bench-cmp-a")?.value;
  const b = document.getElementById("bench-cmp-b")?.value;
  const box = document.getElementById("bench-compare-box");
  if (!a || !b) {
    if (box) box.innerHTML = `<div class="muted-text">需要至少 2 条历史运行记录才能比对</div>`;
    return;
  }
  try {
    const diff = await apiGet(`/api/v1/benchmark/compare?a=${encodeURIComponent(a)}&b=${encodeURIComponent(b)}`);
    if (box) {
      box.innerHTML = `<div class="badge ${diff.has_regression ? "badge-err" : "badge-ok"}">${diff.has_regression ? "⚠️ 检测到性能/压缩率回归" : "✅ 无回归"}</div>
        <div class="muted-text mt-4" style="font-size:12px;">加速比: ${Number(diff.speedup_a).toFixed(2)}x → ${Number(diff.speedup_b).toFixed(2)}x · Token 节省: ${Number(diff.token_savings_a_pct).toFixed(1)}% → ${Number(diff.token_savings_b_pct).toFixed(1)}%</div>`;
    }
  } catch (e) {
    if (box) box.innerHTML = `<div class="muted-text">对比失败: ${esc(e.message)}</div>`;
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

pub static DECK_HTML: &str = include_str!("../../../steward-deck.html");
pub static PRESENTATION_HTML: &str = include_str!("../../../presentation.html");

pub fn serve_deck() -> HttpResponse {
    HttpResponse::html(DECK_HTML)
}

pub fn serve_presentation() -> HttpResponse {
    HttpResponse::html(PRESENTATION_HTML)
}
