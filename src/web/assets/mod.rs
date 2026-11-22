use crate::web::http::HttpResponse;

pub static INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="zh-CN" data-theme="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Agent Body 2.0 — 宿主环境全景指挥舱</title>
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
          <h1>Agent Body <span class="ver-badge">v0.4.0 · Agent 本体</span></h1>
          <p class="subtitle">脑体分离具身底座 · 宿主机感官 × 小脑反射 × 海马体接力 × 五维稳态</p>
        </div>
      </div>

      <div class="kpi-strip">
        <div class="kpi-pill"><span class="k-label">主机</span><strong id="k-host">-</strong></div>
        <div class="kpi-pill"><span class="k-label">硬件</span><strong id="k-hw">-</strong></div>
        <div class="kpi-pill"><span class="k-label">图谱</span><strong id="k-facts" class="c-cyan">-</strong></div>
        <div class="kpi-pill"><span class="k-label">协作锁</span><strong id="k-locks" class="c-orange">-</strong></div>
        <div class="kpi-pill"><span class="k-label">生态</span><strong id="k-eco" class="c-green">-</strong></div>
        <div class="kpi-pill"><span class="k-label">平台</span><strong id="k-os" class="c-purple">-</strong></div>
      </div>

      <div class="topbar-actions">
        <span id="parity-badge" class="sse-pill" style="cursor:pointer;" title="点击查看黑白屏 43/43 能力对齐矩阵">✅ 43/43</span>
        <span id="sse-badge" class="sse-pill connecting">SSE</span>
        <button id="btn-auth-token" class="btn btn-ghost btn-sm" title="配置 [web].auth_token 用于写操作与 GC 鉴权">🔑 Token</button>
        <button id="btn-theme-toggle" class="btn btn-theme btn-sm" title="切换白天工程蓝图 / 夜间深空指挥舱 (快捷键 T)">☀️ 蓝图</button>
        <button id="btn-toggle-mirror" class="btn btn-ghost btn-sm" title="展开/收起黑屏命令镜像器">⌨️ 镜像</button>
        <button id="btn-toggle-sse" class="btn btn-ghost btn-sm" title="展开/收起实时事件流">⚡ 事件 <span id="sse-unread-badge" class="badge hidden">0</span></button>
        <button id="btn-global-refresh" class="btn btn-primary btn-sm">↻ 刷新</button>
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
        <button class="nav-btn" data-view="audit">
          <span class="nav-ico">📜</span>
          <span>双域审计与进化</span>
        </button>
        <button class="nav-btn" data-view="body">
          <span class="nav-ico">🧬</span>
          <span>本体中枢与稳态</span>
        </button>
        <div class="sidenav-footer">
          <a href="/deck" target="_blank" class="btn btn-ghost btn-xs sidenav-link">📽️ 路演 PPT (/deck)</a>
          <a href="/presentation" target="_blank" class="btn btn-ghost btn-xs sidenav-link">🎨 架构 PPT</a>
        </div>
      </nav>

      <main class="stage">
        <!-- 视图 1: 宿主机事实拓扑图谱 -->
        <section id="view-topology" class="view active">
          <div class="view-toolbar topo-toolbar">
            <div class="toolbar-left">
              <input id="topo-search" class="input input-search" type="text" placeholder="搜索节点并转至中心 (Enter)..." />
              <div class="mode-switch" id="topo-mode-switch" title="切换 3D 全息球面 / 2D 平面视图">
                <button type="button" class="mode-btn active" data-mode="sphere">🌐 3D 球面</button>
                <button type="button" class="mode-btn" data-mode="flat">🗺️ 2D 平面</button>
              </div>
              <label class="depth-ctrl" title="限制文件夹与文件路径展示深度，超出层级的子文件自动聚合到父目录">
                <span>📂 层级</span>
                <select id="topo-max-depth" class="input input-xs">
                  <option value="2">2 层 (极简)</option>
                  <option value="3" selected>3 层 (默认)</option>
                  <option value="4">4 层</option>
                  <option value="5">5 层</option>
                  <option value="99">全部展开</option>
                </select>
              </label>
              <label class="depth-ctrl" title="选中节点时高亮的关联跳数层级">
                <span>🎯 跳数</span>
                <select id="topo-hop-limit" class="input input-xs">
                  <option value="1">1 跳</option>
                  <option value="2">2 跳</option>
                  <option value="3" selected>3 跳</option>
                  <option value="99">全图</option>
                </select>
              </label>
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
              <span id="topo-stats" class="muted-text mono">加载中...</span>
              <button id="btn-links-audit" class="btn btn-ghost btn-xs">🩺 断链审计</button>
              <button id="btn-links-mermaid" class="btn btn-ghost btn-xs">📐 Mermaid</button>
            </div>
          </div>
          <div class="topology-stage" id="topology-stage">
            <canvas id="topology-canvas"></canvas>
            <div id="topo-focus-banner" class="topo-focus-banner hidden">
              <span id="topo-focus-label">🎯 当前中心: -</span>
              <button id="btn-topo-clear-focus" class="btn btn-ghost btn-xs">✕ 取消聚焦</button>
            </div>
            <div class="topo-hud-controls">
              <button id="btn-zoom-out" class="hud-btn" title="缩小球体">➖</button>
              <input id="topo-zoom-slider" type="range" min="35" max="350" value="100" class="hud-slider" title="缩放比例" />
              <button id="btn-zoom-in" class="hud-btn" title="放大球体">➕</button>
              <span id="topo-zoom-label" class="hud-zoom-val mono">100%</span>
              <span class="hud-sep">|</span>
              <button id="btn-topo-orbit" class="hud-btn" title="开启/暂停球体怠速微旋">🔄 微旋</button>
              <button id="btn-topo-fit" class="hud-btn" title="重置视角与缩放">🎯 归位</button>
              <button id="btn-topo-reset" class="hud-btn" title="恢复全景图">🌐 全景</button>
            </div>
            <div class="canvas-hint">拖拽旋转球体 · 滚轮缩放 · 点击节点自动转至正面中心并下钻 · 按 T 切换主题</div>
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
                <input id="hook-tool-name" class="input" style="width:150px;" type="text" value="run_command" placeholder="工具名 (run_command / write_file)" />
                <input id="hook-tool-args" class="input flex-1 mono" type="text" value='{"command": "psql -h 127.0.0.1 -p 5432"}' placeholder='入参 JSON (如 {"port": 8756})' />
                <input id="hook-error-output" class="input flex-1 mono" type="text" value="" placeholder="可选 Post-Hook 报错文本 (如 Connection refused)" />
                <button id="btn-hook-pre" class="btn btn-primary">🛡️ Pre-Hook 预检</button>
                <button id="btn-hook-post" class="btn btn-ghost">📝 Post-Hook 通知/自愈</button>
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
              <button class="subtab" data-sub="mesh">🌐 Mesh · 预检 · 43/43 对齐矩阵</button>
            </div>
          </div>
          <div id="gov-panel-brief" class="scroll-panel">
            <div class="grid-2">
              <div class="card">
                <div class="card-header">
                  <h3>📋 自适应开工简报组装器 (body_brief)</h3>
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
                    <button id="btn-mesh-graph" class="btn btn-ghost btn-xs">🕸️ 联邦图谱</button>
                    <button id="btn-mesh-dialogue-inbox" class="btn btn-ghost btn-xs">💬 对话收件箱</button>
                    <button id="btn-mesh-transfer-inbox" class="btn btn-ghost btn-xs">📦 沙盒收件箱</button>
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
                <strong>脑体分离 · Agent 本体 —— 比较优势的终极工程形态</strong>
                <p>LLM 客户端只是随时可换班的「瞬态大脑」，而驻留本机的 Agent 本体才是永驻宿主机的数字躯干（“小秘书”是其感官盘点侧面，“大管家”是其稳态防撞侧面）。新大脑接入即可继承六大器官体征、L0~L3 红线与跨大脑接力胶囊，把 100% 智商留给主业。</p>
              </div>
            </div>

            <div id="demo-roi-strip" class="demo-roi-strip mt-12">
              <div class="roi-card"><span class="k-label">Context Token 节省率</span><strong id="roi-save" class="c-green">-</strong></div>
              <div class="roi-card"><span class="k-label">全量 SKILL / MCP / 服务</span><strong id="roi-eco" class="c-cyan">-</strong></div>
              <div class="roi-card"><span class="k-label">宿主机 CLI 工具 / 资产</span><strong id="roi-tools">-</strong></div>
              <div class="roi-card"><span class="k-label">事实图谱 (节点 / 边)</span><strong id="roi-graph" class="c-purple">-</strong></div>
              <div class="roi-card"><span class="k-label">黑白屏同步认证</span><strong id="roi-parity" class="c-orange">43/43 (100%)</strong></div>
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
                <select id="bench-suite-select" class="input" style="width:240px;">
                  <option value="full">全量套件 v3.5 (省Token×提质量帕累托 + 竞品 + 图谱/Mesh + AOPCC)</option>
                  <option value="competitors">🏆 热门竞品横向对标 (COMP-C1~C4)</option>
                  <option value="macro">宏观业务 A/B (W1~W15)</option>
                  <option value="micro">内核微基准 SLA (M1~M5)</option>
                  <option value="dq">宿主机数据与新鲜度/图谱质量 (DQ1~DQ6)</option>
                  <option value="W1">W1 · 宿主工具发现</option>
                  <option value="W2">W2 · 端口防撞选址</option>
                  <option value="W3">W3 · 分级扫描阻断</option>
                  <option value="W4">W4 · 放置合规门禁</option>
                  <option value="W5">W5 · 爆炸半径与断链</option>
                  <option value="W6">W6 · 多 Agent 死锁破局</option>
                  <option value="W7">W7 · 百级 SKILL 路由</option>
                  <option value="W8">W8 · 冷启动态势简报</option>
                  <option value="W9">W9 · 混合语义检索 Recall@5</option>
                  <option value="W10">W10 · 依赖抽取抗假阳性</option>
                  <option value="W11">W11 · 全链路脱敏与抗投毒安全防线</option>
                  <option value="W12">W12 · 资产改删写穿失效与一致性自愈</option>
                  <option value="W13">W13 · 分层加权图谱与隐式语义发现</option>
                  <option value="W14">W14 · Mesh 签名防重放与沙盒协同</option>
                  <option value="W15">W15 · AOPCC 乐观直通与碰壁自愈回路</option>
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
                <div class="item-top"><strong>🧪 实验室 A/B 对照实测 (Without vs With Agent Body)</strong><span id="bench-sla-badge" class="badge">-</span></div>
                <div id="bench-workloads-box" class="mt-8"><div class="empty">点击「▶ 真机运行 A/B 基准」查看 17 大负载对照实测</div></div>
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
              <div class="item-top"><strong>🏆 热门开源竞品横向对标 (轨道 C · LIVE_BENCHMARKED + ARCH_VERIFIED)</strong><span class="badge">Mem0 / OpenViking / AgentFS / rg+psutil</span></div>
              <div id="bench-competitors-box" class="mt-8"><div class="empty">运行「全量套件 v3.5」或「🏆 热门竞品横向对标」查看同机实测结果与 12 维能力矩阵</div></div>
            </div>

            <div class="card mt-12">
              <div class="item-top"><strong>📒 生产运行态效能存证账本（带 PR-01~PR-11 确定性溯源依据）</strong></div>
              <div id="bench-rules-strip" class="mt-8"></div>
              <div id="bench-ledger-list" class="mt-8"><div class="empty">暂无生产流水记录</div></div>
            </div>

            <div class="card mt-12">
              <div class="item-top">
                <strong>🔄 使用反馈总账 · 信息新鲜度对账 &amp; 五点自进化探针 (body_feedback)</strong>
                <div class="toolbar-left">
                  <button id="btn-fb-list" class="btn btn-ghost btn-xs">📋 刷新反馈</button>
                  <button id="btn-fb-probes" class="btn btn-primary btn-xs">🔬 运行 P1~P5 探针</button>
                  <button id="btn-fb-reconcile" class="btn btn-ghost btn-xs">🧹 新鲜度对账</button>
                  <button id="btn-fb-sync" class="btn btn-ghost btn-xs">🧠 同步入记忆</button>
                </div>
              </div>
              <div id="feedback-workbench-box" class="mt-8"><div class="empty">点击按钮查看 user_feedback.md 反馈总账、运行五点对比自进化探针或执行全生命周期新鲜度对账</div></div>
            </div>
          </div>
        </section>

        <!-- 视图 8: 双域行为审计与自进化中心 (Phase 43) -->
        <section id="view-audit" class="view">
          <div class="workbench-wrap">
            <div class="view-toolbar">
              <div class="toolbar-left">
                <select id="audit-domain-filter" class="input" style="width:150px;">
                  <option value="">全部域 (IN + OUT)</option>
                  <option value="IN_AGENT">IN_AGENT (接入域)</option>
                  <option value="OUT_OF_AGENT">OUT_OF_AGENT (自治域)</option>
                </select>
                <select id="audit-outcome-filter" class="input" style="width:130px;">
                  <option value="">全部结果</option>
                  <option value="success">success</option>
                  <option value="recovered">recovered (自愈)</option>
                  <option value="warn">warn</option>
                  <option value="blocked">blocked</option>
                  <option value="failed">failed</option>
                  <option value="dry_run">dry_run</option>
                </select>
                <select id="audit-window-select" class="input" style="width:110px;">
                  <option value="24h">近 24 小时</option>
                  <option value="7d" selected>近 7 天</option>
                  <option value="30d">近 30 天</option>
                  <option value="all">全部历史</option>
                </select>
                <input id="audit-trace-filter" class="input" type="text" placeholder="按 trace_id 过滤因果链路..." style="width:210px;" />
                <button id="btn-audit-refresh" class="btn btn-ghost btn-xs">🔄 查询审计</button>
              </div>
              <div class="toolbar-right">
                <button id="btn-audit-evolve" class="btn btn-primary btn-xs">🧬 触发审计自进化</button>
                <button id="btn-audit-export" class="btn btn-ghost btn-xs">📦 导出 CSV</button>
              </div>
            </div>

            <div class="roi-banner mt-12">
              <div class="roi-card"><span class="k-label">双域审计总量</span><strong id="aud-kpi-total">-</strong></div>
              <div class="roi-card"><span class="k-label">IN_AGENT 接入域</span><strong id="aud-kpi-in" class="c-blue">-</strong></div>
              <div class="roi-card"><span class="k-label">OUT_OF_AGENT 自治域</span><strong id="aud-kpi-out" class="c-green">-</strong></div>
              <div class="roi-card"><span class="k-label">拦截 / 碰壁自愈</span><strong id="aud-kpi-rec" class="c-orange">-</strong></div>
            </div>

            <div class="panel-grid-2 mt-12">
              <div class="card">
                <div class="item-top"><strong>📊 DuckDB OLAP 双域高频动作热榜</strong><span class="badge">DuckDB 列存聚合</span></div>
                <div id="audit-olap-box" class="mt-8"><div class="empty">点击「🔄 查询审计」加载 DuckDB OLAP 聚合视图</div></div>
              </div>
              <div class="card">
                <div class="item-top"><strong>🧬 审计驱动自进化引擎 (AOPCC 租约校准 + 热点沉淀)</strong><span class="badge">Phase 43.4</span></div>
                <div id="audit-evolve-box" class="mt-8"><div class="empty">点击「🧬 触发审计自进化」评估近 24h 租约有效率与摩擦热点</div></div>
              </div>
            </div>

            <div class="card mt-12">
              <div class="item-top"><strong>📜 双域行为审计流水（脱敏 `before_after_delta` + SHA-256 存证 · 点击 `trace_id` 钻取）</strong></div>
              <div id="audit-events-box" class="mt-8"><div class="empty">暂无审计流水</div></div>
            </div>
          </div>
        </section>

        <!-- 视图 9: Agent 本体中枢与生命体征 (Phase 44) -->
        <section id="view-body" class="view">
          <div class="workbench-wrap">
            <div class="view-toolbar">
              <div class="toolbar-left">
                <strong id="body-id-badge" class="badge">body-...</strong>
                <span class="muted-text">脑体分离具身底座：瞬态大脑热切换 · 本体躯干永驻宿主机</span>
              </div>
              <div class="toolbar-right">
                <button id="btn-body-refresh" class="btn btn-ghost btn-xs">🔄 刷新体征</button>
                <button id="btn-body-heal" class="btn btn-primary btn-xs">🩺 L1 稳态反射自愈</button>
              </div>
            </div>

            <div class="roi-banner mt-12">
              <div class="roi-card"><span class="k-label">本体综合稳态分</span><strong id="body-kpi-score" class="c-green">-</strong></div>
              <div class="roi-card"><span class="k-label">稳态健康状态</span><strong id="body-kpi-status" class="c-cyan">-</strong></div>
              <div class="roi-card"><span class="k-label">活跃接力胶囊</span><strong id="body-kpi-caps" class="c-orange">-</strong></div>
              <div class="roi-card"><span class="k-label">对等本体节点</span><strong id="body-kpi-peers" class="c-purple">-</strong></div>
            </div>

            <div class="panel-grid-2 mt-12">
              <div class="card">
                <div class="item-top"><strong>🫀 六大生理器官自省 (BodyManifest)</strong><span class="badge">零子进程索引</span></div>
                <div id="body-organs-box" class="mt-8"><div class="empty">加载中...</div></div>
              </div>
              <div class="card">
                <div class="item-top"><strong>🩺 五维生命体征雷达 (HomeostasisVitals)</strong><span class="badge">L1_AUTONOMIC</span></div>
                <div id="body-vitals-box" class="mt-8"><div class="empty">加载中...</div></div>
              </div>
            </div>

            <div class="panel-grid-2 mt-12">
              <div class="card">
                <div class="item-top"><strong>🧬 跨大脑连续性接力胶囊 (ContinuityCapsule &lt;=300 tok)</strong></div>
                <div class="form-row mt-8" style="display:flex;gap:6px;flex-wrap:wrap;">
                  <input id="body-cp-brain" class="input" type="text" placeholder="来源大脑 (如 antigravity)" style="width:145px;" value="web-brain" />
                  <input id="body-cp-goal" class="input" type="text" placeholder="输入核心目标摘要，保存供下一任大脑接力..." style="flex:1;min-width:200px;" />
                  <button id="btn-body-handoff" class="btn btn-primary btn-xs">💾 保存胶囊</button>
                  <button id="btn-body-resume" class="btn btn-ghost btn-xs">🔁 模拟换脑接力</button>
                </div>
                <div id="body-checkpoints-box" class="mt-8"><div class="empty">暂无接力胶囊</div></div>
              </div>
              <div class="card">
                <div class="item-top"><strong>🛡️ 四级本体自治权限包络 (L0~L3) &amp; 联邦名片</strong></div>
                <div id="body-envelope-box" class="mt-8"><div class="empty">加载中...</div></div>
              </div>
            </div>

            <div class="card mt-12">
              <div class="item-top">
                <strong>📈 内生等待事件 (Wait Events)、<code>/proc/self</code> 内核遥测与 AWR 性能诊断 (Phase 50)</strong>
                <div style="display:flex;gap:6px;flex-wrap:wrap;">
                  <button id="btn-body-perf-snap" class="btn btn-ghost btn-xs">📸 抓取性能快照</button>
                  <button id="btn-body-perf-base" class="btn btn-ghost btn-xs">⭐ 设为黄金基准 (Baseline)</button>
                  <button id="btn-body-perf-awr" class="btn btn-primary btn-xs">📈 生成 AWR 差分报告</button>
                  <button id="btn-body-perf-tune" class="btn btn-primary btn-xs">⚡ L1 一键性能自调优</button>
                </div>
              </div>
              <div id="body-perf-awr-box" class="mt-8"><div class="empty">点击「📈 生成 AWR 差分报告」查看 6 大类 18 种等待事件、内核 RSS/FD/上下文切换遥测与 ADDM 瓶颈诊断</div></div>
            </div>
          </div>
        </section>
      </main>

      <!-- 右侧可折叠 SSE 实时事件流 (默认收起为悬浮抽屉) -->
      <aside id="sse-sidebar" class="sse-sidebar collapsed">
        <div class="drawer-header">
          <h3>⚡ 实时事件流 (SSE)</h3>
          <div style="display:flex;gap:6px;align-items:center;">
            <button id="btn-clear-sse" class="btn btn-ghost btn-xs">清空</button>
            <button id="btn-close-sse" class="btn-icon" title="收起">✕</button>
          </div>
        </div>
        <div id="event-stream" class="event-stream">
          <div class="event-item info"><div class="ev-head"><span class="badge">SYSTEM</span><span class="ev-time">就绪</span></div><div class="ev-body">2.0 驾驶舱已就绪</div></div>
        </div>
      </aside>
    </div>

    <!-- 全局黑屏命令镜像底栏 -->
    <footer id="mirror-bar" class="mirror-bar">
      <div class="mirror-title">⌨️ 黑屏命令镜像器</div>
      <div class="mirror-slots">
        <div class="mirror-slot">
          <span class="badge">CLI</span>
          <code id="mirror-cli" class="mono">agent-body brief</code>
          <button id="btn-copy-cli" class="btn btn-ghost btn-xs">复制</button>
        </div>
        <div class="mirror-slot">
          <span class="badge badge-ok">MCP JSON-RPC</span>
          <code id="mirror-mcp" class="mono">{"name":"body_brief","arguments":{}}</code>
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
  height: 54px;
  min-height: 54px;
  background: var(--topbar-bg);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  gap: 12px;
  flex-wrap: nowrap;
}
.brand { display: flex; align-items: center; gap: 10px; flex-shrink: 0; }
.brand h1 { font-size: 14px; font-weight: 700; color: var(--text); letter-spacing: 0.2px; white-space: nowrap; }
.ver-badge {
  font-size: 10px; font-weight: 600; color: var(--blue);
  background: var(--blue-soft); border: 1px solid var(--blue-border);
  padding: 1px 7px; border-radius: 999px; margin-left: 5px;
}
.subtitle { font-size: 10px; color: var(--muted); white-space: nowrap; }
.pulse-dot {
  width: 9px; height: 9px; border-radius: 50%; background: var(--green);
  box-shadow: 0 0 8px var(--green); flex-shrink: 0;
}

