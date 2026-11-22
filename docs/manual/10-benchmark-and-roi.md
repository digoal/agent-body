# 10 — 实证 Benchmark 对照评测与生产效能存证账本

> 对应规约：[README.md 规约 #18](../../README.md) · 架构规格：[10-benchmark-spec.md](../design_doc/10-benchmark-spec.md) · 当前标准：**`Benchmark v3.8`**

---

## 一、为什么需要三轨实证体系？

AI 基础设施最容易陷入“自说自话”的宣传陷阱：写死几个漂亮的百分比，却拿不出同机对照数据和日常流水凭证。**Agent Body（Agent 本体）** 通过 **三轨实证闭环** 确保每一项收益都 100% 有据可查：

1. **轨道 A — 实验室可复现 A/B 对照基准 (`BenchmarkEngine`)**：
   - 在同一宿主机上真实运行 **Without Steward（裸奔等价命令/遍历/绑定碰撞）** 与 **With Steward（Agent 本体引擎）**；
   - 覆盖 **18 大标准宿主业务负载（`W1`~`W18`）**、**6 大内核微基准 SLA（`M1`~`M6`）** 与 **9 大现网数据及 AWR 稳态质量门禁（`DQ1`~`DQ9`）**，全量满足「既省 Token × 上下文质量不降反升」帕累托双门禁；
   - 每次跑批输出带宿主机指纹与 **SHA-256 防篡改签名** 的报告并落库 `benchmark_runs`。
2. **轨道 B — 生产运行态效能存证账本 (`EffectivenessLedger`)**：
   - 日常每一次 MCP/CLI/Web/Hook 调用与分级扫描都会以 `<100 μs` 极低开销自动记账至 `effectiveness_ledger`；
   - 每笔收益绑定确定性推导规则 **`PR-01` ~ `PR-14`** 与可读溯源依据 `evidence_summary`，随时可反算核验。
3. **轨道 C — 热门开源竞品横向同机对标 (`COMP-C1` ~ `COMP-C4`)**：
   - 在记忆（Mem0/Letta/PowerContext/Shodh-Memory）、上下文数据库（OpenViking）、Agent 文件系统（AgentFS）与宿主探针（`rg`+`psutil`+`ss`+`git`）四大开源赛道同机实跑对比，严格区分 `LIVE_BENCHMARKED` 与 `ARCH_VERIFIED`。

---

## 二、CLI 快速实操（`agent-body bench`）

```bash
# 1. 查看 18 大标准负载与 PR-01~14 推导规则字典
agent-body bench workloads

# 2. 在当前宿主机真机跑批（W1~W18 + M1~M6 + DQ1~DQ9 + COMP-C1~C4）并落库存证
agent-body bench run --suite full --save true

# 3. 单独运行竞品横向对标
agent-body bench competitors --track all

# 4. 查看最近一次实证报告（Markdown 表格 + SHA-256 存证指纹）
agent-body bench report

# 5. 审计过去 7 天的生产效能存证账本（按 PR 规则聚合 + 逐笔明细）
agent-body bench ledger --window 7d --limit 20

# 6. 跨版本 / 跨批次性能与 Token 节省回归对比
agent-body bench compare <run_a_id> <run_b_id>

# 7. 导出可发表 Markdown / JSON / CSV 报告
agent-body bench export --format markdown --out /tmp/agent-body-bench.md

# 8. 清理 bench:* 隔离测试沙盒记录
agent-body bench reset
```

---

## 三、第 40 个原生 MCP 工具：`body_benchmark`

```json
{
  "name": "body_benchmark",
  "arguments": {
    "action": "run",
    "suite": "full",
    "save": true
  }
}
```

支持 `action`: `"workloads" | "run" | "report" | "ledger" | "compare" | "export" | "reset" | "competitors"`，与 CLI 和 Web UI「📊 实证基准与账本」工作台保持 **43/43 (100%)** 双向同步。

---

## 四、Benchmark 标准 v2.0 演进（Phase 31）

- **`spec_version = "2.0"`**：兼容读取 `v1.0` 历史评测批次，`compare_runs` 自动标注口径版本变化；
- **几何平均加速比（Geomean）+ 分组加速比**：主指标采用几何平均加速比，并独立展示 `Query/Guard` 子集加速比；
- **新增 D6 认知质量与准确率维度**：
  - `W9`（`hybrid_semantic_recall`）：混合语义检索 `Recall@5 >= 90%`；
  - `W10`（`link_precision_regression`）：`CON-02` 模板/占位符/`.json` 抗假阳性抽取 `Precision >= 95%`；
  - `DQ1 ~ DQ3`（宿主机数据质量门禁）：资产描述覆盖率 `>= 99%`、本地向量覆盖率 `>= 95%`、依赖拓扑非假阳性健康度 `>= 85%`。
- **全渠道 `PR-01 ~ PR-10` 生产账本**：覆盖 MCP、CLI（`brief`/`search`/`fact`/`links audit`/`feedback`）与 Web REST API。

---

## 五、Benchmark 标准 `v3.0 ~ v3.3` 演进（Phase 35 ~ 39）

- **`v3.0` 横向竞品轨道 C (`COMP-C1 ~ COMP-C4`)**：引入长期记忆、上下文数据库、AgentFS 与宿主机 CLI 探针四大开源赛道同场实跑，区分 `LIVE_BENCHMARKED` 与 `ARCH_VERIFIED`。
- **`v3.1` 帕累托质量双门禁**：为全部 `W1 ~ W11` 与 `COMP-C1 ~ COMP-C4` 实装四维上下文质量评分（$Q = 40\% R_{\text{fact}} + 30\% P_{\text{signal}} + 20\% A_{\text{action}} + 10\% S_{\text{safety}}$）与有效信息密度倍数（`Facts / 1k Tokens`），确保“既省 Token 又提质量”。
- **`v3.2` 物理健康度与等价口径校准**：`DQ3` 与 `LinkAuditor` 物理断链口径 100% 拉齐，`W10` 扩充 Rust 测试/`#[cfg(test)]` 与 `.md` 示例抗误报门禁，并校准 `COMP-C4` 等价进程/代码检索采集口径。
- **`v3.4` 分层加权图谱与 Mesh 安全协同门禁（Phase 40 ~ 41）**：新增 `W13`（`L2` 上卷 + `L3` 向量隐式语义发现与加权衰减爆炸半径）、`W14`（`MeshEnvelope v1.0` HMAC 防重放与沙盒收件箱投递）与 `DQ6`（现网工程图谱连通度与分层边健康度门禁）。
- **`v3.5` AOPCC 并发回路门禁（Phase 42 · ISSUE #25）**：新增 `W15`（`optimistic_vs_pessimistic_execution_loop`：同场对比全悲观前置查询 vs 裸奔盲猜 vs AOPCC 乐观直通+碰壁秒级自愈，考核整轮对话税 `rounds_saved` 与 100% 事实恢复）与 `PR-11`（乐观直通租约命中与 `<2ms` 自愈处方存证规则）。
- **`v3.6` 双域行为审计与自进化飞轮门禁（Phase 43 · ISSUE #26）**：新增 `W16`（`dual_domain_audit_and_lease_self_evolution`：验证 `IN_AGENT` 脱敏防泄露、`Watcher` 60s 折叠 10→1、`OUT_OF_AGENT` 守护巡检留痕与 AOPCC 租约滑窗升降级）、`DQ7`（`audit_completeness_and_redaction_hygiene`）与 `PR-12`（`AuditDrivenSelfEvolution`）。
- **`v3.7` Agent 本体连续性与五维稳态门禁（Phase 44 · ISSUE #27）**：新增 `W17`（`cross_brain_continuity_and_body_proprioception`：验证大脑 A→B `<=300 tok` 脱敏抗投毒接力胶囊、6 器官 `BodyManifest` 与 `L3_FORBIDDEN` 包络拦截，确保 `rounds_saved >= 2` 与 `+ΔQ > 0`）、`DQ8`（`body_homeostasis_and_envelope_hygiene`：校验五维稳态评分、零泄露及 `/etc/shadow` 拦截率 100%）与 `PR-13`（`CrossBrainContinuitySaved`）。
- **`v3.8` 内生 Wait Events、AWR 基线与 ADDM 自调优门禁（Phase 50~54 · ISSUE #32, #38）**：新增 `W18`（`wait_events_awr_and_addm_tune`：验证 18/18 WaitEvent 全埋点采集、AWR 快照/黄金基线差分与 `L1` 自调优）、`M6`（`wait_event_probe_latency` 亚微秒无锁探针微基准）、`DQ9`（`kernel_telemetry_and_awr_integrity` 现网内核水位与 18/18 埋点覆盖检查）与 `PR-14`（`BodyPerfAwrAnalyzed`）。
