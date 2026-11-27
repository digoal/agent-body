/* ─── Phase 44: Agent 本体中枢工作台 ──────────────────────────────────── */
document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("btn-body-refresh")?.addEventListener("click", loadBodyWorkbench);
  document.getElementById("btn-body-heal")?.addEventListener("click", triggerBodyHeal);
  document.getElementById("btn-body-handoff")?.addEventListener("click", triggerBodyHandoff);
  document.getElementById("btn-body-resume")?.addEventListener("click", triggerBodyResume);
  document.getElementById("btn-body-perf-snap")?.addEventListener("click", () => triggerBodyPerfSnapshot(false));
  document.getElementById("btn-body-perf-base")?.addEventListener("click", () => triggerBodyPerfSnapshot(true));
  document.getElementById("btn-body-perf-awr")?.addEventListener("click", () => generateSelectedBodyPerfAwr());
  document.getElementById("btn-body-perf-tune")?.addEventListener("click", triggerBodyPerfTune);
  document.getElementById("btn-body-perf-metrics")?.addEventListener("click", loadBodyPrometheusMetrics);
  document.getElementById("body-perf-report-select")?.addEventListener("change", (ev) => {
    const rid = ev.target.value;
    if (rid) loadSavedBodyPerfReport(rid);
  });
  document.getElementById("btn-body-perf-copy-md")?.addEventListener("click", copyBodyPerfMarkdown);
  document.getElementById("btn-body-perf-del-report")?.addEventListener("click", deleteSelectedBodyPerfReport);
  document.getElementById("btn-body-runtime-save")?.addEventListener("click", saveBodyRuntimeMode);
  document.getElementById("btn-body-ab-compare")?.addEventListener("click", runBodyAbCompare);
  document.getElementById("btn-body-ab-refresh")?.addEventListener("click", loadBodyRuntimeAndAb);
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
    loadBodyPerfAwr(true);
    loadBodyRuntimeAndAb();
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
  const btnId = setBaseline ? "btn-body-perf-base" : "btn-body-perf-snap";
  const btn = document.getElementById(btnId);
  const oldText = btn ? btn.innerText : "";
  const labelInput = document.getElementById("body-perf-snap-label");
  const customLabel = (labelInput?.value || "").trim() || (setBaseline ? "web-golden" : "web-snap");
  if (btn) { btn.disabled = true; btn.innerText = "⏳ 正在采样..."; }
  const cmd = setBaseline
    ? `agent-body body perf-snapshot --label "${customLabel}" --set-baseline`
    : `agent-body body perf-snapshot --label "${customLabel}"`;
  mirrorBlackScreen(cmd, "body_state", { action: "perf_snapshot", label: customLabel, set_baseline: setBaseline });
  try {
    const snap = await apiPost("/api/v1/body/perf/snapshot", { label: customLabel, set_baseline: setBaseline });
    const msg = setBaseline
      ? `⭐ 已抓取并设为黄金基准: ${snap.snapshot_id} [标签: ${snap.label}] (VmRSS=${snap.kernel_telemetry?.rss_kb ?? 0} KB, FDs=${snap.kernel_telemetry?.open_fds ?? 0})`
      : `📸 已抓取性能快照: ${snap.snapshot_id} [标签: ${snap.label}] (VmRSS=${snap.kernel_telemetry?.rss_kb ?? 0} KB, FDs=${snap.kernel_telemetry?.open_fds ?? 0})`;
    setPerfActionBanner(msg, false);
    showToast(msg, false);
    // 刷新快照下拉列表，并将新抓的快照自动设为结束快照 B（若设为基线则设为起始快照 A）
    await loadBodyPerfAwr(true, setBaseline ? snap.snapshot_id : null, !setBaseline ? snap.snapshot_id : null);
  } catch (e) {
    setPerfActionBanner(`❌ 抓取快照失败: ${e.message}`, true);
    showToast(`抓取快照失败: ${e.message}`, true);
  } finally {
    if (btn) { btn.disabled = false; btn.innerText = oldText; }
  }
}