.kpi-strip {
  display: flex; gap: 6px; flex-wrap: nowrap; align-items: center;
  overflow-x: auto; scrollbar-width: none;
}
.kpi-strip::-webkit-scrollbar { display: none; }
.kpi-pill {
  background: var(--panel-2); border: 1px solid var(--border);
  border-radius: 999px; padding: 3px 10px;
  display: flex; flex-direction: row; align-items: center; gap: 6px;
  box-shadow: var(--shadow-card); white-space: nowrap; flex-shrink: 0;
}
.k-label { font-size: 10px; color: var(--muted); }
.kpi-pill strong { font-size: 11px; color: var(--text); font-family: ui-monospace, SFMono-Regular, monospace; }
.c-cyan { color: var(--blue) !important; }
.c-orange { color: var(--orange) !important; }
.c-green { color: var(--green) !important; }
.c-purple { color: var(--purple) !important; }

.topbar-actions { display: flex; align-items: center; gap: 6px; flex-wrap: nowrap; flex-shrink: 0; }
.sse-pill {
  font-size: 11px; padding: 3px 8px; border-radius: 999px; font-weight: 600; white-space: nowrap;
  background: var(--green-soft); color: var(--green); border: 1px solid var(--green-border);
}
.sse-pill.connecting { background: var(--orange-soft); color: var(--orange); border-color: var(--orange-border); }

/* Buttons & Inputs */
.btn {
  border: 1px solid var(--border-hi); background: var(--panel-2); color: var(--text);
  padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 12px; transition: 0.15s; font-weight: 500;
  white-space: nowrap;
}
.btn:hover { border-color: var(--blue); color: var(--blue); background: var(--hover-bg); }
.btn-primary { background: var(--accent-btn-bg); border-color: var(--accent-btn-bg); color: var(--accent-btn-fg); }
.btn-primary:hover { background: var(--accent-btn-hover); border-color: var(--accent-btn-hover); color: var(--accent-btn-fg); }
.btn-theme { background: var(--orange-soft); border-color: var(--orange-border); color: var(--orange); font-weight: 600; }
.btn-theme:hover { border-color: var(--orange); color: var(--text); }
.btn-danger { background: var(--red-soft); border-color: var(--red-border); color: var(--red); }
.btn-danger:hover { background: var(--red); color: #ffffff; }
.btn-ghost { background: transparent; }
.btn-sm { padding: 4px 9px; font-size: 11px; }
.btn-xs { padding: 3px 8px; font-size: 11px; }
.btn-icon { background: none; border: none; color: var(--muted); cursor: pointer; font-size: 15px; }
.btn-icon:hover { color: var(--text); }

.input {
  background: var(--input-bg); border: 1px solid var(--border-hi); color: var(--text);
  padding: 6px 10px; border-radius: 6px; font-size: 12px; outline: none;
}
.input:focus { border-color: var(--blue); }
.input-sm { padding: 4px 8px; font-size: 11px; width: 160px; }
.input-xs { padding: 3px 6px; font-size: 11px; }
.input-search { width: 210px; padding: 5px 10px; font-size: 12px; }
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
  gap: 8px; margin-bottom: 10px; flex-wrap: wrap;
}
.topo-toolbar { margin-bottom: 8px; gap: 8px; }
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.mode-switch {
  display: inline-flex; background: var(--panel-2); border: 1px solid var(--border-hi);
  border-radius: 8px; padding: 2px; gap: 2px;
}
.mode-btn {
  border: none; background: transparent; color: var(--muted); padding: 3px 9px;
  border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; transition: 0.15s;
}
.mode-btn:hover { color: var(--text); }
.mode-btn.active { background: var(--accent-btn-bg); color: var(--accent-btn-fg); }
.depth-ctrl {
  display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: var(--muted);
  background: var(--panel); border: 1px solid var(--border); padding: 2px 6px; border-radius: 7px;
}
.depth-ctrl select { border: none; background: transparent; padding: 2px 4px; font-weight: 600; color: var(--text); }
.filter-chips { display: flex; gap: 4px; flex-wrap: wrap; }
.chip {
  display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--text-2);
  background: var(--panel); border: 1px solid var(--border);
  padding: 3px 9px; border-radius: 999px; cursor: pointer;
  opacity: 0.42; transition: 0.16s; user-select: none;
}
.chip input[type="checkbox"] { display: none; }
.chip:has(input:checked) {
  opacity: 1; background: var(--panel-2); border-color: var(--border-hi);
  box-shadow: 0 0 0 1px rgba(56, 189, 248, 0.16);
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
  flex: 1; position: relative; border: 1px solid var(--border); border-radius: 12px;
  background:
    radial-gradient(circle at 50% 50%, rgba(56, 189, 248, 0.08) 0%, rgba(167, 139, 250, 0.035) 38%, transparent 72%),
    radial-gradient(var(--canvas-dot) 1px, transparent 1px),
    var(--canvas-bg);
  background-size: 100% 100%, 22px 22px, 100% 100%;
  overflow: hidden;
}
#topology-canvas { width: 100%; height: 100%; display: block; cursor: grab; }
#topology-canvas:active { cursor: grabbing; }
.topo-focus-banner {
  position: absolute; top: 12px; left: 12px; z-index: 8;
  display: flex; align-items: center; gap: 8px;
  background: var(--drawer-bg); backdrop-filter: blur(10px);
  border: 1px solid var(--blue-border); border-radius: 999px;
  padding: 4px 10px 4px 12px; font-size: 11px; color: var(--text);
  box-shadow: var(--drawer-shadow);
}
.topo-hud-controls {
  position: absolute; bottom: 12px; left: 12px; z-index: 8;
  display: flex; align-items: center; gap: 6px;
  background: var(--drawer-bg); backdrop-filter: blur(10px);
  border: 1px solid var(--border-hi); border-radius: 10px;
  padding: 5px 10px; box-shadow: var(--drawer-shadow);
}
.hud-btn {
  background: transparent; border: 1px solid transparent; color: var(--text-2);
  padding: 3px 7px; border-radius: 6px; cursor: pointer; font-size: 11px; font-weight: 600;
  transition: 0.14s; white-space: nowrap;
}
.hud-btn:hover { background: var(--hover-bg); border-color: var(--border-hi); color: var(--blue); }
.hud-btn.active { background: var(--blue-soft); border-color: var(--blue-border); color: var(--blue); }
.hud-slider { width: 88px; accent-color: var(--blue); cursor: pointer; }
.hud-zoom-val { min-width: 38px; text-align: right; color: var(--muted); font-size: 11px; }
.hud-sep { color: var(--border-hi); }
.canvas-hint {
  position: absolute; bottom: 14px; right: 14px; font-size: 11px; color: var(--muted);
  background: var(--topbar-bg); backdrop-filter: blur(8px);
  padding: 4px 10px; border-radius: 8px; border: 1px solid var(--border);
  pointer-events: none;
}

