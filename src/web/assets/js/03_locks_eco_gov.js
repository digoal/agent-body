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
  const msgEl = document.getElementById("ana-export-msg");
  try {
    const res = await apiPost("/api/v1/analytics/export", { table });
    const actualOut = res.output_path || `<temp_dir>/agent_body_${table}.parquet`;
    mirrorBlackScreen(`agent-body analytics export --table ${table} --output-path ${actualOut}`, "body_analytics_export", { table, output_path: actualOut });
    if (msgEl) msgEl.innerText = `✅ 已导出 ${res.rows_exported ?? ""} 行至 ${actualOut}`;
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
      const scanned = res.tmp_dir_scanned || "temp_dir";
      out.innerHTML = `
        <div class="item-top"><strong>🧹 ${esc(scanned)} 扫描结果</strong><span class="badge badge-ok">Dry-Run 安全预览</span></div>
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