function populateBodyPerfSelectors(data, preferSnapA, preferSnapB, selectedReportId) {
  const snaps = data.snapshots_meta || [];
  const baseMeta = data.baseline_meta || null;
  const reports = data.reports_meta || [];
  const rep = data.report || {};

  const selA = document.getElementById("body-perf-snap-a");
  const selB = document.getElementById("body-perf-snap-b");
  if (selA && selB) {
    const prevA = preferSnapA || selA.value;
    const prevB = preferSnapB || selB.value;

    const fmtSnap = (m) => {
      const tag = m.is_baseline ? "⭐[黄金基线] " : "";
      const timeShort = String(m.captured_at || "").replace("T", " ").slice(0, 19);
      return `${tag}${m.snapshot_id} (${m.label || "snap"} · ${timeShort} · RSS ${m.rss_kb ?? 0}KB)`;
    };

    let htmlA = "";
    if (baseMeta) {
      htmlA += `<option value="baseline">⭐ 黄金基准 (${esc(baseMeta.snapshot_id)} · ${esc(baseMeta.label)} · RSS ${baseMeta.rss_kb}KB)</option>`;
    }
    snaps.forEach((m) => {
      htmlA += `<option value="${esc(m.snapshot_id)}">${esc(fmtSnap(m))}</option>`;
    });
    selA.innerHTML = htmlA || `<option value="baseline">⭐ 黄金基准 (Golden Baseline)</option>`;

    let htmlB = `<option value="live">⚡ 实时当前态 (Live Current — 即时采样)</option>`;
    snaps.forEach((m) => {
      htmlB += `<option value="${esc(m.snapshot_id)}">${esc(fmtSnap(m))}</option>`;
    });
    selB.innerHTML = htmlB;

    // 恢复或智能预选两个不同的快照
    if (prevA && Array.from(selA.options).some((o) => o.value === prevA)) {
      selA.value = prevA;
    } else if (snaps.length >= 2) {
      selA.value = snaps[1].snapshot_id;
    }
    if (prevB && Array.from(selB.options).some((o) => o.value === prevB)) {
      selB.value = prevB;
    } else if (snaps.length >= 1) {
      selB.value = snaps[0].snapshot_id;
    }
  }

  const repSel = document.getElementById("body-perf-report-select");
  if (repSel) {
    const activeRid = selectedReportId || rep.report_id || "";
    if (reports.length === 0) {
      repSel.innerHTML = `<option value="">-- 暂无已保存历史报告 --</option>`;
    } else {
      repSel.innerHTML = reports.map((r) => {
        const st = r.regression_detected ? "⚠️退化" : "✅稳态";
        const ts = String(r.generated_at || "").replace("T", " ").slice(0, 19);
        return `<option value="${esc(r.report_id)}">[${esc(r.report_id)}] ${esc(r.base_snapshot_id)}(${esc(r.base_label || "-")}) → ${esc(r.target_snapshot_id)}(${esc(r.target_label || "-")}) | RSS ${Number(r.rss_drift_pct || 0).toFixed(1)}% | ${st} | ${esc(ts)}</option>`;
      }).join("");
      if (activeRid && Array.from(repSel.options).some((o) => o.value === activeRid)) {
        repSel.value = activeRid;
      }
    }
  }
}