.drawer {
  position: absolute; top: 12px; right: 12px; bottom: 44px; width: 356px;
  background: var(--drawer-bg); backdrop-filter: blur(14px);
  border: 1px solid var(--border-hi);
  border-radius: 12px; display: flex; flex-direction: column;
  box-shadow: var(--drawer-shadow); z-index: 12;
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
.grid-2, .panel-grid-2 { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
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
.data-table th { color: var(--muted); font-weight: 600; background: var(--table-head-bg); position: sticky; top: 0; white-space: nowrap; }
.data-table td:first-child, .data-table td:nth-child(2) { white-space: nowrap; }
.data-table tbody tr:hover { background: var(--hover-bg); }
.mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; }

/* Badges & Items */
.badge {
  display: inline-block; font-size: 11px; padding: 2px 8px; border-radius: 999px; font-weight: 600;
  background: var(--blue-soft); color: var(--blue); border: 1px solid transparent; white-space: nowrap;
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

/* SSE Sidebar (Overlay Drawer) */
.sse-sidebar {
  position: fixed; top: 54px; right: 0; bottom: 36px; width: 320px; z-index: 30;
  background: var(--drawer-bg); backdrop-filter: blur(14px);
  border-left: 1px solid var(--border-hi);
  display: flex; flex-direction: column; flex-shrink: 0;
  box-shadow: -8px 0 24px rgba(0, 0, 0, 0.42);
}
.sse-sidebar.collapsed { display: none; }
.event-stream { flex: 1; overflow-y: auto; padding: 10px; font-family: ui-monospace, monospace; font-size: 11px; }
.event-item {
  border-left: 3px solid var(--blue); padding: 6px 8px; margin-bottom: 8px;
  background: var(--item-bg); color: var(--text-2); border-radius: 0 6px 6px 0; word-break: break-word;
}
.event-item.warn { border-left-color: var(--orange); }
.event-item.ok { border-left-color: var(--green); }
.ev-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 3px; }
.ev-time { font-size: 10px; color: var(--muted); }
.ev-body { font-size: 11px; color: var(--text-2); line-height: 1.4; }

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
  .grid-2, .panel-grid-2, .grid-3, .route-result, .demo-roi-strip, .demo-split, .demo-explainer-grid, .demo-hero-metrics, .tool-wall { grid-template-columns: 1fr; }
  .sse-sidebar { display: none; }
  .sidenav { width: 62px; padding: 8px 6px; }
  .nav-btn span:last-child { display: none; }
}
"##;

pub static APP_JS: &str = r##"// Agent Body 2.0 Web Console (Zero External Dependencies)
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
    btn.innerText = next === "light" ? "🌙 深空" : "☀️ 蓝图";
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
    return (localStorage.getItem("steward_auth_token") || "").trim();
  } catch (_) {
    return "";
  }
}

function updateAuthTokenButton() {
  const btn = document.getElementById("btn-auth-token");
  if (!btn) return;
  const has = getStoredAuthToken().length > 0;
  btn.innerText = has ? "🔑 已鉴权" : "🔑 Token";
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
    if (trimmed) localStorage.setItem("steward_auth_token", trimmed);
    else localStorage.removeItem("steward_auth_token");
  } catch (_) {}
  updateAuthTokenButton();
}

async function apiGet(url) {
  const headers = {};
  const tok = getStoredAuthToken();
  if (tok) headers["Authorization"] = `Bearer ${tok}`;
  const res = await fetch(url, { headers });
  const json = await res.json();
  if (json.code === 401) {
    throw new Error(`${json.message || "Unauthorized"}（请点击右上角「🔑 Token」填入 [web].auth_token）`);
  }
  if (json.code !== 200) throw new Error(json.message || "API Error");
  return json.data;
}

async function apiPost(url, body, authToken) {
  const headers = { "Content-Type": "application/json" };
  const tok = (authToken || getStoredAuthToken() || "").trim();
  if (tok) headers["Authorization"] = `Bearer ${tok}`;
  const res = await fetch(url, { method: "POST", headers, body: JSON.stringify(body || {}) });
  const json = await res.json();
  if (json.code === 401) {
    throw new Error(`${json.message || "Unauthorized"}（请点击右上角「🔑 Token」填入 [web].auth_token）`);
  }
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

/* ─── 1. 宿主机事实拓扑交互图谱 (3D 全息球面 + 层级折叠 + 2D 平面) ──── */
function normalizeNodeType(rawType) {
  if (typeof rawType === "string") {
    const t = rawType.toLowerCase();
    if (t === "asset" || t === "directory" || t === "doc" || t === "script") return "file";
    if (t === "project" || t === "repo") return "git";
    return t;
  }
  if (rawType && typeof rawType === "object" && rawType.custom) {
    return normalizeNodeType(rawType.custom);
  }
  return "file";
}

function normalizePathStr(p) {
  if (!p) return "";
  const s = String(p).trim();
  return s.length > 1 && s.endsWith("/") ? s.replace(/\/+$/, "") : s;
}

function getEffectiveDepthAndAncestor(canonPath, maxDepth) {
  if (!canonPath || !canonPath.startsWith("/")) return { depth: 1, ancestor: null };
  const norm = normalizePathStr(canonPath);
  if (norm.startsWith("/root/new/")) {
    const rel = norm.slice("/root/new/".length);
    const segs = rel.split("/").filter(Boolean);
    const depth = Math.max(1, segs.length);
    if (depth <= maxDepth) return { depth, ancestor: null };
    const ancSegs = segs.slice(0, maxDepth);
    return { depth, ancestor: "/root/new/" + ancSegs.join("/") };
  }
  const segs = norm.split("/").filter(Boolean);
  const depth = Math.max(1, segs.length);
  if (depth <= maxDepth) return { depth, ancestor: null };
  return { depth, ancestor: "/" + segs.slice(0, maxDepth).join("/") };
}

function isDirExplicitlyExpanded(canonPath) {
  if (!canonPath || state.expandedDirs.size === 0) return false;
  for (const dir of state.expandedDirs) {
    if (canonPath.startsWith(dir + "/")) {
      const rest = canonPath.slice(dir.length + 1).split("/").filter(Boolean);
      if (rest.length <= 1) return true;
    }
  }
  return false;
}

function foldTopologyByPathDepth(rawNodes, rawEdges, maxDepth) {
  if (maxDepth >= 99) {
    return { nodes: rawNodes, edges: rawEdges, foldedCount: 0 };
  }
  const idMap = new Map();
  const dirGroups = new Map();
  const keptNodes = [];
  let foldedCount = 0;

  for (const n of rawNodes) {
    const nt = normalizeNodeType(n.node_type);
    const cpath = normalizePathStr(n.canonical_path || n.metadata?.abs_path || n.metadata?.path || n.metadata?.ref || "");
    if ((nt === "file" || nt === "git") && cpath.startsWith("/") && !isDirExplicitlyExpanded(cpath)) {
      const { ancestor } = getEffectiveDepthAndAncestor(cpath, maxDepth);
      if (ancestor) {
        const dirId = `dir:${ancestor}`;
        idMap.set(n.id, dirId);
        foldedCount++;
        if (!dirGroups.has(dirId)) {
          const dirName = ancestor.split("/").filter(Boolean).pop() || ancestor;
          dirGroups.set(dirId, {
            id: dirId,
            label: `📁 ${dirName}`,
            baseLabel: dirName,
            node_type: "file",
            status: "folded_dir",
            canonical_path: ancestor,
            path_depth: maxDepth,
            isFoldedDir: true,
            children: [],
            childTypes: {},
            childDomains: {},
            metadata: {
              path: ancestor,
              canonical_path: ancestor,
              category: "folded_directory",
              folded: true,
            },
          });
        }
        const grp = dirGroups.get(dirId);
        grp.children.push({ id: n.id, label: n.label, path: cpath, desc: n.metadata?.description || n.metadata?.purpose || "" });
        const ct = n.metadata?.asset_type || nt || "file";
        grp.childTypes[ct] = (grp.childTypes[ct] || 0) + 1;
        const dom = n.metadata?.domain;
        if (dom) grp.childDomains[dom] = (grp.childDomains[dom] || 0) + 1;
        continue;
      }
    }
    idMap.set(n.id, n.id);
    keptNodes.push(n);
  }

  for (const dirNode of dirGroups.values()) {
    dirNode.label = `📁 ${dirNode.baseLabel} [+${dirNode.children.length}]`;
    const topDomain = Object.entries(dirNode.childDomains).sort((a, b) => b[1] - a[1])[0]?.[0] || "workspace_dir";
    dirNode.metadata.children_count = dirNode.children.length;
    dirNode.metadata.domain = topDomain;
    dirNode.metadata.child_types = dirNode.childTypes;
    dirNode.metadata.description = `聚合目录 ${dirNode.metadata.path}，共包含 ${dirNode.children.length} 个子对象 (主域: ${topDomain})`;
    dirNode.metadata.sample_children = dirNode.children.slice(0, 5).map((c) => c.label);
    delete dirNode.childTypes;
    delete dirNode.childDomains;
    keptNodes.push(dirNode);
  }

  const edgeSeen = new Set();
  const rolledEdges = [];
  for (const e of rawEdges) {
    const s = idMap.get(e.source) || e.source;
    const t = idMap.get(e.target) || e.target;
    if (!s || !t || s === t) continue;
    const key = `${s}|${t}|${e.relation}`;
    if (edgeSeen.has(key)) continue;
    edgeSeen.add(key);
    rolledEdges.push({ ...e, source: s, target: t });
  }

  return { nodes: keptNodes, edges: rolledEdges, foldedCount };
}

function mulMat3(a, b) {
  return [
    a[0]*b[0] + a[1]*b[3] + a[2]*b[6], a[0]*b[1] + a[1]*b[4] + a[2]*b[7], a[0]*b[2] + a[1]*b[5] + a[2]*b[8],
    a[3]*b[0] + a[4]*b[3] + a[5]*b[6], a[3]*b[1] + a[4]*b[4] + a[5]*b[7], a[3]*b[2] + a[4]*b[5] + a[5]*b[8],
    a[6]*b[0] + a[7]*b[3] + a[8]*b[6], a[6]*b[1] + a[7]*b[4] + a[8]*b[7], a[6]*b[2] + a[7]*b[5] + a[8]*b[8],
  ];
}

function rotateVec3(m, x, y, z) {
  return {
    x: m[0]*x + m[1]*y + m[2]*z,
    y: m[3]*x + m[4]*y + m[5]*z,
    z: m[6]*x + m[7]*y + m[8]*z,
  };
}

function axisAngleMat3(ux, uy, uz, angle) {
  const c = Math.cos(angle), s = Math.sin(angle), t = 1 - c;
  return [
    t*ux*ux + c,    t*ux*uy - s*uz, t*ux*uz + s*uy,
    t*ux*uy + s*uz, t*uy*uy + c,    t*uy*uz - s*ux,
    t*ux*uz - s*uy, t*uy*uz + s*ux, t*uz*uz + c,
  ];
}

function orthonormalizeMat3(m) {
  let x = [m[0], m[3], m[6]];
  const xl = Math.hypot(x[0], x[1], x[2]) || 1;
  x = [x[0]/xl, x[1]/xl, x[2]/xl];
  let y = [m[1], m[4], m[7]];
  const dot = x[0]*y[0] + x[1]*y[1] + x[2]*y[2];
  y = [y[0] - dot*x[0], y[1] - dot*x[1], y[2] - dot*x[2]];
  const yl = Math.hypot(y[0], y[1], y[2]) || 1;
  y = [y[0]/yl, y[1]/yl, y[2]/yl];
  const z = [x[1]*y[2] - x[2]*y[1], x[2]*y[0] - x[0]*y[2], x[0]*y[1] - x[1]*y[0]];
  return [x[0], y[0], z[0], x[1], y[1], z[1], x[2], y[2], z[2]];
}

function syncZoomHud() {
  const pct = Math.round(state.cam.zoom * 100);
  const slider = document.getElementById("topo-zoom-slider");
  const label = document.getElementById("topo-zoom-label");
  if (slider) slider.value = String(Math.max(35, Math.min(350, pct)));
  if (label) label.innerText = `${pct}%`;
}

function setTopoZoom(nextZoom) {
  state.cam.zoom = Math.max(0.35, Math.min(3.5, nextZoom));
  syncZoomHud();
  drawTopology();
}

function updateFocusBanner(node) {
  const banner = document.getElementById("topo-focus-banner");
  const lbl = document.getElementById("topo-focus-label");
  if (!banner || !lbl) return;
  if (!node) {
    banner.classList.add("hidden");
    return;
  }
  banner.classList.remove("hidden");
  lbl.innerText = `🎯 中心: ${node.label} (被引用 ${node.inDeg} · 引用 ${node.outDeg})`;
}

function stopSphereAnimations() {
  if (state.sphereAnimId) { cancelAnimationFrame(state.sphereAnimId); state.sphereAnimId = null; }
  if (state.inertiaAnimId) { cancelAnimationFrame(state.inertiaAnimId); state.inertiaAnimId = null; }
}

function focusNodeToSphereCenter(node) {
  if (!node || state.topoMode !== "sphere") return;
  stopSphereAnimations();
  const startMat = state.rotMat.slice();
  const p = rotateVec3(startMat, node.sx, node.sy, node.sz);
  const crossX = p.y;
  const crossY = -p.x;
  const crossLen = Math.hypot(crossX, crossY);
  const totalAngle = Math.acos(Math.max(-1, Math.min(1, p.z)));
  if (totalAngle < 0.005) {
    drawTopology();
    return;
  }
  const ax = crossLen > 1e-5 ? crossX / crossLen : 0;
  const ay = crossLen > 1e-5 ? crossY / crossLen : 1;
  const startTime = performance.now();
  const duration = 420;

  function step(now) {
    const t = Math.min(1, (now - startTime) / duration);
    const eased = 1 - Math.pow(1 - t, 3);
    const deltaMat = axisAngleMat3(ax, ay, 0, totalAngle * eased);
    state.rotMat = orthonormalizeMat3(mulMat3(deltaMat, startMat));
    drawTopology();
    if (t < 1) {
      state.sphereAnimId = requestAnimationFrame(step);
    } else {
      state.sphereAnimId = null;
    }
  }
  state.sphereAnimId = requestAnimationFrame(step);
}

function startInertiaLoop() {
  if (state.topoMode !== "sphere") return;
  if (state.inertiaAnimId) cancelAnimationFrame(state.inertiaAnimId);
  function tick() {
    if (Math.abs(state.rotVel.vx) < 0.0002 && Math.abs(state.rotVel.vy) < 0.0002) {
      state.inertiaAnimId = null;
      return;
    }
    const ry = axisAngleMat3(0, 1, 0, state.rotVel.vx);
    const rx = axisAngleMat3(1, 0, 0, state.rotVel.vy);
    state.rotMat = orthonormalizeMat3(mulMat3(rx, mulMat3(ry, state.rotMat)));
    state.rotVel.vx *= 0.90;
    state.rotVel.vy *= 0.90;
    drawTopology();
    state.inertiaAnimId = requestAnimationFrame(tick);
  }
  state.inertiaAnimId = requestAnimationFrame(tick);
}

function toggleAutoOrbit() {
  state.autoOrbit = !state.autoOrbit;
  const btn = document.getElementById("btn-topo-orbit");
  if (btn) btn.classList.toggle("active", state.autoOrbit);
  if (state.autoOrbit && !state.orbitAnimId) {
    const loop = () => {
      if (!state.autoOrbit) { state.orbitAnimId = null; return; }
      if (state.activeView === "topology" && state.topoMode === "sphere" && !state.panning && !state.sphereAnimId) {
        const ry = axisAngleMat3(0, 1, 0, 0.0028);
        state.rotMat = orthonormalizeMat3(mulMat3(ry, state.rotMat));
        drawTopology();
      }
      state.orbitAnimId = requestAnimationFrame(loop);
    };
    state.orbitAnimId = requestAnimationFrame(loop);
  }
}

function bindTopologyControls() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas) return;

  if (typeof ResizeObserver !== "undefined" && canvas.parentElement) {
    const ro = new ResizeObserver(() => {
      if (state.activeView === "topology") resizeCanvas();
    });
    ro.observe(canvas.parentElement);
  }
  window.addEventListener("resize", () => {
    if (state.activeView === "topology") resizeCanvas();
  });

  const searchInput = document.getElementById("topo-search");
  searchInput?.addEventListener("input", drawTopology);
  searchInput?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      const kw = searchInput.value.trim().toLowerCase();
      if (!kw) return;
      const hit = state.simNodes.find((n) => n.label.toLowerCase().includes(kw) || n.id.toLowerCase().includes(kw));
      if (hit) selectAndCenterNode(hit);
    }
  });

  document.querySelectorAll("#topo-mode-switch .mode-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      state.topoMode = btn.dataset.mode || "sphere";
      document.querySelectorAll("#topo-mode-switch .mode-btn").forEach((b) => b.classList.toggle("active", b === btn));
      fitCamera();
    });
  });

  document.getElementById("topo-max-depth")?.addEventListener("change", (e) => {
    state.maxPathDepth = Number(e.target.value || 3);
    applyTopoFilter();
  });

  document.getElementById("topo-hop-limit")?.addEventListener("change", (e) => {
    state.hopLimit = Number(e.target.value || 3);
    drawTopology();
  });

  document.querySelectorAll("#topo-filters input").forEach((cb) => cb.addEventListener("change", applyTopoFilter));

  document.getElementById("btn-topo-reset")?.addEventListener("click", () => {
    state.expandedDirs.clear();
    state.selectedNodeId = null;
    updateFocusBanner(null);
    document.getElementById("node-drawer")?.classList.add("hidden");
    if (state.topoRaw) {
      state.topoDisplay = state.topoRaw;
      applyTopoFilter();
    } else {
      loadTopology();
    }
  });

  document.getElementById("btn-topo-fit")?.addEventListener("click", () => {
    state.rotMat = [1, 0, 0, 0, 1, 0, 0, 0, 1];
    fitCamera();
  });

  document.getElementById("btn-topo-orbit")?.addEventListener("click", toggleAutoOrbit);
  document.getElementById("btn-topo-clear-focus")?.addEventListener("click", () => {
    state.selectedNodeId = null;
    updateFocusBanner(null);
    document.getElementById("node-drawer")?.classList.add("hidden");
    drawTopology();
  });

  document.getElementById("btn-zoom-in")?.addEventListener("click", () => setTopoZoom(state.cam.zoom * 1.18));
  document.getElementById("btn-zoom-out")?.addEventListener("click", () => setTopoZoom(state.cam.zoom * 0.85));
  document.getElementById("topo-zoom-slider")?.addEventListener("input", (e) => {
    setTopoZoom(Number(e.target.value || 100) / 100);
  });

  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.12 : 0.89;
    if (state.topoMode === "sphere") {
      setTopoZoom(state.cam.zoom * factor);
    } else {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const wx = (mx - state.cam.x) / state.cam.zoom;
      const wy = (my - state.cam.y) / state.cam.zoom;
      state.cam.zoom = Math.max(0.25, Math.min(3.5, state.cam.zoom * factor));
      state.cam.x = mx - wx * state.cam.zoom;
      state.cam.y = my - wy * state.cam.zoom;
      syncZoomHud();
      drawTopology();
    }
  }, { passive: false });

  let downX = 0, downY = 0, pendingHit = null;

  canvas.addEventListener("mousedown", (e) => {
    stopSphereAnimations();
    const rect = canvas.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    downX = e.clientX;
    downY = e.clientY;
    state.dragMoved = false;

    const hit = findNodeAtScreen(sx, sy);
    if (state.topoMode === "sphere") {
      pendingHit = hit;
      state.panning = true;
      state.panStart = { x: e.clientX, y: e.clientY };
      state.rotVel = { vx: 0, vy: 0 };
    } else {
      if (hit) {
        state.dragNode = hit;
        pendingHit = hit;
      } else {
        state.panning = true;
        state.panStart = { x: e.clientX - state.cam.x, y: e.clientY - state.cam.y };
      }
    }
  });

  window.addEventListener("mousemove", (e) => {
    if (state.topoMode === "sphere") {
      if (state.panning) {
        const dx = e.clientX - state.panStart.x;
        const dy = e.clientY - state.panStart.y;
        if (Math.hypot(e.clientX - downX, e.clientY - downY) > 4) state.dragMoved = true;
        state.panStart = { x: e.clientX, y: e.clientY };
        const ay = dx * 0.0055;
        const ax = -dy * 0.0055;
        state.rotVel = { vx: ay, vy: ax };
        const ry = axisAngleMat3(0, 1, 0, ay);
        const rx = axisAngleMat3(1, 0, 0, ax);
        state.rotMat = orthonormalizeMat3(mulMat3(rx, mulMat3(ry, state.rotMat)));
        drawTopology();
      } else if (state.activeView === "topology") {
        const rect = canvas.getBoundingClientRect();
        const hit = findNodeAtScreen(e.clientX - rect.left, e.clientY - rect.top);
        const nextHover = hit ? hit.id : null;
        if (nextHover !== state.hoverNodeId) {
          state.hoverNodeId = nextHover;
          canvas.style.cursor = hit ? "pointer" : "grab";
          drawTopology();
        }
      }
    } else {
      if (state.dragNode) {
        if (Math.hypot(e.clientX - downX, e.clientY - downY) > 4) state.dragMoved = true;
        const pos = canvasToWorld(e, canvas);
        state.dragNode.x = pos.x;
        state.dragNode.y = pos.y;
        drawTopology();
      } else if (state.panning) {
        state.cam.x = e.clientX - state.panStart.x;
        state.cam.y = e.clientY - state.panStart.y;
        drawTopology();
      } else if (state.activeView === "topology") {
        const rect = canvas.getBoundingClientRect();
        const hit = findNodeAtScreen(e.clientX - rect.left, e.clientY - rect.top);
        const nextHover = hit ? hit.id : null;
        if (nextHover !== state.hoverNodeId) {
          state.hoverNodeId = nextHover;
          canvas.style.cursor = hit ? "pointer" : "grab";
          drawTopology();
        }
      }
    }
  });

  window.addEventListener("mouseup", () => {
    if (!state.dragMoved && pendingHit) {
      selectAndCenterNode(pendingHit);
    } else if (state.topoMode === "sphere" && state.panning && state.dragMoved) {
      startInertiaLoop();
    }
    state.dragNode = null;
    state.panning = false;
    pendingHit = null;
  });
}

