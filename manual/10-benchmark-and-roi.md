# 10 — 实证 Benchmark 对照评测与生产效能存证账本

> 对应规约：[README.md 规约 #18](../README.md#L282-L284) · 架构规格：[10-benchmark-spec.md](../design_doc/10-benchmark-spec.md)

---

## 一、为什么需要双轨实证体系？

AI 基础设施最容易陷入“自说自话”的宣传陷阱：写死几个漂亮的百分比，却拿不出同机对照数据和日常流水凭证。**Agent Host Steward** 通过 **双轨实证闭环** 确保每一项收益都 100% 有据可查：

1. **轨道 A — 实验室可复现 A/B 对照基准 (`BenchmarkEngine`)**：
   - 在同一宿主机上真实运行 **Without Steward（裸奔等价命令/遍历/绑定碰撞）** 与 **With Steward（大管家引擎）**；
   - 覆盖 **8 大标准宿主业务负载（`W1`~`W8`）** 与 **5 大内核微基准 SLA（`M1`~`M5`）**；
   - 每次跑批输出带宿主机指纹与 **SHA-256 防篡改签名** 的报告并落库 `benchmark_runs`。
2. **轨道 B — 生产运行态效能存证账本 (`EffectivenessLedger`)**：
   - 日常每一次 MCP 调用、Pre-Hook 拦截与分级扫描都会以 `<100 μs` 极低开销自动记账至 `effectiveness_ledger`；
   - 每笔收益绑定确定性推导规则 **`PR-01` ~ `PR-08`** 与可读溯源依据 `evidence_summary`，随时可反算核验。

---

## 二、CLI 快速实操（`agent-steward bench`）

```bash
# 1. 查看 8 大标准负载与 PR-01~08 推导规则字典
agent-steward bench workloads

# 2. 在当前宿主机真机跑批（W1~W8 + M1~M5）并落库存证
agent-steward bench run --suite full --save

# 3. 查看最近一次实证报告（Markdown 表格 + SHA-256 存证指纹）
agent-steward bench report

# 4. 审计过去 7 天的生产效能存证账本（按 PR 规则聚合 + 逐笔明细）
agent-steward bench ledger --window 7d --limit 20

# 5. 跨版本 / 跨批次性能与 Token 节省回归对比
agent-steward bench compare <run_a_id> <run_b_id>

# 6. 导出可发表 Markdown / JSON / CSV 报告
agent-steward bench export --format markdown --out /root/new/tmp/steward-bench.md

# 7. 清理 bench:* 隔离测试沙盒记录
agent-steward bench reset
```

---

## 三、第 40 个原生 MCP 工具：`steward_benchmark`

```json
{
  "name": "steward_benchmark",
  "arguments": {
    "action": "run",
    "suite": "full",
    "save": true
  }
}
```

支持 `action`: `"workloads" | "run" | "report" | "ledger" | "compare" | "export" | "reset"`，与 CLI 和 Web UI「📊 实证基准与账本」工作台保持 **40/40 (100%)** 双向同步。