function renderBodyPerfAwrReport(rep, snapshotsCount) {
  const box = document.getElementById("body-perf-awr-box");
  if (!box) return;
  window.__currentAwrMarkdown = rep.markdown_summary || "";
  const tm = rep.delta_time_model || {};
  const bk = rep.base_kernel || {};
  const tk = rep.target_kernel || {};
  const waits = rep.top_wait_events || [];
  const ops = rep.top_slow_operations || [];
  const findings = rep.addm_findings || [];
  const fmtDelta = (d, unit = "") => `${d >= 0 ? "+" : ""}${d}${unit}`;

  box.innerHTML = `
    <div class="list-item" style="border-left:3px solid ${rep.regression_detected ? "var(--orange)" : "var(--green)"};">
      <div class="item-top">
        <div>
          <strong style="font-size:14px;">📊 AWR 诊断报告: <code>${esc(rep.report_id || "awr")}</code></strong>
          <span class="muted-text" style="font-size:12px;margin-left:8px;">生成时间: ${esc(rep.generated_at || "-")} · 快照库: ${snapshotsCount} 份</span>
        </div>
        <span class="badge ${rep.regression_detected ? "badge-warn" : "badge-ok"}">${rep.regression_detected ? "⚠️ 检测到性能/资源水位退化" : "✅ 0 退化健康稳态"}</span>
      </div>

      <!-- 1. 起始快照 A vs 结束快照 B 核心指标对比表 -->
      <div class="mt-8" style="overflow-x:auto;">
        <table class="data-table" style="width:100%;font-size:12px;">
          <thead>
            <tr>
              <th>对比维度</th>
              <th>起始快照 A: <code>${esc(rep.base_snapshot_id || "-")}</code> (${esc(rep.base_label || "base")})</th>
              <th>结束快照 B: <code>${esc(rep.target_snapshot_id || "-")}</code> (${esc(rep.target_label || "target")})</th>
              <th>差分变化 (Δ B - A)</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><strong>采样时间</strong></td>
              <td class="mono">${esc(rep.base_captured_at || "-")}</td>
              <td class="mono">${esc(rep.target_captured_at || "-")}</td>
              <td class="mono">${esc(rep.mode || "-")}</td>
            </tr>
            <tr>
              <td><strong>进程驻留内存 (VmRSS)</strong></td>
              <td class="mono">${bk.rss_kb ?? "-"} KB</td>
              <td class="mono"><strong>${tk.rss_kb ?? 0} KB</strong> (${((tk.rss_kb || 0) / 1024).toFixed(1)} MB)</td>
              <td class="mono" style="color:${(rep.rss_delta_kb || 0) > 10240 ? "var(--orange)" : "var(--green)"};"><strong>${fmtDelta(rep.rss_delta_kb || 0, " KB")} (${Number(rep.rss_drift_pct || 0).toFixed(1)}%)</strong></td>
            </tr>
            <tr>
              <td><strong>历史峰值内存 (VmHWM) / 句柄数 (FDs)</strong></td>
              <td class="mono">HWM ${bk.hwm_kb ?? "-"} KB · ${bk.open_fds ?? "-"} FDs</td>
              <td class="mono">HWM ${tk.hwm_kb ?? 0} KB · <strong>${tk.open_fds ?? 0} FDs</strong></td>
              <td class="mono">FD ${fmtDelta(rep.fd_delta || 0)}</td>
            </tr>
            <tr>
              <td><strong>内核上下文切换 (主动 I/O 锁等待 / 被动 CPU 抢占)</strong></td>
              <td class="mono">主动 ${bk.voluntary_ctxt_switches ?? "-"} / 被动 ${bk.nonvoluntary_ctxt_switches ?? "-"}</td>
              <td class="mono">主动 ${tk.voluntary_ctxt_switches ?? 0} / 被动 ${tk.nonvoluntary_ctxt_switches ?? 0}</td>
              <td class="mono">主动 ${fmtDelta(rep.voluntary_cs_delta || 0)} / 被动 ${fmtDelta(rep.nonvoluntary_cs_delta || 0)}</td>
            </tr>
            <tr>
              <td><strong>SQLite WAL 日志尺寸 / 磁盘写流量</strong></td>
              <td class="mono">WAL ${Math.round((bk.sqlite_wal_bytes || 0) / 1024)} KB</td>
              <td class="mono">WAL ${Math.round((tk.sqlite_wal_bytes || 0) / 1024)} KB</td>
              <td class="mono">写字节 ${fmtDelta(rep.io_write_bytes_delta || 0, " B")}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 2. Body Time 五维分解条 -->
      <div class="mt-8" style="padding:8px 10px;background:var(--panel);border-radius:6px;border:1px solid var(--border);">
        <div class="item-top" style="font-size:12px;">
          <strong>⏱️ Body Time 时间模型分解 (总活跃耗时: <code>${tm.total_body_time_us ?? 0} μs</code>)</strong>
          <span class="mono">CPU <strong>${Number(tm.cpu_pct || 0).toFixed(1)}%</strong> (${tm.cpu_time_us ?? 0}μs) | Lock <strong>${Number(tm.lock_pct || 0).toFixed(1)}%</strong> (${tm.lock_wait_us ?? 0}μs) | IO <strong>${Number(tm.io_pct || 0).toFixed(1)}%</strong> (${tm.io_wait_us ?? 0}μs) | Net <strong>${Number(tm.net_pct || 0).toFixed(1)}%</strong> (${tm.net_wait_us ?? 0}μs) | IPC <strong>${Number(tm.ipc_pct || 0).toFixed(1)}%</strong></span>
        </div>
      </div>
    </div>

    <!-- 3. Top 等待事件与 ADDM 根因诊断双栏 -->
    <div class="panel-grid-2 mt-8">
      <div class="card" style="padding:10px;">
        <div class="item-top"><strong>⏳ Top 等待与执行事件 (Wait Events · 8 档对数分位桶)</strong></div>
        <div class="mt-8">
          ${waits.slice(0, 6).map((w) => `
            <div class="list-item">
              <div class="item-top"><strong class="mono">${esc(w.event_name)}</strong><span class="badge">${Number(w.pct_of_body_time || 0).toFixed(1)}% · P95=${w.est_p95_us ?? 0}μs</span></div>
              <div class="muted-text" style="font-size:11px;">${esc(w.description)} · calls=<strong>${w.calls ?? 0}</strong> · total=<strong>${w.total_us ?? 0}μs</strong> · avg=${w.avg_us ?? 0}μs · max=${w.max_us ?? 0}μs</div>
            </div>
          `).join("")}
        </div>
      </div>
      <div class="card" style="padding:10px;">
        <div class="item-top"><strong>🩺 ADDM 自动瓶颈诊断与调优处方 &amp; Top 操作耗时</strong></div>
        <div class="mt-8">
          ${findings.map((f) => `
            <div class="list-item">
              <div class="item-top"><strong>[${esc(f.rule_id)}] ${esc(f.summary)}</strong><span class="badge ${f.severity === "CRITICAL" ? "badge-err" : (f.severity === "WARN" ? "badge-warn" : "badge-ok")}">${esc(f.severity)}</span></div>
              <div class="muted-text" style="font-size:12px;">证据: ${esc(f.evidence)}</div>
              <div style="font-size:12px;margin-top:3px;color:var(--green);">👉 <strong>处方: ${esc(f.recommendation)}</strong></div>
            </div>
          `).join("")}
          ${ops.length ? `<div class="mt-8 muted-text" style="font-size:11px;font-weight:600;">Top 操作耗时 (24h audit_events):</div>` + ops.slice(0, 4).map((op) => `
            <div class="list-item" style="padding:5px 8px;">
              <div class="item-top"><span class="mono" style="font-size:11px;">${esc(op.operation)} (${esc(op.category)})</span><span class="mono" style="font-size:11px;">calls=${op.calls} · P95=${op.p95_us}μs · total=${op.total_us}μs</span></div>
            </div>
          `).join("") : ""}
        </div>
      </div>
    </div>

    <!-- 4. 完整 AWR Markdown 报告原文展示区 -->
    <div class="card mt-8" style="padding:10px;">
      <div class="item-top">
        <strong>📄 完整 AWR 性能诊断报告全文 (Markdown 原文 · 可直接复制归档)</strong>
        <span class="badge mono">${esc(rep.report_id || "")}</span>
      </div>
      <pre id="body-perf-md-pre" class="md-pre mt-8" style="max-height:320px;overflow-y:auto;padding:10px;background:var(--panel-2);border:1px solid var(--border);border-radius:6px;">${esc(rep.markdown_summary || "")}</pre>
    </div>
  `;
}