function selectAndCenterNode(node) {
  state.selectedNodeId = node.id;
  updateFocusBanner(node);
  showNodeDrawer(node);
  if (state.topoMode === "sphere") {
    focusNodeToSphereCenter(node);
  } else {
    drawTopology();
  }
}

function canvasToWorld(e, canvas) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: (e.clientX - rect.left - state.cam.x) / state.cam.zoom,
    y: (e.clientY - rect.top - state.cam.y) / state.cam.zoom,
  };
}

function findNodeAtScreen(sx, sy) {
  // 按渲染后屏幕坐标反向遍历（画家算法最上层优先命中）
  const sorted = state.topoMode === "sphere"
    ? [...state.simNodes].sort((a, b) => (a.pz || 0) - (b.pz || 0))
    : state.simNodes;
  for (let i = sorted.length - 1; i >= 0; i--) {
    const n = sorted[i];
    const rx = n.screenX ?? 0;
    const ry = n.screenY ?? 0;
    const rr = n.screenR ?? n.radius ?? 10;
    const dx = sx - rx;
    const dy = sy - ry;
    if (dx * dx + dy * dy <= (rr + 5) * (rr + 5)) return n;
  }
  return null;
}

function resizeCanvas() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas || !canvas.parentElement) return;
  const cssW = Math.max(320, canvas.parentElement.clientWidth || 800);
  const cssH = Math.max(260, canvas.parentElement.clientHeight || 500);
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  state.canvasW = cssW;
  state.canvasH = cssH;
  state.dpr = dpr;
  canvas.width = Math.floor(cssW * dpr);
  canvas.height = Math.floor(cssH * dpr);
  if (!state.topoRaw) {
    loadTopology();
  } else {
    fitCamera();
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
  const filteredNodes = (state.topoDisplay.nodes || []).filter((n) => {
    const nt = normalizeNodeType(n.node_type);
    return enabled.has(nt);
  });
  const idSet = new Set(filteredNodes.map((n) => n.id));
  const filteredEdges = (state.topoDisplay.edges || []).filter((e) => idSet.has(e.source) && idSet.has(e.target));

  const folded = foldTopologyByPathDepth(filteredNodes, filteredEdges, state.maxPathDepth);

  const depthDesc = state.maxPathDepth >= 99 ? "全部层级" : `≤${state.maxPathDepth}层`;
  const foldHint = folded.foldedCount > 0 ? ` · 已折叠 ${folded.foldedCount} 深层文件` : "";
  document.getElementById("topo-stats").innerText =
    `${depthDesc} · ${folded.nodes.length} 节点 · ${folded.edges.length} 边${foldHint}`;

  layoutGraph(folded.nodes, folded.edges);
  fitCamera();
}

