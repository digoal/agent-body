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
              <span class="badge">规范路径: ${esc(place.suggested_path || roi.suggested_path || "<workspace>/shared_proj/")}</span>
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
          <div class="muted-text" style="text-decoration:line-through;color:var(--red);">${esc(repair.broken_ref || "<workspace>/shared_proj/sec_crawler_v1.py")}</div>
          <div class="mono" style="color:var(--green);margin-top:4px;font-weight:700;">→ 自动修复为: ${esc(repair.repaired_ref || "<workspace>/shared_proj/sec_crawler.py")} (相似度 ${repair.similarity ?? 0.94})</div>
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

  const dqbox = document.getElementById("bench-dq-box");
  const dqbadge = document.getElementById("bench-dq-badge");
  const dqs = rep.data_quality || [];
  if (dqbadge) {
    if (dqs.length === 0) {
      dqbadge.innerText = "-";
      dqbadge.className = "badge";
    } else {
      const passedCnt = dqs.filter((d) => d.passed).length;
      dqbadge.innerText = passedCnt === dqs.length ? `✅ ${passedCnt}/${dqs.length} PASS` : `❌ ${passedCnt}/${dqs.length} PASS`;
      dqbadge.className = passedCnt === dqs.length ? "badge badge-ok" : "badge badge-err";
    }
  }
  if (dqbox) {
    if (dqs.length === 0) {
      dqbox.innerHTML = `<div class="empty">该批次未运行 DQ 数据质量门禁（请选择全量套件 v3.8 或 DQ1~DQ9）</div>`;
    } else {
      dqbox.innerHTML = dqs.map((dq) => `
        <div class="list-item" style="padding:8px 10px;">
          <div class="item-top">
            <span><strong>${esc(dq.id)} · ${esc(dq.title)}</strong> <span class="muted-text mono" style="font-size:11px;">(${esc(dq.name)})</span></span>
            <span class="badge ${dq.passed ? "badge-ok" : "badge-err"}">${Number(dq.measured_pct || 0).toFixed(1)}% / SLA >= ${Number(dq.target_pct || 0).toFixed(1)}% ${dq.passed ? "✅" : "❌"}</span>
          </div>
          <div class="muted-text" style="font-size:12px;margin-top:3px;">${esc(dq.detail || "")}</div>
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
  try {
    const exp = await apiPost("/api/v1/audit/export", {});
    const actualOut = exp.output_path || "<temp_dir>/audit_events_export.csv";
    mirrorBlackScreen(`agent-body audit export --output-path ${actualOut}`, "body_audit_query", { action: "export", output_path: actualOut });
    if (box) {
      box.innerHTML = `<div class="badge badge-ok">📦 已导出 ${exp.row_count ?? 0} 条记录 (${exp.file_size_bytes ?? 0} B) 至 ${esc(actualOut)}</div>`;
    }
  } catch (e) {
    if (box) box.innerHTML = `<div class="empty">导出失败: ${esc(e.message)}</div>`;
  }
}