async function loadBodyPerfAwr(viewOnly, preferSnapA, preferSnapB) {
  const box = document.getElementById("body-perf-awr-box");
  try {
    const url = viewOnly ? "/api/v1/body/perf/awr?view_only=true" : "/api/v1/body/perf/awr?vs_baseline=true";
    const data = await apiGet(url);
    populateBodyPerfSelectors(data, preferSnapA, preferSnapB, data.report?.report_id);
    renderBodyPerfAwrReport(data.report || {}, (data.snapshots_meta || []).length);
  } catch (e) {
    setPerfActionBanner(`❌ 加载 AWR 报告失败: ${e.message}`, true);
    if (box) box.innerHTML = `<div class="empty">加载 AWR 报告失败: ${esc(e.message)}</div>`;
  }
}

async function generateSelectedBodyPerfAwr() {
  const btn = document.getElementById("btn-body-perf-awr");
  const oldText = btn ? btn.innerText : "";
  const snapA = document.getElementById("body-perf-snap-a")?.value || "baseline";
  const snapB = document.getElementById("body-perf-snap-b")?.value || "live";
  if (btn) { btn.disabled = true; btn.innerText = "⏳ 正在差分分析..."; }
  mirrorBlackScreen(
    `agent-body body perf-awr --snap-a "${snapA}" --snap-b "${snapB}"`,
    "body_state",
    { action: "perf_awr", snap_a: snapA, snap_b: snapB, vs_baseline: snapA === "baseline" }
  );
  try {
    const data = await apiPost("/api/v1/body/perf/awr", {
      snap_a: snapA,
      snap_b: snapB,
      vs_baseline: snapA === "baseline",
    });
    const rep = data.report || {};
    populateBodyPerfSelectors(data, snapA, snapB, rep.report_id);
    renderBodyPerfAwrReport(rep, (data.snapshots_meta || []).length);
    const msg = `📈 已生成并保存 AWR 差分报告 ${rep.report_id} [起点 ${rep.base_snapshot_id} (${rep.base_label}) → 终点 ${rep.target_snapshot_id} (${rep.target_label})]`;
    setPerfActionBanner(msg, false);
    showToast(msg, false);
  } catch (e) {
    setPerfActionBanner(`❌ 生成 AWR 差分报告失败: ${e.message}`, true);
    showToast(`生成 AWR 差分报告失败: ${e.message}`, true);
  } finally {
    if (btn) { btn.disabled = false; btn.innerText = oldText; }
  }
}