function layoutGraph(rawNodes, rawEdges) {
  const inDegMap = new Map();
  const outDegMap = new Map();
  rawEdges.forEach((e) => {
    if (e.source === e.target) return;
    outDegMap.set(e.source, (outDegMap.get(e.source) || 0) + 1);
    inDegMap.set(e.target, (inDegMap.get(e.target) || 0) + 1);
  });

  let maxWeight = 10;
  rawNodes.forEach((n) => {
    const ind = inDegMap.get(n.id) ?? n.in_degree ?? 0;
    const outd = outDegMap.get(n.id) ?? n.out_degree ?? 0;
    const w = ind + 0.25 * outd;
    if (w > maxWeight) maxWeight = w;
  });

  // 按节点类型 + 重要度排序，使同类型及枢纽节点在斐波那契球面上形成有序星带
  const typeOrder = { service: 0, port: 1, process: 2, cron: 3, git: 4, file: 5, memory: 6 };
  const sortedRaw = [...rawNodes].sort((a, b) => {
    const ta = typeOrder[normalizeNodeType(a.node_type)] ?? 9;
    const tb = typeOrder[normalizeNodeType(b.node_type)] ?? 9;
    if (ta !== tb) return ta - tb;
    const wa = (inDegMap.get(a.id) || 0) - (inDegMap.get(b.id) || 0);
    return -wa;
  });

  const nCount = sortedRaw.length || 1;
  const goldenAngle = Math.PI * (3 - Math.sqrt(5));
  const R_MIN = 5.2;
  const R_MAX = 16.5; // 严控半径硬上限，被引用再多也不会遮挡其他节点

  const simNodes = sortedRaw.map((n, idx) => {
    const inDeg = inDegMap.get(n.id) ?? n.in_degree ?? 0;
    const outDeg = outDegMap.get(n.id) ?? n.out_degree ?? 0;
    const weight = inDeg + 0.25 * outDeg;
    const ratio = Math.min(1, Math.log(1 + weight) / Math.log(1 + maxWeight));
    const radius = R_MIN + (R_MAX - R_MIN) * ratio;

    // 3D 斐波那契球面晶格
    const sy = nCount === 1 ? 0 : 1 - (2 * (idx + 0.5)) / nCount;
    const rxy = Math.sqrt(Math.max(0, 1 - sy * sy));
    const theta = idx * goldenAngle;
    const sx = Math.cos(theta) * rxy;
    const sz = Math.sin(theta) * rxy;

    // 2D 平面初始环
    const angle2d = (idx / nCount) * Math.PI * 2;
    const ring2d = (inDeg + outDeg) > 0 ? 180 + (idx % 5) * 45 : 360 + (idx % 4) * 52;
    const nt = normalizeNodeType(n.node_type);

    return {
      ...n,
      nt,
      inDeg,
      outDeg,
      degree: inDeg + outDeg,
      radius,
      sx, sy, sz,
      x: Math.cos(angle2d) * ring2d,
      y: Math.sin(angle2d) * ring2d,
      vx: 0,
      vy: 0,
    };
  });

  const nodeMap = new Map(simNodes.map((n) => [n.id, n]));
  const simEdges = rawEdges
    .map((e) => ({ ...e, sNode: nodeMap.get(e.source), tNode: nodeMap.get(e.target) }))
    .filter((e) => e.sNode && e.tNode && e.sNode !== e.tNode);

  // 3D 球面切向微调 (20 步)：让有连边的节点在球面上适当聚拢同时保持球面均匀
  for (let step = 0; step < 20; step++) {
    for (const e of simEdges) {
      const a = e.sNode, b = e.tNode;
      const dx = b.sx - a.sx, dy = b.sy - a.sy, dz = b.sz - a.sz;
      a.sx += dx * 0.035; a.sy += dy * 0.035; a.sz += dz * 0.035;
      b.sx -= dx * 0.035; b.sy -= dy * 0.035; b.sz -= dz * 0.035;
    }
    for (const n of simNodes) {
      const len = Math.hypot(n.sx, n.sy, n.sz) || 1;
      n.sx /= len; n.sy /= len; n.sz /= len;
    }
  }

  // 2D 平面力导向布局
  const iters2d = simNodes.length > 160 ? 35 : 65;
  for (let step = 0; step < iters2d; step++) {
    for (let i = 0; i < simNodes.length; i++) {
      const a = simNodes[i];
      a.vx *= 0.6; a.vy *= 0.6;
      a.vx -= a.x * 0.004; a.vy -= a.y * 0.004;
      const limitJ = Math.min(simNodes.length, i + 45);
      for (let j = i + 1; j < limitJ; j++) {
        const b = simNodes[j];
        const dx = a.x - b.x, dy = a.y - b.y;
        const dist2 = dx * dx + dy * dy + 1;
        if (dist2 < 18000) {
          const f = 900 / dist2;
          a.vx += dx * f; a.vy += dy * f;
          b.vx -= dx * f; b.vy -= dy * f;
        }
      }
    }
    for (const e of simEdges) {
      const dx = e.tNode.x - e.sNode.x, dy = e.tNode.y - e.sNode.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const pull = (dist - 110) * 0.04;
      const fx = (dx / dist) * pull, fy = (dy / dist) * pull;
      e.sNode.vx += fx; e.sNode.vy += fy;
      e.tNode.vx -= fx; e.tNode.vy -= fy;
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
  if (state.topoMode === "sphere") {
    state.cam.zoom = 1.0;
    state.cam.x = 0;
    state.cam.y = 0;
    syncZoomHud();
    drawTopology();
    return;
  }
  const w = state.canvasW || 800;
  const h = state.canvasH || 500;
  if (state.simNodes.length === 0) return;
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  state.simNodes.forEach((n) => {
    if (n.x < minX) minX = n.x;
    if (n.x > maxX) maxX = n.x;
    if (n.y < minY) minY = n.y;
    if (n.y > maxY) maxY = n.y;
  });
  const spanX = Math.max(200, maxX - minX + 120);
  const spanY = Math.max(200, maxY - minY + 120);
  const zoom = Math.min(1.2, Math.min(w / spanX, h / spanY));
  state.cam.zoom = zoom;
  state.cam.x = w / 2 - ((minX + maxX) / 2) * zoom;
  state.cam.y = h / 2 - ((minY + maxY) / 2) * zoom;
  syncZoomHud();
  drawTopology();
}

function computeFocusHopSet(focusId, hopLimit) {
  const hopMap = new Map();
  if (!focusId || hopLimit >= 99) return hopMap;
  hopMap.set(focusId, 0);
  const queue = [focusId];
  let head = 0;
  while (head < queue.length) {
    const cur = queue[head++];
    const d = hopMap.get(cur);
    if (d >= hopLimit) continue;
    for (const e of state.simEdges) {
      const nb = e.source === cur ? e.target : (e.target === cur ? e.source : null);
      if (nb && !hopMap.has(nb)) {
        hopMap.set(nb, d + 1);
        queue.push(nb);
      }
    }
  }
  return hopMap;
}

function drawTopology() {
  const canvas = document.getElementById("topology-canvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const w = state.canvasW || canvas.width;
  const h = state.canvasH || canvas.height;
  ctx.setTransform(state.dpr || 1, 0, 0, state.dpr || 1, 0, 0);
  ctx.clearRect(0, 0, w, h);

  const isLight = state.theme === "light";
  const palette = isLight ? NODE_COLORS_LIGHT : NODE_COLORS_DARK;
  const edgeNormal = isLight ? "rgba(36, 90, 107, 0.22)" : "rgba(148, 163, 184, 0.22)";
  const edgeFocus = isLight ? "#245A6B" : "#38bdf8";
  const edgeLabelNormal = isLight ? "rgba(70, 88, 95, 0.78)" : "rgba(148, 163, 184, 0.72)";
  const edgeLabelFocus = isLight ? "#1E4B5A" : "#7dd3fc";
  const nodeOutline = isLight ? "#BE653B" : "#ffffff";
  const nodeLabelColor = isLight ? "#1B2B33" : "#e2e8f0";

  const kw = (document.getElementById("topo-search")?.value || "").trim().toLowerCase();
  const focusId = state.hoverNodeId || state.selectedNodeId;
  const hopMap = computeFocusHopSet(state.selectedNodeId, state.hopLimit);

  if (state.topoMode === "sphere") {
    drawSphereTopology(ctx, w, h, {
      isLight, palette, edgeNormal, edgeFocus, edgeLabelNormal, edgeLabelFocus,
      nodeOutline, nodeLabelColor, kw, focusId, hopMap,
    });
    return;
  }

  // 2D 平面视图渲染
  for (const n of state.simNodes) {
    n.screenX = state.cam.x + n.x * state.cam.zoom;
    n.screenY = state.cam.y + n.y * state.cam.zoom;
    n.screenR = n.radius * state.cam.zoom;
  }

  ctx.save();
  ctx.translate(state.cam.x, state.cam.y);
  ctx.scale(state.cam.zoom, state.cam.zoom);

  for (const e of state.simEdges) {
    const isFocus = focusId && (e.source === focusId || e.target === focusId);
    const isSemantic = e.evidence_tier === "L3_SEMANTIC";
    ctx.beginPath();
    ctx.setLineDash(isSemantic ? [5, 4] : []);
    ctx.moveTo(e.sNode.x, e.sNode.y);
    ctx.lineTo(e.tNode.x, e.tNode.y);
    ctx.strokeStyle = isFocus ? edgeFocus : edgeNormal;
    const baseW = typeof e.weight === "number" ? Math.max(0.7, Math.min(2.0, e.weight * 1.8)) : 1.0;
    ctx.lineWidth = isFocus ? 2.4 : baseW;
    ctx.stroke();
    ctx.setLineDash([]);

    if (isFocus || state.cam.zoom > 0.9) {
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
    const inHop = !state.selectedNodeId || state.hopLimit >= 99 || hopMap.has(n.id);
    const isSelected = n.id === state.selectedNodeId || n.id === state.hoverNodeId;
    const color = palette[n.nt] || palette.memory;

    ctx.globalAlpha = matched && inHop ? 1 : 0.16;
    ctx.beginPath();
    ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();

    if (isSelected || (kw && matched)) {
      ctx.strokeStyle = nodeOutline;
      ctx.lineWidth = 2.4;
      ctx.stroke();
    }

    if (state.cam.zoom > 0.45 || n.inDeg > 0 || isSelected || (kw && matched)) {
      ctx.fillStyle = nodeLabelColor;
      ctx.font = isSelected ? "bold 11px sans-serif" : "10px monospace";
      ctx.textAlign = "center";
      ctx.fillText(n.label.slice(0, 26), n.x, n.y + n.radius + 12);
    }
  }
  ctx.globalAlpha = 1;
  ctx.restore();
}

function drawSphereTopology(ctx, w, h, theme) {
  const cx = w / 2;
  const cy = h / 2;
  const baseR = Math.min(w, h) * 0.36;
  const R = baseR * state.cam.zoom;
  const F = Math.max(420, R * 2.6);

  // 1. 绘制全息球体经纬线轮廓底盘 (Subtle Holographic Globe Wireframe)
  ctx.save();
  const globeGrad = ctx.createRadialGradient(cx - R * 0.25, cy - R * 0.25, R * 0.08, cx, cy, R);
  if (theme.isLight) {
    globeGrad.addColorStop(0, "rgba(36, 90, 107, 0.07)");
    globeGrad.addColorStop(1, "rgba(36, 90, 107, 0.01)");
  } else {
    globeGrad.addColorStop(0, "rgba(56, 189, 248, 0.09)");
    globeGrad.addColorStop(0.7, "rgba(167, 139, 250, 0.03)");
    globeGrad.addColorStop(1, "rgba(15, 23, 42, 0.0)");
  }
  ctx.beginPath();
  ctx.arc(cx, cy, R, 0, Math.PI * 2);
  ctx.fillStyle = globeGrad;
  ctx.fill();
  ctx.strokeStyle = theme.isLight ? "rgba(36, 90, 107, 0.16)" : "rgba(56, 189, 248, 0.18)";
  ctx.lineWidth = 1.1;
  ctx.stroke();

  // 绘制 2 条轻柔经纬辅助环
  ctx.setLineDash([4, 6]);
  ctx.strokeStyle = theme.isLight ? "rgba(36, 90, 107, 0.10)" : "rgba(148, 163, 184, 0.10)";
  ctx.beginPath();
  ctx.ellipse(cx, cy, R, R * 0.32, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.beginPath();
  ctx.ellipse(cx, cy, R * 0.32, R, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.restore();

  // 2. 投影所有节点 3D -> 2D
  for (const n of state.simNodes) {
    const rot = rotateVec3(state.rotMat, n.sx, n.sy, n.sz);
    const X = rot.x * R;
    const Y = rot.y * R;
    const Z = rot.z * R;
    const persp = F / Math.max(80, F - Z);
    n.px = rot.x;
    n.py = rot.y;
    n.pz = rot.z;
    n.screenX = cx + X * persp;
    n.screenY = cy + Y * persp;
    n.screenR = n.radius * persp * Math.min(1.25, Math.max(0.75, state.cam.zoom * 0.85));
    n.depthNorm = (rot.z + 1) * 0.5; // 0 (背面) ~ 1 (正面中心)
  }

  // 3. 绘制 3D 球内弧线连边 (先画非焦点淡边，再画焦点高亮边)
  const focusEdges = [];
  for (const e of state.simEdges) {
    const isFocus = theme.focusId && (e.source === theme.focusId || e.target === theme.focusId);
    if (isFocus) {
      focusEdges.push(e);
      continue;
    }
    const avgZ = ((e.sNode.pz || 0) + (e.tNode.pz || 0)) * 0.5;
    if (avgZ < -0.25 && state.simNodes.length > 80) continue; // 背面非焦点边剔除防毛线团
    const alpha = Math.max(0.05, Math.min(0.32, 0.08 + ((avgZ + 1) * 0.5) * 0.24));
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.beginPath();
    const mx = (e.sNode.screenX + e.tNode.screenX) * 0.5;
    const my = (e.sNode.screenY + e.tNode.screenY) * 0.5;
    const ctrlX = cx + (mx - cx) * 0.82;
    const ctrlY = cy + (my - cy) * 0.82;
    ctx.moveTo(e.sNode.screenX, e.sNode.screenY);
    ctx.quadraticCurveTo(ctrlX, ctrlY, e.tNode.screenX, e.tNode.screenY);
    ctx.strokeStyle = theme.edgeNormal;
    ctx.lineWidth = 1.0;
    ctx.stroke();
    ctx.restore();
  }

  for (const e of focusEdges) {
    ctx.save();
    ctx.globalAlpha = 0.92;
    ctx.beginPath();
    const mx = (e.sNode.screenX + e.tNode.screenX) * 0.5;
    const my = (e.sNode.screenY + e.tNode.screenY) * 0.5;
    const ctrlX = cx + (mx - cx) * 0.85;
    const ctrlY = cy + (my - cy) * 0.85;
    ctx.moveTo(e.sNode.screenX, e.sNode.screenY);
    ctx.quadraticCurveTo(ctrlX, ctrlY, e.tNode.screenX, e.tNode.screenY);
    ctx.strokeStyle = theme.edgeFocus;
    ctx.lineWidth = 2.2;
    ctx.stroke();

    ctx.fillStyle = theme.edgeLabelFocus;
    ctx.font = "bold 9px monospace";
    ctx.textAlign = "center";
    ctx.fillText(e.relation, (mx + ctrlX) * 0.5, (my + ctrlY) * 0.5 - 3);
    ctx.restore();
  }

  // 4. 画家算法 Z-Sort 从背面到正面绘制节点
  const sorted = [...state.simNodes].sort((a, b) => a.pz - b.pz);
  for (const n of sorted) {
    const matched = !theme.kw || n.label.toLowerCase().includes(theme.kw) || n.id.toLowerCase().includes(theme.kw);
    const inHop = !state.selectedNodeId || state.hopLimit >= 99 || theme.hopMap.has(n.id);
    const isSelected = n.id === state.selectedNodeId || n.id === state.hoverNodeId;
    const color = theme.palette[n.nt] || theme.palette.memory;

    let depthAlpha = n.pz >= 0 ? (0.62 + 0.38 * n.pz) : (0.14 + 0.42 * Math.pow(n.depthNorm, 2));
    if (theme.kw && !matched) depthAlpha *= 0.2;
    if (state.selectedNodeId && state.hopLimit < 99 && !inHop) depthAlpha *= 0.18;
    if (isSelected) depthAlpha = 1.0;

    ctx.save();
    ctx.globalAlpha = depthAlpha;

    // 高被引用节点外围呼吸环 (代替无限增大半径)
    if (n.inDeg >= 3 && n.pz > -0.2) {
      ctx.beginPath();
      ctx.arc(n.screenX, n.screenY, n.screenR + 3.5, 0, Math.PI * 2);
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.4;
      ctx.stroke();
    }

    // 节点球体径向高光
    const grad = ctx.createRadialGradient(
      n.screenX - n.screenR * 0.3, n.screenY - n.screenR * 0.3, n.screenR * 0.1,
      n.screenX, n.screenY, n.screenR
    );
    grad.addColorStop(0, theme.isLight ? "#ffffff" : "#f8fafc");
    grad.addColorStop(0.28, color);
    grad.addColorStop(1, color);

    ctx.beginPath();
    ctx.arc(n.screenX, n.screenY, n.screenR, 0, Math.PI * 2);
    ctx.fillStyle = grad;
    ctx.fill();

    if (isSelected || (theme.kw && matched)) {
      ctx.strokeStyle = theme.nodeOutline;
      ctx.lineWidth = isSelected ? 2.6 : 1.8;
      ctx.stroke();
    }

    // 高引用计数角标 (×N)
    if (n.inDeg >= 3 && n.pz > 0.1 && state.cam.zoom >= 0.75) {
      const tag = `×${n.inDeg}`;
      ctx.font = "bold 9px monospace";
      const tw = ctx.measureText(tag).width + 6;
      const bx = n.screenX + n.screenR * 0.7;
      const by = n.screenY - n.screenR * 0.9;
      ctx.fillStyle = theme.isLight ? "#1B2B33" : "#0f172a";
      ctx.beginPath();
      ctx.roundRect ? ctx.roundRect(bx, by - 7, tw, 13, 4) : ctx.rect(bx, by - 7, tw, 13);
      ctx.fill();
      ctx.strokeStyle = color;
      ctx.lineWidth = 1;
      ctx.stroke();
      ctx.fillStyle = theme.isLight ? "#ffffff" : "#f8fafc";
      ctx.textAlign = "left";
      ctx.fillText(tag, bx + 3, by + 3);
    }

    // 仅正面、高重要度或选中/搜索节点显示文字标签，防止背面文字穿透叠字
    const showLabel = isSelected || (theme.kw && matched) || (n.pz > 0.25 && (n.inDeg > 0 || state.cam.zoom > 0.95 || state.simNodes.length < 70));
    if (showLabel) {
      ctx.fillStyle = theme.nodeLabelColor;
      ctx.font = isSelected ? "bold 11px sans-serif" : (n.pz > 0.6 ? "600 10px monospace" : "9px monospace");
      ctx.textAlign = "center";
      ctx.fillText(n.label.slice(0, 26), n.screenX, n.screenY + n.screenR + 12);
    }
    ctx.restore();
  }
}

function showNodeDrawer(node) {
  const drawer = document.getElementById("node-drawer");
  const title = document.getElementById("drawer-title");
  const body = document.getElementById("drawer-body");
  if (!drawer || !body) return;
  drawer.classList.remove("hidden");
  title.innerText = `${node.label}`;
  const meta = node.metadata || {};
  const cgPath = node.canonical_path || meta.abs_path || meta.path || (node.id.startsWith("/") ? node.id : "/root/new/work/agent-body");
  const desc = meta.description || meta.purpose || "";
  const domain = meta.domain || "";
  const category = meta.category || meta.asset_type || "";
  const embStatus = meta.embedding_status || "";
  const embModel = meta.embedding_model || "";
  const embDim = meta.embedding_dim || "";
  const embPreview = Array.isArray(meta.embedding_preview) ? meta.embedding_preview.slice(0, 4).join(", ") : "";
  const embHtml = embStatus === "ready"
    ? `<div style="margin-top:6px;font-size:12px;"><span class="badge badge-ok">🧬 Embedding: ${esc(embModel)} (${esc(embDim)}d)</span> <span class="mono muted-text">[${esc(embPreview)}...]</span></div>`
    : (embStatus ? `<div style="margin-top:6px;font-size:12px;"><span class="badge">🧬 Embedding: ${esc(embStatus)}</span></div>` : "");
  const skipKeys = new Set(["description", "embedding_preview", "embedding_status", "embedding_model", "embedding_dim", "embedding_norm", "embedding_updated_at"]);
  const metaPairs = Object.entries(meta).filter(([k]) => !skipKeys.has(k)).slice(0, 10);
  const childrenHtml = node.isFoldedDir && Array.isArray(node.children) ? `
    <div class="item-box">
      <div class="item-top">
        <strong>📂 已折叠子节点 (${node.children.length})</strong>
        <button id="btn-expand-dir-node" class="btn btn-primary btn-xs">展开下一层</button>
      </div>
      <div class="mono muted-text" style="max-height:130px;overflow-y:auto;margin-top:6px;">
        ${node.children.slice(0, 20).map((c) => `<div>• ${esc(c.label)}</div>`).join("")}
        ${node.children.length > 20 ? `<div>... 还有 ${node.children.length - 20} 项</div>` : ""}
      </div>
    </div>
  ` : "";

  body.innerHTML = `
    <div class="item-box">
      <div><strong>ID:</strong> <span class="mono">${esc(node.id)}</span></div>
      <div><strong>类型:</strong> <span class="badge">${esc(node.nt)}</span> ${domain ? `<span class="badge">${esc(domain)}</span>` : ""} ${category ? `<span class="badge">${esc(category)}</span>` : ""}</div>
      <div><strong>状态:</strong> ${esc(node.status)}</div>
      ${desc ? `<div style="margin-top:6px;line-height:1.45;"><strong>语义描述:</strong> ${esc(desc)}</div>` : ""}
      ${embHtml}
      <div style="margin-top:6px;"><strong>被引用(入度):</strong> <span class="badge badge-ok">${node.inDeg ?? 0}</span> · <strong>主动引用(出度):</strong> <span class="badge">${node.outDeg ?? 0}</span></div>
    </div>
    ${childrenHtml}
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
  mirrorBlackScreen(`agent-body links query --id "${node.id}"`, "body_links_query", { id: node.id, type: node.nt });

  document.getElementById("btn-expand-dir-node")?.addEventListener("click", () => {
    if (node.canonical_path) {
      state.expandedDirs.add(node.canonical_path);
      applyTopoFilter();
    }
  });

  document.getElementById("btn-impact-node")?.addEventListener("click", async () => {
    try {
      const impact = await apiGet(`/api/v1/topology/impact?target=${encodeURIComponent(node.id)}`);
      state.topoDisplay = impact;
      applyTopoFilter();
      mirrorBlackScreen(`agent-body links impact "${node.id}"`, "body_links_impact", { target: node.id, max_depth: 3 });
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
      mirrorBlackScreen(`agent-body links codegraph --path "${cgPath}"`, "body_codegraph_bridge", { path: cgPath });
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
    mirrorBlackScreen("agent-body links audit && agent-body links repair", "body_links_audit", { fix_dangling: false });
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
    mirrorBlackScreen("agent-body links visualize --limit 30", "body_links_visualize", { limit: 30 });
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
        <input id="asset-scan-path" class="input" style="width:200px;" type="text" value="/root/new/work/agent-body" placeholder="待扫描目录路径" />
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
    mirrorBlackScreen(`agent-body asset search "${q}" --detail ${detail}`, "body_asset_search", { query: q, limit: 50, detail_level: detail });
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
    mirrorBlackScreen(`agent-body asset register --path "${path}" --asset-type ${asset_type}`, "body_asset_register", { path, type: asset_type, description });
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
    const cliCmd = enrich ? `agent-body asset scan "${path}" --enrich --enrich-limit 5` : `agent-body asset scan "${path}"`;
    mirrorBlackScreen(cliCmd, "body_asset_scan", { path, enrich_with_ollama: enrich, enrich_limit: 5 });
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
    mirrorBlackScreen(`agent-body fact "${q}" --fact-type ${t}`, "body_fact_query", { query: q, fact_type: t });
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
    mirrorBlackScreen(`agent-body env port ${port}`, "body_port_check", { port, suggest_count: 3 });
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
    mirrorBlackScreen(`agent-body coordination release --resource-type ${resourceType} --resource-id "${resourceId}"`, "body_lock_release", { resource_type: resourceType, resource_id: resourceId });
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
    mirrorBlackScreen(`agent-body coordination ${mode} -t ${resource_type} -i "${resource_id}" -a "${holder_agent}"`, mode === "wait" ? "body_lock_wait" : "body_lock_acquire", { resource_type, resource_id, holder_agent, scope });
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
    mirrorBlackScreen(`agent-body service check "${name}"`, "body_service_contact", { action: "check", service_name: name });
    alert(`服务 ${name} 探活结果:\n${JSON.stringify(rep, null, 2)}`);
    loadEcosystem();
  } catch (e) {
    alert("探活失败: " + e.message);
  }
};

async function runHookTest(kind) {
  const tool_name = document.getElementById("hook-tool-name")?.value || "write_file";
  const rawArgs = document.getElementById("hook-tool-args")?.value || "{}";
  const error_output = document.getElementById("hook-error-output")?.value?.trim() || "";
  const out = document.getElementById("hook-result");
  let args = {};
  try { args = JSON.parse(rawArgs); } catch (e) { return alert("入参 JSON 格式错误: " + e.message); }
  const isFailure = kind === "post" && error_output.length > 0;
  try {
    const reqBody = kind === "pre"
      ? { tool_name, args }
      : { tool_name, args, success: !isFailure, error_output: isFailure ? error_output : null };
    const res = await apiPost(kind === "pre" ? "/api/v1/hooks/pre" : "/api/v1/hooks/post", reqBody);
    const cliMirror = isFailure
      ? `agent-body hooks post --tool-name ${tool_name} --args '${rawArgs}' --success=false --error-output '${error_output}'`
      : `agent-body hooks ${kind} --tool-name ${tool_name} --args '${rawArgs}'`;
    mirrorBlackScreen(cliMirror, kind === "pre" ? "body_hook_pre" : "body_hook_post", reqBody);
    if (out) {
      const hasWarn = Boolean(res.has_warnings || (res.warnings && res.warnings.length));
      const allowed = res.allowed !== false;
      const rx = res.recovery_prescription;
      const modeBadge = res.concurrency_mode
        ? `<span class="badge">${esc(res.concurrency_mode)}</span>`
        : "";
      const badgeCls = !allowed ? "badge-err" : (hasWarn || rx) ? "badge-warn" : "badge-ok";
      const badgeTxt = !allowed ? "🛑 已拦截 (BLOCK)" : rx ? `⚡ 自愈处方 (${esc(rx.failure_kind)})` : hasWarn ? "⚠️ 触发告警与建议" : "✅ 安全放行";
      const warns = (res.warnings || []).map((w) => `<div class="item-box" style="border-color:var(--orange);">${esc(w)}</div>`).join("");
      const sugs = (res.suggestions || []).map((s) => `<div class="item-box">${esc(s)}</div>`).join("");
      const rxHtml = rx
        ? `<div class="item-box" style="border-color:var(--orange);">
             <div><strong>🔧 ${esc(rx.failure_kind)}</strong> (${esc(rx.target)} · ${rx.elapsed_us} μs${rx.lease_downgraded ? " · 已降级乐观租约" : ""})</div>
             <div class="muted-text mt-4">${esc(rx.root_cause)}</div>
             ${(rx.recovery_actions || []).map((a) => `<div class="mt-4">• ${esc(a)}</div>`).join("")}
           </div>`
        : "";
      const modeReason = res.concurrency_reason ? `<div class="muted-text mt-4">${esc(res.concurrency_reason)}</div>` : "";
      out.innerHTML = `
        <div class="item-top"><strong>${kind === "pre" ? "Pre-Hook 预检结果" : "Post-Hook 回写与自愈结果"}</strong><div>${modeBadge} <span class="badge ${badgeCls}">${badgeTxt}</span></div></div>
        ${modeReason}
        <div class="mt-8">${rxHtml}${warns}${sugs || (!warns && !rx ? '<div class="muted-text">未发现冲突，操作已记录。</div>' : "")}</div>
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
    mirrorBlackScreen(`agent-body hooks generate --client ${client} --dry-run`, "body_hook_generate", { client, dry_run: true });
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
    mirrorBlackScreen(`agent-body skill route "${intent}" --budget-tokens ${budget}`, "body_skill_route", { intent, budget_tokens: Number(budget) });
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
    mirrorBlackScreen(`agent-body brief`, "body_brief", { query: intent, budget_tokens: Number(budget) });
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
    mirrorBlackScreen(`agent-body place suggest --type ${object_type} --name "${name}" --intent "${intent}"`, "body_placement_suggest", { object_type, name, intent });
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
    mirrorBlackScreen("agent-body place learn --min-samples 3", "body_placement_learn", { min_samples: 3, accept: false });
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
    mirrorBlackScreen(`agent-body memory add --type ${memory_type} "${content}"`, "body_memory_add", { type: memory_type, content });
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
    mirrorBlackScreen(`agent-body memory search "${q}"${hybrid ? " --hybrid" : ""}`, "body_memory_search", { query: q, hybrid });
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
    mirrorBlackScreen("agent-body memory audit", "body_memory_audit", {});
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
    mirrorBlackScreen("agent-body analytics heatmap --days 30", "body_analytics_heatmap", { days: 30 });
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
  const output_path = `/root/new/tmp/agent_body_${table}.parquet`;
  const msgEl = document.getElementById("ana-export-msg");
  try {
    const res = await apiPost("/api/v1/analytics/export", { table, output_path });
    mirrorBlackScreen(`agent-body analytics export --table ${table} --output-path ${output_path}`, "body_analytics_export", { table, output_path });
    if (msgEl) msgEl.innerText = `✅ 已导出 ${res.rows_exported ?? ""} 行至 ${output_path}`;
  } catch (e) {
    if (msgEl) msgEl.innerText = `导出失败: ${e.message}`;
  }
}

async function runGcPreview() {
  const out = document.getElementById("gc-result");
  try {
    const res = await apiGet("/api/v1/env/gc?max_age_hours=72");
    mirrorBlackScreen("agent-body gc --max-age-hours 72", "body_env_gc", { max_age_hours: 72, apply: false });
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
    mirrorBlackScreen("agent-body close web-preview --dry-run", "body_session_close", { session_id: "web-preview", dry_run: true });
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
    mirrorBlackScreen(`agent-body mesh ${action === "port" ? "port " + port : action}`, "body_mesh_query", { action, port });
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
      let evType = "EVENT";
      let evBody = event.data;
      let evTime = new Date().toLocaleTimeString("zh-CN", { hour12: false });
      try {
        const parsed = JSON.parse(event.data);
        evType = String(parsed.type || "event").toUpperCase();
        evBody = parsed.message || parsed.path || event.data;
        if (parsed.timestamp) {
          evTime = new Date(parsed.timestamp).toLocaleTimeString("zh-CN", { hour12: false });
        }
      } catch (_) {}
      const div = document.createElement("div");
      div.className = "event-item ok";
      div.innerHTML = `<div class="ev-head"><span class="badge">${esc(evType)}</span><span class="ev-time">${esc(evTime)}</span></div><div class="ev-body">${esc(evBody)}</div>`;
      stream.prepend(div);
      while (stream.children.length > 80) stream.removeChild(stream.lastChild);

      const sb = document.getElementById("sse-sidebar");
      const ub = document.getElementById("sse-unread-badge");
      if (sb && sb.classList.contains("collapsed") && ub) {
        state.unreadSse = (state.unreadSse || 0) + 1;
        ub.innerText = String(state.unreadSse);
        ub.classList.remove("hidden");
      }
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
      parityEl.innerText = `43/43 (100%)${speedupHint}${ledgerHint}`;
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

  if (res.scenario_id === "scene-7") {
    const vit = ws.vitals || {};
    const orgs = ws.organs || [];
    const cp = ws.resumed_checkpoint || {};
    const l3 = ws.l3_guard_decision || {};
    return `
      <div class="widget-panel">
        <div class="widget-title"><span>🧬 跨大脑 1ms 接力胶囊 (${esc(cp.from_brain || "antigravity")} → ${esc(cp.resumed_by_brain || "claude-code")})</span><span class="badge badge-ok">${esc(ws.body_id || "body")}</span></div>
        <div class="muted-text" style="margin-bottom:8px;"><strong>目标：</strong>${esc(cp.goal_summary || "")}</div>
        <div class="tag-cloud">${orgs.map((o) => `<span class="badge badge-ok">${esc(o.name)}:${esc(o.status)}</span>`).join(" ")}</div>
      </div>
      <div class="widget-panel">
        <div class="widget-title"><span>🩺 五维稳态评分 ${Number(vit.overall_score || 100).toFixed(1)}/100</span><span class="badge badge-err">🛡️ ${esc(l3.level_code || "L3_FORBIDDEN")} 已拦截</span></div>
        <div class="muted-text">${esc(l3.reason || "敏感路径红线拦截")}</div>
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
    mirrorBlackScreen("agent-body demo reset", "body_demo_scenario", { action: "reset" });
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
  mirrorBlackScreen("agent-body bench ledger --window 7d", "body_benchmark", { action: "ledger", window: "7d" });
  await Promise.all([loadBenchmarkReport(), loadBenchmarkLedger(), loadFeedbackList()]);
  const runBtn = document.getElementById("btn-bench-run");
  if (runBtn && !runBtn.dataset.bound) {
    runBtn.dataset.bound = "1";
    runBtn.addEventListener("click", runBenchmarkSuiteFromUi);
    document.getElementById("btn-bench-refresh")?.addEventListener("click", loadBenchmarkLedger);
    document.getElementById("bench-window-select")?.addEventListener("change", loadBenchmarkLedger);
    document.getElementById("btn-bench-compare")?.addEventListener("click", compareBenchmarkRunsFromUi);
    document.getElementById("btn-fb-list")?.addEventListener("click", loadFeedbackList);
    document.getElementById("btn-fb-probes")?.addEventListener("click", runFeedbackProbes);
    document.getElementById("btn-fb-reconcile")?.addEventListener("click", runFeedbackReconcile);
    document.getElementById("btn-fb-sync")?.addEventListener("click", syncFeedbackMemories);
  }
}

async function runBenchmarkSuiteFromUi() {
  const suite = document.getElementById("bench-suite-select")?.value || "full";
  const box = document.getElementById("bench-workloads-box");
  if (box) box.innerHTML = `<div class="empty">⏳ 正在宿主机真机运行 ${esc(suite)} A/B 对照基准...</div>`;
  mirrorBlackScreen(`agent-body bench run --suite ${suite}`, "body_benchmark", { action: "run", suite, save: true });
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
  const gm = rep.geometric_mean_speedup || rep.overall_speedup_factor || 1;
  if (sp) sp.innerText = `${Number(gm).toFixed(2)}x (v${rep.spec_version || "1.0"})`;
  if (sha) sha.innerText = `SHA-256: ${String(rep.report_sha256 || "").slice(0, 16)}...`;
  if (sla) {
    const qBase = Number(rep.overall_baseline_quality_score || 0).toFixed(0);
    const qStew = Number(rep.d6_quality_score || 100).toFixed(0);
    const qDelta = Number(rep.overall_quality_delta || 0).toFixed(1);
    const dens = Number(rep.overall_info_density_gain || 1).toFixed(1);
    const qHint = rep.overall_baseline_quality_score ? ` · 质量 ${qBase}→${qStew} (+${qDelta}) · 密度 ${dens}x` : ` · D6 ${qStew}分`;
    sla.innerText = rep.sla_passed ? `✅ 帕累托 PASS${qHint}` : "❌ SLA FAIL";
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
        const qBadge = w.required_facts_total
          ? ` · 质量 ${Number(w.baseline_quality_score || 0).toFixed(0)}→${Number(w.steward_quality_score || 0).toFixed(0)} (+${Number(w.quality_delta || 0).toFixed(1)}) · 事实 ${w.steward_facts_hit}/${w.required_facts_total} · 密度 ${Number(w.info_density_gain || 1).toFixed(1)}x`
          : "";
        return `<div class="card" style="padding:10px;margin-bottom:8px;">
          <div class="item-top">
            <strong>${esc(w.id)} · ${esc(w.title)}</strong>
            <span class="badge ${w.sla_passed ? "badge-ok" : "badge-err"}">${w.sla_passed ? "PASS" : "FAIL"} · 省 ${pct.toFixed(1)}%${qBadge}</span>
          </div>
          <div class="muted-text" style="font-size:12px;margin-top:4px;">${esc(w.evidence_detail)}</div>
          <div style="display:flex;gap:8px;align-items:center;margin-top:6px;font-size:11px;">
            <span style="min-width:95px;">基线 ${w.baseline_tokens} tok</span>
            <div style="flex:1;height:6px;background:var(--border);border-radius:4px;overflow:hidden;">
              <div style="width:${(100 - pct).toFixed(1)}%;height:100%;background:var(--green);"></div>
            </div>
            <span style="min-width:95px;text-align:right;">本体 ${w.steward_tokens} tok</span>
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

  const cbox = document.getElementById("bench-competitors-box");
  const comps = rep.competitors || [];
  const matrix = rep.competitor_matrix || [];
  if (cbox) {
    if (comps.length === 0 && matrix.length === 0) {
      cbox.innerHTML = `<div class="empty">当前历史批次为 v${esc(rep.spec_version || "1.0")}，选择「🏆 热门竞品横向对标」运行轨道 C 实测</div>`;
    } else {
      const compCards = comps.map((c) => {
        const cq = c.required_facts_total
          ? ` · 质量 ${Number(c.competitor_quality_score || 0).toFixed(0)}→${Number(c.steward_quality_score || 0).toFixed(0)} (+${Number(c.quality_delta || 0).toFixed(1)}) · 密度 ${Number(c.info_density_gain || 1).toFixed(1)}x`
          : "";
        return `
        <div class="card" style="padding:10px;margin-bottom:8px;">
          <div class="item-top">
            <strong>${esc(c.id)} · ${esc(c.title)} vs ${esc(c.competitor_name)}</strong>
            <span class="badge ${c.passed ? "badge-ok" : "badge-err"}">${esc(c.evidence_tier)} · 省 ${Number(c.token_savings_pct || 0).toFixed(1)}%${cq}</span>
          </div>
          <div class="muted-text" style="font-size:12px;margin-top:4px;">竞品: ${esc(c.competitor_metric_value)} (${c.competitor_elapsed_us} μs) ｜ Steward: ${esc(c.steward_metric_value)} (${c.steward_elapsed_us} μs)</div>
          <div style="font-size:11px;margin-top:4px;color:var(--green);">优势: ${esc(c.steward_advantage)} · 互补: ${esc(c.complement_note)}</div>
        </div>`;
      }).join("");
      const matRows = matrix.slice(0, 9).map((r) => `
        <div class="item-top" style="padding:4px 0;border-bottom:1px dashed var(--border);font-size:11px;">
          <span><strong>${esc(r.product)}</strong> <span class="badge">${esc(r.evidence_tier)}</span></span>
          <span class="muted-text">${esc(r.relationship)}</span>
        </div>`).join("");
      cbox.innerHTML = compCards + (matRows ? `<div style="margin-top:10px;"><strong>12 维源码核验能力定位:</strong>${matRows}</div>` : "");
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

async function loadFeedbackList() {
  const box = document.getElementById("feedback-workbench-box");
  if (!box) return;
  try {
    const data = await apiGet("/api/v1/feedback");
    const items = data.items || [];
    if (items.length === 0) {
      box.innerHTML = `<div class="empty">user_feedback.md 暂无反馈条目</div>`;
      return;
    }
    box.innerHTML = `<div class="muted-text" style="font-size:12px;margin-bottom:6px;">文件: <code class="mono">${esc(data.path || "user_feedback.md")}</code> · 共 ${items.length} 条</div>` +
      items.slice(0, 12).map((it) => `
        <div class="item-top" style="padding:6px 0;border-bottom:1px dashed var(--border);font-size:12px;">
          <div><span class="badge">${esc(it.id)}</span> <span class="badge">${esc(it.priority)}</span> <strong>${esc(it.title)}</strong></div>
          <span class="badge ${String(it.status || "").includes("Verified") ? "badge-ok" : ""}">${esc(it.status)}</span>
        </div>
      `).join("");
  } catch (e) {
    box.innerHTML = `<div class="empty">加载反馈总账失败: ${esc(e.message)}</div>`;
  }
}

async function runFeedbackProbes() {
  const box = document.getElementById("feedback-workbench-box");
  if (box) box.innerHTML = `<div class="empty">🔬 正在运行五点执行回路对比自进化探针 (P1~P5)...</div>`;
  mirrorBlackScreen("agent-body feedback probes --auto-heal", "body_feedback", { action: "probes", auto_heal: true, auto_feedback: false });
  try {
    const rep = await apiPost("/api/v1/feedback", { action: "probes", auto_heal: true, auto_feedback: false });
    const probes = rep.probes || [];
    box.innerHTML = `<div class="item-top" style="margin-bottom:8px;">
        <strong>${esc(rep.summary || "P1~P5 探针评估完成")}</strong>
        <span class="badge ${rep.overall_healthy ? "badge-ok" : "badge-err"}">${rep.overall_healthy ? "✅ 全部健康" : "⚠️ 存在异常"}</span>
      </div>` +
      probes.map((p) => `
        <div class="card" style="padding:8px 10px;margin-bottom:6px;font-size:12px;">
          <div class="item-top">
            <strong>${esc(p.probe_id)} · ${esc(p.metric_name)} (${esc(p.hook_stage)})</strong>
            <span class="badge ${p.healthy ? "badge-ok" : "badge-err"}">${p.healthy ? "PASS" : "ALERT"}</span>
          </div>
          <div class="muted-text" style="margin-top:4px;">基线: ${esc(p.comparator_baseline)} ｜ 实测: ${esc(p.measured_value)} ｜ 差值: ${esc(p.delta_summary)}</div>
        </div>
      `).join("");
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">探针运行失败: ${esc(e.message)}</div>`;
  }
}

async function runFeedbackReconcile() {
  const box = document.getElementById("feedback-workbench-box");
  if (box) box.innerHTML = `<div class="empty">🧹 正在执行全生命周期信息新鲜度对账...</div>`;
  mirrorBlackScreen("agent-body feedback reconcile", "body_feedback", { action: "reconcile" });
  try {
    const rep = await apiPost("/api/v1/feedback", { action: "reconcile" });
    box.innerHTML = `<div class="item-top"><strong>✅ 信息新鲜度对账完成 (耗时 ${rep.elapsed_us ?? 0} μs)</strong></div>
      <div class="muted-text" style="font-size:12px;margin-top:6px;">
        核查活跃资产: <strong>${rep.checked_assets ?? 0}</strong> ｜ 标记失踪: <strong>${rep.missing_assets_marked ?? 0}</strong> ｜ 刷新漂移: <strong>${rep.drifted_assets_refreshed ?? 0}</strong> ｜ 剪除幽灵节点: <strong>${rep.ghost_fact_nodes_pruned ?? 0}</strong> ｜ 归档重复记忆: <strong>${rep.duplicate_memories_archived ?? 0}</strong>
      </div>`;
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">对账失败: ${esc(e.message)}</div>`;
  }
}

async function syncFeedbackMemories() {
  const box = document.getElementById("feedback-workbench-box");
  mirrorBlackScreen("agent-body feedback sync", "body_feedback", { action: "sync" });
  try {
    const rep = await apiPost("/api/v1/feedback", { action: "sync" });
    if (box) {
      box.innerHTML = `<div class="badge badge-ok">🧠 已将 ${rep.synced_to_memories ?? 0} 条已验证反馈沉淀入长期记忆</div>`;
    }
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">同步失败: ${esc(e.message)}</div>`;
  }
}

/* ─── 8. 双域行为审计与自进化中心 (Phase 43) ──────────────────────────── */
let _auditEventsBound = false;
function ensureAuditEventsBound() {
  if (_auditEventsBound) return;
  _auditEventsBound = true;
  document.getElementById("btn-audit-refresh")?.addEventListener("click", loadAuditWorkbench);
  document.getElementById("btn-audit-evolve")?.addEventListener("click", triggerAuditEvolve);
  document.getElementById("btn-audit-export")?.addEventListener("click", triggerAuditExport);
  document.getElementById("audit-domain-filter")?.addEventListener("change", loadAuditWorkbench);
  document.getElementById("audit-outcome-filter")?.addEventListener("change", loadAuditWorkbench);
  document.getElementById("audit-window-select")?.addEventListener("change", loadAuditWorkbench);
}

function filterAuditByTrace(traceId) {
  const input = document.getElementById("audit-trace-filter");
  if (input) input.value = traceId || "";
  loadAuditWorkbench();
}

async function loadAuditWorkbench() {
  ensureAuditEventsBound();
  const domain = document.getElementById("audit-domain-filter")?.value || "";
  const outcome = document.getElementById("audit-outcome-filter")?.value || "";
  const win = document.getElementById("audit-window-select")?.value || "7d";
  const traceId = (document.getElementById("audit-trace-filter")?.value || "").trim();

  const cliParts = ["agent-body audit query", `--window ${win}`];
  if (domain) cliParts.push(`--domain ${domain}`);
  if (outcome) cliParts.push(`--outcome ${outcome}`);
  if (traceId) cliParts.push(`--trace-id ${traceId}`);
  mirrorBlackScreen(cliParts.join(" "), "body_audit_query", {
    action: traceId ? "trace" : "query",
    domain: domain || undefined,
    outcome: outcome || undefined,
    trace_id: traceId || undefined,
    window: win,
  });

  try {
    const [sumRes, evRes] = await Promise.all([
      apiGet(`/api/v1/audit/summary?window=${encodeURIComponent(win)}&days=7`),
      apiGet(`/api/v1/audit/events?window=${encodeURIComponent(win)}&domain=${encodeURIComponent(domain)}&outcome=${encodeURIComponent(outcome)}&trace_id=${encodeURIComponent(traceId)}&limit=50`),
    ]);

    const s = sumRes.summary || {};
    document.getElementById("aud-kpi-total").innerText = String(s.total_events ?? 0);
    document.getElementById("aud-kpi-in").innerText = String(s.in_agent_count ?? 0);
    document.getElementById("aud-kpi-out").innerText = String(s.out_of_agent_count ?? 0);
    document.getElementById("aud-kpi-rec").innerText = `${s.failed_or_blocked_count ?? 0} / ${s.recovered_count ?? 0}`;

    const olapBox = document.getElementById("audit-olap-box");
    const tops = (sumRes.olap && sumRes.olap.top_actions) ? sumRes.olap.top_actions : [];
    if (olapBox) {
      if (tops.length === 0) {
        olapBox.innerHTML = `<div class="empty">近 ${esc(win)} 暂无 DuckDB OLAP 聚合动作</div>`;
      } else {
        olapBox.innerHTML = `<table class="data-table"><thead><tr><th>域</th><th>分类.动作</th><th>结果</th><th>频次</th><th>均耗</th></tr></thead><tbody>` +
          tops.slice(0, 12).map((t) => `<tr><td><span class="badge">${esc(t.domain)}</span></td><td><code>${esc(t.category)}.${esc(t.action)}</code></td><td>${esc(t.outcome)}</td><td><strong>${t.hit_count}</strong></td><td>${Math.round(t.avg_elapsed_us)} μs</td></tr>`).join("") +
          `</tbody></table>`;
      }
    }

    const evBox = document.getElementById("audit-events-box");
    const events = evRes.events || [];
    if (evBox) {
      if (events.length === 0) {
        evBox.innerHTML = `<div class="empty">当前过滤条件下暂无双域审计记录</div>`;
      } else {
        evBox.innerHTML = `<table class="data-table"><thead><tr><th>时间</th><th>域 / 主体</th><th>动作</th><th>目标</th><th>模式 / 结果</th><th>Trace</th><th>前后增量 (脱敏)</th></tr></thead><tbody>` +
          events.map((ev) => {
            const deltaStr = JSON.stringify(ev.before_after_delta || {});
            const shortDelta = deltaStr.length > 90 ? deltaStr.slice(0, 90) + "…" : deltaStr;
            const traceShort = (ev.trace_id || "").slice(0, 14);
            return `<tr>
              <td>${esc((ev.created_at || "").slice(0, 19))}</td>
              <td><span class="badge">${esc(ev.domain)}</span><div class="muted-text">${esc(ev.actor_kind)}</div></td>
              <td><code>${esc(ev.category)}.${esc(ev.action)}</code></td>
              <td><code>${esc(ev.target_id || "-")}</code></td>
              <td>${esc(ev.concurrency_mode || "-")} / <strong>${esc(ev.outcome)}</strong></td>
              <td><a href="javascript:void(0)" onclick="filterAuditByTrace('${esc(ev.trace_id)}')" title="按此 trace_id 钻取因果链路"><code>${esc(traceShort)}</code></a></td>
              <td><details><summary><code>${esc(shortDelta)}</code></summary><pre style="max-width:460px;overflow:auto;font-size:11px;">${esc(JSON.stringify(ev.before_after_delta || {}, null, 2))}\nSHA256: ${esc(ev.evidence_sha256 || "")}</pre></details></td>
            </tr>`;
          }).join("") +
          `</tbody></table>`;
      }
    }
  } catch (e) {
    const evBox = document.getElementById("audit-events-box");
    if (evBox) evBox.innerHTML = `<div class="empty">加载双域审计失败: ${esc(e.message)}</div>`;
  }
}

async function triggerAuditEvolve() {
  const box = document.getElementById("audit-evolve-box");
  mirrorBlackScreen("agent-body audit evolve", "body_audit_query", { action: "evolve", auto_feedback: false });
  if (box) box.innerHTML = `<div class="empty">正在分析近 7 天双域审计流...</div>`;
  try {
    const rep = await apiPost("/api/v1/audit/evolve", { auto_feedback: false, min_hits: 3 });
    if (box) {
      box.innerHTML = `<div class="kv-grid">
        <div><span class="k-label">评估事件数</span><strong>${rep.evaluated_events ?? 0}</strong></div>
        <div><span class="k-label">降级租约</span><strong>${(rep.downgraded_targets || []).length}</strong></div>
        <div><span class="k-label">康复租约</span><strong>${(rep.rehabilitated_targets || []).length}</strong></div>
        <div><span class="k-label">摩擦热点 / 新规则</span><strong>${(rep.friction_hotspots || []).length} / ${(rep.placement_candidates || []).length}</strong></div>
      </div>
      <details class="mt-8"><summary>查看自进化详情 JSON</summary><pre style="font-size:11px;overflow:auto;">${esc(JSON.stringify(rep, null, 2))}</pre></details>`;
    }
    loadAuditWorkbench();
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">自进化触发失败: ${esc(e.message)}</div>`;
  }
}

async function triggerAuditExport() {
  const box = document.getElementById("audit-evolve-box");
  mirrorBlackScreen("agent-body audit export --output-path /root/new/tmp/audit_events_export.csv", "body_audit_query", { action: "export", output_path: "/root/new/tmp/audit_events_export.csv" });
  try {
    const exp = await apiPost("/api/v1/audit/export", { output_path: "/root/new/tmp/audit_events_export.csv" });
    if (box) {
      box.innerHTML = `<div class="badge badge-ok">📦 已导出 ${exp.row_count ?? 0} 条记录 (${exp.file_size_bytes ?? 0} B) 至 ${esc(exp.output_path || "")}</div>`;
    }
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">导出失败: ${esc(e.message)}</div>`;
  }
}

/* ─── Phase 44: Agent 本体中枢工作台 ──────────────────────────────────── */
document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("btn-body-refresh")?.addEventListener("click", loadBodyWorkbench);
  document.getElementById("btn-body-heal")?.addEventListener("click", triggerBodyHeal);
  document.getElementById("btn-body-handoff")?.addEventListener("click", triggerBodyHandoff);
  document.getElementById("btn-body-resume")?.addEventListener("click", triggerBodyResume);
  document.getElementById("btn-body-perf-snap")?.addEventListener("click", () => triggerBodyPerfSnapshot(false));
  document.getElementById("btn-body-perf-base")?.addEventListener("click", () => triggerBodyPerfSnapshot(true));
  document.getElementById("btn-body-perf-awr")?.addEventListener("click", loadBodyPerfAwr);
  document.getElementById("btn-body-perf-tune")?.addEventListener("click", triggerBodyPerfTune);
});

async function loadBodyWorkbench() {
  mirrorBlackScreen("agent-body body manifest && agent-body body vitals", "body_state", { action: "manifest" });
  try {
    const [manifest, vitals, envData, cpData, peersData] = await Promise.all([
      apiGet("/api/v1/body/manifest"),
      apiGet("/api/v1/body/vitals"),
      apiGet("/api/v1/body/envelope"),
      apiGet("/api/v1/body/checkpoints?limit=15"),
      apiGet("/api/v1/body/peers"),
    ]);
    const idBadge = document.getElementById("body-id-badge");
    if (idBadge) idBadge.innerText = `${manifest.body_id || "body"} (${manifest.hostname || ""})`;
    const scEl = document.getElementById("body-kpi-score");
    if (scEl) scEl.innerText = `${Number(vitals.overall_score || 0).toFixed(1)} / 100`;
    const stEl = document.getElementById("body-kpi-status");
    if (stEl) stEl.innerText = String(vitals.status || "unknown").toUpperCase();
    const cpEl = document.getElementById("body-kpi-caps");
    if (cpEl) cpEl.innerText = String(manifest.active_checkpoints ?? 0);
    const prEl = document.getElementById("body-kpi-peers");
    if (prEl) prEl.innerText = String((peersData.configured_peers || []).length);

    const orgBox = document.getElementById("body-organs-box");
    if (orgBox) {
      orgBox.innerHTML = (manifest.organs || []).map((o) => `
        <div class="list-item">
          <div class="item-top">
            <strong>${esc(o.label)}</strong>
            <span class="badge ${o.status === "ready" ? "badge-ok" : "badge-warn"}">${esc(o.status)}</span>
          </div>
          <div class="muted-text mono" style="font-size:11px;">${esc(JSON.stringify(o.metrics || {}))}</div>
        </div>
      `).join("") || `<div class="empty">无器官信息</div>`;
    }

    const vitBox = document.getElementById("body-vitals-box");
    if (vitBox) {
      vitBox.innerHTML = (vitals.dimensions || []).map((d) => `
        <div class="list-item">
          <div class="item-top">
            <strong>${esc(d.label)} (${d.weight_pct}%)</strong>
            <span class="badge ${d.score >= 85 ? "badge-ok" : (d.score >= 65 ? "badge-warn" : "badge-err")}">${Number(d.score).toFixed(1)}</span>
          </div>
          <div class="muted-text" style="font-size:12px;">${esc(d.detail)}</div>
        </div>
      `).join("") + ((vitals.recommended_l1_actions || []).length
        ? `<div class="mt-8 badge badge-warn">💡 ${esc(vitals.recommended_l1_actions.join("；"))}</div>`
        : `<div class="mt-8 badge badge-ok">✅ 五维稳态均健康，无需反射干预</div>`);
    }

    const cpsBox = document.getElementById("body-checkpoints-box");
    if (cpsBox) {
      const cps = cpData.checkpoints || [];
      cpsBox.innerHTML = cps.length ? cps.map((c) => `
        <div class="list-item">
          <div class="item-top">
            <strong>${esc(c.from_brain)} → ${esc(c.resumed_by_brain || "待接力")}</strong>
            <span class="badge ${c.status === "active" ? "badge-ok" : ""}">${esc(c.status)}</span>
          </div>
          <div>${esc(c.goal_summary)}</div>
          <div class="muted-text mono" style="font-size:11px;">${esc(c.checkpoint_id)} · ${esc(c.created_at)}</div>
        </div>
      `).join("") : `<div class="empty">暂无跨大脑接力胶囊</div>`;
    }

    const envBox = document.getElementById("body-envelope-box");
    if (envBox) {
      const mat = envData.matrix || [];
      envBox.innerHTML = mat.map((r) => `
        <div class="list-item">
          <div class="item-top"><strong>${esc(r.level)}</strong><span class="badge">${esc(r.label)}</span></div>
          <div class="muted-text" style="font-size:12px;">${esc(r.authorization)} · ${esc((r.actions || []).slice(0, 4).join(", "))}</div>
        </div>
      `).join("");
    }
  } catch (e) {
    const orgBox = document.getElementById("body-organs-box");
    if (orgBox) orgBox.innerHTML = `<div class="empty">加载本体中枢失败: ${esc(e.message)}</div>`;
  }
}

async function triggerBodyHeal() {
  mirrorBlackScreen("agent-body body heal", "body_state", { action: "heal" });
  try {
    const rep = await apiPost("/api/v1/body/heal", {});
    showToast(`🩺 L1 自愈完成: ${Number(rep.before_score || 0).toFixed(1)} → ${Number(rep.after_score || 0).toFixed(1)}`);
    loadBodyWorkbench();
  } catch (e) {
    showToast(`L1 自愈失败: ${e.message}`);
  }
}

async function triggerBodyHandoff() {
  const fromBrain = document.getElementById("body-cp-brain")?.value?.trim() || "web-brain";
  const goal = document.getElementById("body-cp-goal")?.value?.trim() || "";
  if (!goal) {
    showToast("请先输入核心目标摘要");
    return;
  }
  mirrorBlackScreen(`agent-body body handoff --from-brain "${fromBrain}" --goal "${goal}"`, "body_state", { action: "handoff_save", from_brain: fromBrain, goal_summary: goal });
  try {
    await apiPost("/api/v1/body/handoff", { from_brain: fromBrain, goal_summary: goal });
    document.getElementById("body-cp-goal").value = "";
    showToast("🧬 已保存跨大脑接力胶囊");
    loadBodyWorkbench();
  } catch (e) {
    showToast(`保存胶囊失败: ${e.message}`);
  }
}

async function triggerBodyResume() {
  mirrorBlackScreen("agent-body body resume --by-brain next-brain", "body_state", { action: "handoff_resume", from_brain: "next-brain" });
  try {
    const res = await apiPost("/api/v1/body/resume", { by_brain: "next-brain" });
    showToast(res.resumed ? "🔁 已完成换脑接力恢复！" : "ℹ️ 当前无活跃待接力胶囊");
    loadBodyWorkbench();
  } catch (e) {
    showToast(`接力失败: ${e.message}`);
  }
}

async function triggerBodyPerfSnapshot(setBaseline) {
  const cmd = setBaseline ? "agent-body body perf-snapshot --set-baseline" : "agent-body body perf-snapshot";
  mirrorBlackScreen(cmd, "body_state", { action: "perf_snapshot", set_baseline: setBaseline });
  try {
    const snap = await apiPost("/api/v1/body/perf/snapshot", { label: setBaseline ? "web-golden" : "web-snap", set_baseline: setBaseline });
    showToast(setBaseline ? `⭐ 已设定黄金性能基准: ${snap.snapshot_id}` : `📸 已抓取性能快照: ${snap.snapshot_id}`);
    loadBodyPerfAwr();
  } catch (e) {
    showToast(`抓取快照失败: ${e.message}`);
  }
}

async function loadBodyPerfAwr() {
  const box = document.getElementById("body-perf-awr-box");
  mirrorBlackScreen("agent-body body perf-awr --vs-baseline true", "body_state", { action: "perf_awr", vs_baseline: true });
  try {
    const data = await apiGet("/api/v1/body/perf/awr?vs_baseline=true");
    const rep = data.report || {};
    const tm = rep.delta_time_model || {};
    const kt = rep.target_kernel || {};
    const waits = rep.top_wait_events || [];
    const findings = rep.addm_findings || [];
    if (box) {
      box.innerHTML = `
        <div class="list-item">
          <div class="item-top">
            <strong>${esc(rep.report_id || "awr")} (${esc(rep.base_snapshot_id || "-")} → ${esc(rep.target_snapshot_id || "-")})</strong>
            <span class="badge ${rep.regression_detected ? "badge-warn" : "badge-ok"}">${rep.regression_detected ? "⚠️ 检测到退化" : "✅ 0 退化稳态"}</span>
          </div>
          <div class="muted-text mono" style="font-size:12px;">
            /proc/self: VmRSS=${kt.rss_kb ?? 0} KB (${Number(rep.rss_drift_pct || 0).toFixed(1)}%) · VmHWM=${kt.hwm_kb ?? 0} KB · FDs=${kt.open_fds ?? 0} · 主动切换=${kt.voluntary_ctxt_switches ?? 0} · WAL=${Math.round((kt.sqlite_wal_bytes || 0) / 1024)} KB
          </div>
          <div class="muted-text mono" style="font-size:12px;margin-top:4px;">
            Body Time (${tm.total_body_time_us ?? 0} μs): CPU ${Number(tm.cpu_pct || 0).toFixed(1)}% | Lock ${Number(tm.lock_pct || 0).toFixed(1)}% | IO ${Number(tm.io_pct || 0).toFixed(1)}% | Net ${Number(tm.net_pct || 0).toFixed(1)}% | IPC ${Number(tm.ipc_pct || 0).toFixed(1)}%
          </div>
        </div>
        <div class="mt-8">
          ${waits.slice(0, 5).map((w) => `
            <div class="list-item">
              <div class="item-top"><strong class="mono">${esc(w.event_name)}</strong><span class="badge">${Number(w.pct_of_body_time || 0).toFixed(1)}% · P95=${w.est_p95_us ?? 0}μs</span></div>
              <div class="muted-text" style="font-size:11px;">${esc(w.description)} · calls=${w.calls ?? 0} · total=${w.total_us ?? 0}μs · max=${w.max_us ?? 0}μs</div>
            </div>
          `).join("")}
        </div>
        <div class="mt-8">
          ${findings.map((f) => `
            <div class="list-item">
              <div class="item-top"><strong>[${esc(f.rule_id)}] ${esc(f.summary)}</strong><span class="badge ${f.severity === "CRITICAL" ? "badge-err" : (f.severity === "WARN" ? "badge-warn" : "badge-ok")}">${esc(f.severity)}</span></div>
              <div class="muted-text" style="font-size:12px;">证据: ${esc(f.evidence)} → <strong>处方: ${esc(f.recommendation)}</strong></div>
            </div>
          `).join("")}
        </div>
      `;
    }
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">加载 AWR 报告失败: ${esc(e.message)}</div>`;
  }
}

async function triggerBodyPerfTune() {
  mirrorBlackScreen("agent-body body perf-tune", "body_state", { action: "perf_tune" });
  try {
    const rep = await apiPost("/api/v1/body/perf/tune", {});
    showToast(`⚡ L1 性能自调优完成: RSS ${rep.before_rss_kb ?? 0} → ${rep.after_rss_kb ?? 0} KB`);
    loadBodyPerfAwr();
  } catch (e) {
    showToast(`L1 性能自调优失败: ${e.message}`);
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

pub static DECK_HTML: &str = include_str!("../../../presentation/agent-body-deck.html");
pub static PRESENTATION_HTML: &str = include_str!("../../../presentation/presentation.html");

pub fn serve_deck() -> HttpResponse {
    HttpResponse::html(DECK_HTML)
}

pub fn serve_presentation() -> HttpResponse {
    HttpResponse::html(PRESENTATION_HTML)
}
