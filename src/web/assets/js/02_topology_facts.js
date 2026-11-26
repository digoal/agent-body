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
  const wsMatch = norm.match(/^(\/[^/]+\/[^/]+)\/(work|shared_proj|src|shared_git|skills|note|cron|tmp)(?:\/(.*))?$/);
  if (wsMatch) {
    const base = wsMatch[1];
    const rel = wsMatch[3] ? `${wsMatch[2]}/${wsMatch[3]}` : wsMatch[2];
    const segs = rel.split("/").filter(Boolean);
    const depth = Math.max(1, segs.length);
    if (depth <= maxDepth) return { depth, ancestor: null };
    return { depth, ancestor: `${base}/${segs.slice(0, maxDepth).join("/")}` };
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
  const cgPath = node.canonical_path || meta.abs_path || meta.path || (node.id.startsWith("/") ? node.id : ".");
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
        <input id="asset-search-q" class="input flex-1" type="text" value="${esc(q)}" placeholder="Omni-RRF 检索纳管资产 (BM25/模糊/图谱/层级/互链)..." />
        <select id="asset-detail-level" class="input input-sm" style="width:104px;" title="Token 披露粒度">
          <option value="standard">standard</option>
          <option value="micro">micro</option>
          <option value="full">full</option>
        </select>
        <select id="asset-graph-hops" class="input input-sm" style="width:94px;" title="知识图扩散跳数">
          <option value="1">1-hop 图</option>
          <option value="2">2-hop 图</option>
          <option value="0">0-hop 图</option>
        </select>
        <button id="btn-asset-search" class="btn btn-primary btn-xs">🔍 检索资产</button>
        <input id="asset-scan-path" class="input" style="width:180px;" type="text" value="" placeholder="待扫描目录绝对路径" />
        <label style="font-size:12px;display:inline-flex;align-items:center;gap:4px;color:var(--muted);">
          <input id="asset-scan-mine-kg" type="checkbox" checked /> 🧠 知识建网
        </label>
        <label style="font-size:12px;display:inline-flex;align-items:center;gap:4px;color:var(--muted);">
          <input id="asset-scan-enrich" type="checkbox" /> 🤖 Ollama精炼
        </label>
        <button id="btn-asset-scan" class="btn btn-ghost btn-xs">📂 分级扫描</button>
      </div>
      <div class="inline-bar">
        <input id="asset-reg-path" class="input flex-1" type="text" placeholder="新资产绝对路径 (如 <workspace>/shared_proj/my_script.py)" />
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
  const graphHops = document.getElementById("asset-graph-hops")?.value || "1";
  const out = document.getElementById("asset-panel-out");
  if (!out) return;
  try {
    const list = await apiGet(`/api/v1/assets/search?q=${encodeURIComponent(q)}&limit=50&detail=${encodeURIComponent(detail)}&graph_hops=${encodeURIComponent(graphHops)}`);
    mirrorBlackScreen(`agent-body asset search "${q}" --detail ${detail} --graph-hops ${graphHops}`, "body_asset_search", { query: q, limit: 50, detail_level: detail, graph_hops: Number(graphHops) });
    out.innerHTML = `<table class="data-table"><thead><tr><th>类型 / 域 / 通道</th><th>路径</th><th>核心用途、匹配章节与推理路径</th></tr></thead><tbody>` +
      (list || []).map((a) => {
        const typ = a.type || a.asset_type || "other";
        const kv = a.desc_kv || {};
        const domain = a.domain || kv.domain || "-";
        const purpose = a.purpose || kv.purpose || a.description || "-";
        const caps = Array.isArray(kv.capabilities) ? kv.capabilities.map((c) => `<span class="badge badge-ok" style="margin-right:4px;">${esc(c)}</span>`).join("") : "";
        const channels = Array.isArray(a.matched_channels) ? a.matched_channels.map((ch) => `<span class="badge" style="margin-right:3px;font-size:10px;">${esc(ch)}</span>`).join("") : "";
        const symsArr = kv.symbols || kv.headings || kv.config_sections || [];
        const syms = Array.isArray(symsArr) && symsArr.length ? `<div class="mono" style="font-size:11px;color:var(--muted);margin-top:3px;">符号/章节: ${esc(symsArr.slice(0, 5).join(", "))}</div>` : "";
        const secs = Array.isArray(a.matched_sections) && a.matched_sections.length ? `<div style="font-size:11px;color:var(--accent);margin-top:3px;">📌 片段: ${esc(a.matched_sections[0])}</div>` : "";
        const rpaths = Array.isArray(a.reasoning_paths) && a.reasoning_paths.length ? `<div class="mono" style="font-size:11px;color:var(--muted);margin-top:2px;">🕸️ 推理: ${esc(a.reasoning_paths.slice(0, 2).join(" | "))}</div>` : "";
        const modelBadge = kv.annotation_model ? `<span class="badge">${esc(kv.annotation_model)}</span>` : "";
        return `<tr><td><span class="badge">${esc(typ)}</span> <span class="badge">${esc(domain)}</span> ${modelBadge}<div style="margin-top:4px;">${channels}</div></td><td class="mono">${esc(a.path)}</td><td><div>${esc(purpose)}</div><div style="margin-top:3px;">${caps}</div>${syms}${secs}${rpaths}</td></tr>`;
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
  const mine_knowledge = Boolean(document.getElementById("asset-scan-mine-kg")?.checked);
  const out = document.getElementById("asset-panel-out");
  try {
    const rep = await apiPost("/api/v1/assets/scan", { path, enrich_with_ollama: enrich, enrich_limit: 5, mine_knowledge });
    const cliCmd = `agent-body asset scan "${path}"${mine_knowledge ? " --mine-knowledge" : ""}${enrich ? " --enrich --enrich-limit 5" : ""}`;
    mirrorBlackScreen(cliCmd, "body_asset_scan", { path, enrich_with_ollama: enrich, enrich_limit: 5, mine_knowledge });
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