async function loadSavedBodyPerfReport(reportId) {
  mirrorBlackScreen(
    `agent-body body perf-awr --report-id "${reportId}"`,
    "body_state",
    { action: "perf_awr", report_id: reportId }
  );
  try {
    const data = await apiGet(`/api/v1/body/perf/awr?report_id=${encodeURIComponent(reportId)}`);
    const rep = data.report || {};
    populateBodyPerfSelectors(data, rep.base_snapshot_id, rep.target_snapshot_id, rep.report_id);
    renderBodyPerfAwrReport(rep, (data.snapshots_meta || []).length);
    const msg = `📜 已切换展示历史 AWR 报告 ${rep.report_id} (${rep.base_snapshot_id} → ${rep.target_snapshot_id})`;
    setPerfActionBanner(msg, false);
    showToast(msg, false);
  } catch (e) {
    setPerfActionBanner(`❌ 加载历史报告失败: ${e.message}`, true);
    showToast(`加载历史报告失败: ${e.message}`, true);
  }
}

async function deleteSelectedBodyPerfReport() {
  const repSel = document.getElementById("body-perf-report-select");
  const reportId = repSel?.value || "";
  if (!reportId) {
    showToast("请先选择要删除的历史 AWR 报告", true);
    return;
  }
  mirrorBlackScreen(
    `agent-body body perf-delete --kind report --id "${reportId}"`,
    "body_state",
    { action: "perf_delete", target_kind: "report", target_id: reportId }
  );
  try {
    const res = await apiPost("/api/v1/body/perf/delete", {
      kind: "report",
      id: reportId,
    });
    const deletedOk = res.result?.deleted;
    const msg = deletedOk
      ? `🗑️ 已删除 AWR 历史报告 ${reportId}`
      : `⚠️ 未找到或已删除 AWR 报告 ${reportId}`;
    setPerfActionBanner(msg, !deletedOk);
    showToast(msg, !deletedOk);
    await loadBodyPerfAwr(true);
  } catch (e) {
    setPerfActionBanner(`❌ 删除 AWR 报告失败: ${e.message}`, true);
    showToast(`删除 AWR 报告失败: ${e.message}`, true);
  }
}

async function copyBodyPerfMarkdown() {
  const md = window.__currentAwrMarkdown || document.getElementById("body-perf-md-pre")?.innerText || "";
  if (!md) {
    showToast("当前暂无可复制的 AWR 报告内容", true);
    return;
  }
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(md);
    }
    setPerfActionBanner(`📋 已复制完整 AWR Markdown 报告 (${md.length} 字符) 到剪贴板`, false);
    showToast(`📋 已复制 AWR Markdown 报告 (${md.length} 字符)`, false);
  } catch (e) {
    showToast(`复制失败: ${e.message}`, true);
  }
}

async function loadBodyPrometheusMetrics() {
  const box = document.getElementById("body-perf-metrics-box");
  if (!box) return;
  if (box.style.display === "block" && box.dataset.loaded === "1") {
    box.style.display = "none";
    return;
  }
  box.style.display = "block";
  box.innerHTML = `<div class="muted-text">⏳ 正在拉取 /metrics (OpenMetrics 0.0.4)...</div>`;
  try {
    const resp = await fetch("/metrics");
    const text = await resp.text();
    const traceId = resp.headers.get("X-Trace-Id") || "-";
    const traceparent = resp.headers.get("traceparent") || "-";
    box.dataset.loaded = "1";
    box.innerHTML = `<div class="item-top" style="flex-wrap:wrap;gap:6px;">
        <strong>📊 Prometheus OpenMetrics 0.0.4 实时暴露</strong>
        <span class="badge mono" style="font-size:11px;">X-Trace-Id: ${esc(traceId)}</span>
      </div>
      <div class="muted-text mono mt-4" style="font-size:11px;">traceparent: ${esc(traceparent)}</div>
      <pre class="md-pre mt-8" style="max-height:240px;overflow:auto;padding:8px;font-size:11px;background:var(--panel);border:1px solid var(--border);border-radius:6px;">${esc(text)}</pre>`;
  } catch (e) {
    box.innerHTML = `<div class="empty">拉取 /metrics 失败: ${esc(e.message)}</div>`;
  }
}

async function triggerBodyPerfTune() {
  const btn = document.getElementById("btn-body-perf-tune");
  const oldText = btn ? btn.innerText : "";
  if (btn) { btn.disabled = true; btn.innerText = "⏳ 调优中..."; }
  mirrorBlackScreen("agent-body body perf-tune", "body_state", { action: "perf_tune" });
  try {
    const rep = await apiPost("/api/v1/body/perf/tune", {});
    const msg = `⚡ L1 性能自调优完成 (${rep.elapsed_us ?? 0} μs): VmRSS ${rep.before_rss_kb ?? 0} → ${rep.after_rss_kb ?? 0} KB (释放 ${rep.rss_freed_kb ?? 0} KB) · 检查资产 ${rep.reconciled_assets_checked ?? 0}`;
    setPerfActionBanner(msg, false);
    showToast(msg, false);
    await loadBodyPerfAwr(true);
  } catch (e) {
    setPerfActionBanner(`❌ L1 性能自调优失败: ${e.message}`, true);
    showToast(`L1 性能自调优失败: ${e.message}`, true);
  } finally {
    if (btn) { btn.disabled = false; btn.innerText = oldText; }
  }
}

/* ─── Phase 58/63: 三态运行模式、MCP 档位与 Shadow A/B H1~H5 面板 ───── */
async function loadBodyRuntimeAndAb() {
  const statusBox = document.getElementById("body-runtime-status-box");
  const reportBox = document.getElementById("body-ab-report-box");
  const win = document.getElementById("body-ab-window")?.value || "7d";
  try {
    const [rt, rep] = await Promise.all([
      apiGet("/api/v1/body/runtime-mode"),
      apiGet(`/api/v1/body/ab-report?window=${encodeURIComponent(win)}`),
    ]);
    const modeSel = document.getElementById("body-runtime-mode-select");
    const profSel = document.getElementById("body-mcp-profile-select");
    const primSel = document.getElementById("body-shadow-primary-select");
    if (modeSel && rt.effective_mode) modeSel.value = rt.effective_mode;
    if (profSel && rt.effective_mcp_profile) profSel.value = rt.effective_mcp_profile;
    if (primSel && rt.shadow_primary) primSel.value = rt.shadow_primary;

    if (statusBox) {
      statusBox.innerHTML = `<div class="summary-kpi-row">
        <div class="summary-kpi"><span class="sk-label">生效运行模式</span><span class="sk-val c-cyan">${esc(rt.effective_mode || "-")}</span></div>
        <div class="summary-kpi"><span class="sk-label">MCP 暴露档位</span><span class="sk-val c-green">${esc(rt.effective_mcp_profile || "-")} (${rt.exposed_mcp_tools ?? "-"} / 43 工具)</span></div>
        <div class="summary-kpi"><span class="sk-label">主响应侧</span><span class="sk-val">${esc(rt.shadow_primary || "live")}</span></div>
        <div class="summary-kpi"><span class="sk-label">H1/H2 自动自愈</span><span class="sk-val">${rt.auto_heal_on_hindering === false ? "关闭" : "开启"}</span></div>
      </div>`;
    }

    if (reportBox) {
      const verdictBadge = rep.verdict === "NET_POSITIVE" ? "badge-ok" : (rep.verdict === "HURTING_INVESTIGATE" ? "badge-err" : "badge-warn");
      const recs = (rep.recommendations || []).map((r) => `<li>${esc(r)}</li>`).join("");
      const worst = (rep.worst_regressions || []).slice(0, 5).map((w) => {
        const flags = (w.hindering_flags || []).map((f) => `<span class="badge badge-warn" style="margin-right:4px;">${esc(f)}</span>`).join("");
        return `<div class="list-item"><div class="item-top"><strong class="mono">${esc(w.query)}</strong><span class="badge">${esc(w.verdict)} (auto_healed=${w.auto_healed_count ?? 0})</span></div><div>${flags}<span class="muted-text">${esc(w.reason || "")}</span></div></div>`;
      }).join("");
      reportBox.innerHTML = `<div class="item-box">
        <div class="item-top"><strong>📊 Shadow A/B 实证总评 (${esc(rep.window || win)})</strong><span class="badge ${verdictBadge}">${esc(rep.verdict || "-")}</span></div>
        <div class="summary-kpi-row mt-8">
          <div class="summary-kpi"><span class="sk-label">双轨对照样本</span><span class="sk-val">${rep.shadow_comparisons ?? 0}</span></div>
          <div class="summary-kpi"><span class="sk-label">本体胜率</span><span class="sk-val c-green">${Number(rep.win_rate_pct || 0).toFixed(1)}%</span></div>
          <div class="summary-kpi"><span class="sk-label">负面回退+陈旧率</span><span class="sk-val c-orange">${Number(rep.regression_rate_pct || 0).toFixed(1)}%</span></div>
          <div class="summary-kpi"><span class="sk-label">平均相对召回</span><span class="sk-val">${Number(rep.avg_recall_vs_live_pct || 0).toFixed(1)}%</span></div>
        </div>
        <div class="muted-text mt-8">${esc(rep.verdict_summary || "")}</div>
        ${recs ? `<ul class="mt-8" style="margin-left:18px;font-size:12px;">${recs}</ul>` : ""}
        ${worst ? `<div class="mt-8"><strong>最近负向/自愈明细 (H1~H5)</strong>${worst}</div>` : ""}
      </div>`;
    }
  } catch (e) {
    if (statusBox) statusBox.innerHTML = `<div class="empty">读取运行模式失败: ${esc(e.message)}</div>`;
  }
}

async function saveBodyRuntimeMode() {
  const mode = document.getElementById("body-runtime-mode-select")?.value || "body_only";
  const mcp_profile = document.getElementById("body-mcp-profile-select")?.value || "full";
  const shadow_primary = document.getElementById("body-shadow-primary-select")?.value || "live";
  mirrorBlackScreen(
    `agent-body body runtime-mode --mode ${mode} --mcp-profile ${mcp_profile} --shadow-primary ${shadow_primary}`,
    "body_state",
    { action: "runtime_mode", mode, mcp_profile, shadow_primary }
  );
  try {
    await apiPost("/api/v1/body/runtime-mode", { mode, mcp_profile, shadow_primary });
    showToast(`⚖️ 已保存运行模式: ${mode} (mcp_profile=${mcp_profile})`);
    await loadBodyRuntimeAndAb();
  } catch (e) {
    showToast(`切换运行模式失败: ${e.message}`, true);
  }
}

async function runBodyAbCompare() {
  const query = document.getElementById("body-ab-query")?.value?.trim() || "";
  const cmpBox = document.getElementById("body-ab-compare-box");
  if (!query) {
    showToast("请输入对照检索词 query", true);
    return;
  }
  mirrorBlackScreen(`agent-body body ab-compare "${query}" --limit 10`, "body_state", { action: "ab_compare", query, limit: 10 });
  try {
    const cmp = await apiPost("/api/v1/body/ab-compare", { query, limit: 10 });
    if (cmpBox) {
      const flags = (cmp.hindering_flags || []).length
        ? cmp.hindering_flags.map((f) => `<span class="badge badge-warn" style="margin-right:4px;">${esc(f)}</span>`).join("")
        : '<span class="badge badge-ok">无 H1~H5 帮倒忙</span>';
      cmpBox.innerHTML = `<div class="item-box">
        <div class="item-top"><strong>⚔️ 单次 A/B 实测: <span class="mono">${esc(cmp.query)}</span></strong><span class="badge">${esc(cmp.verdict)} (healed=${cmp.auto_healed_count ?? 0})</span></div>
        <div class="mt-8">${flags} · live=${cmp.live_elapsed_us}μs/${cmp.live_hit_count}hits vs body=${cmp.body_elapsed_us}μs/${cmp.body_hit_count}hits · recall=${cmp.recall_vs_live_pct}% · stale=${cmp.body_stale_count}</div>
        <div class="muted-text mt-4">${esc(cmp.reason || "")}</div>
      </div>`;
    }
    await loadBodyRuntimeAndAb();
  } catch (e) {
    if (cmpBox) cmpBox.innerHTML = `<div class="empty">对照失败: ${esc(e.message)}</div>`;
    showToast(`A/B 对照失败: ${e.message}`, true);
  }
}

