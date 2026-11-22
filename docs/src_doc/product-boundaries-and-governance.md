# Agent Host Steward — Agent 本体（宿主机具身底座）产品边界与治理准则

## 1. 核心定位 (Core Mission)
**Agent Host Steward** 是常驻于宿主机底层的 **Agent 本体（Agent Embodiment Substrate）**——以“物理真实感官 × 规约脊柱与小脑反射 × 跨大脑海马体”为三位一体内核（原“宿主环境大管家 / 小秘书”为本体面向环境治理与比较优势协同的两大功能侧面）。
它专为 AI Agent 生产级开发设计，秉持 **「脑体分离」** 架构（LLM 为可随时切换的瞬态大脑，本工具为永驻宿主机的数字躯干），解决当前自主 Agent 在裸机或容器内作业时面临的**四大核心顽疾**：
1. **真实世界脱节 (Reality Disconnect)**：Agent 不清楚宿主机已安装什么工具、已监听什么端口、已有何种常驻系统服务，反复通过重型 Bash 命令试错探查。
2. **环境随意破坏 (Host Pollution & Conflict)**：随意建立临时目录、占用核心业务端口、遗留孤儿后台死循环进程。
3. **记忆虚幻漂移 (Memory Hallucination)**：缺乏物理可证的资产索引与跨会话事实图谱，记忆越积越乱且存在认知对立矛盾。
4. **爆炸半径盲区 (Blast Radius Ignorance)**：修改基础脚本或公用库时，不清楚哪些 systemd 服务或 cron 依赖它，导致无意中击穿已有业务流水线。

---

## 2. 什么是 Steward，什么不是 Steward (What We Are vs What We Are Not)

| 领域 / 职责 | Agent Host Steward 必须做 (In Scope) | Steward 明确不做 (Out of Scope / Non-Goals) |
| :--- | :--- | :--- |
| **任务编排** | 提供标准 Hook 接口与开工简报，告知 Agent 宿主约束 | **不替代** 上层 Agent 框架（如 LangGraph, AutoGen, CrewAI, Swarm）。Steward 不编排多 Agent 对话拓扑。 |
| **大模型推理** | 仅使用轻量 Embedding 模型或本地规则做向量化与文本匹配 | **不承担** 业务 LLM 思考与代码编写，绝不在管家内塞入庞大 Prompt 链。 |
| **代码执行与沙箱** | 提供真实环境状态审计与危险路径阻断 | **不替代** Docker / gVisor / Firecracker。Steward 面向拥有真实宿主操作权限的特权 Agent 进行辅助，而非做强制沙箱隔离。 |
| **存储中枢** | 通过 SPI 抽象提供 SQLite (单机嵌入) 与 PostgreSQL (分布式/企业级) 双引擎 | **不充当** 通用关系数据库或高并发 OLTP 业务数据库；仅管理 Steward 内部元数据、图谱与长期记忆。 |
| **机密凭据保管** | 识别敏感路径（如 .env, id_rsa）并标记安全保密级别 | **不替代** HashiCorp Vault 或 1Password。Steward 不负责密钥轮转与加解密托管。 |
| **宿主事实图谱** | 维护显式物理(`L1`)、结构聚合(`L2`)与稀疏语义(`L3`)三层有向加权拓扑并桥接 CodeGraph | **不伪造** 物理硬依赖；严禁将高维语义近邻混作确定性上游/下游故障依赖。 |
| **跨主机 Mesh** | 提供基于 `MeshEnvelope v1.0` 签名防重放认证的联邦查询、跨机锁、结构化对话与受控小产物(`<=10MB`)收件箱投递 | **不替代** Raft/Paxos 共识引擎、S3/Rsync 全量文件系统同步或人类聊天 IM；严禁跨机直写覆盖系统与业务路径。 |
| **双域审计与自进化** | 对 `IN_AGENT`（Brief/Hook/MCP/CLI/Web/Mesh）与 `OUT_OF_AGENT`（后台巡检/Watcher 60s 折叠/GC/断链修复/规则演化）全行为记录脱敏前后增量与 SHA-256 存证，用于 DuckDB OLAP 分析与驱动 AOPCC 租约/规则自进化 | **不替代** 企业级通用 SIEM/ELK 日志中心或应用层 APM 全量链路追踪系统（如 Datadog/Jaeger），仅审计宿主机管家双域关键治理与认知动作。 |
| **Agent 本体与跨大脑连续性** | 提供本体身份证与六器官自省 (`BodyManifest`)、`<300 tok` 脱敏抗投毒跨大脑接力胶囊 (`ContinuityCapsule`)、五维稳态体检与 `L1` 反射自愈 (`HomeostasisVitals`)、四级自治权限包络 (`L0~L3`) 及多本体只读名片发现 | **不替代** 全栈 Agent Runtime、主战 LLM ReAct 规划循环、模型路由代理或面向人类的个人私人助理。 |

---

## 3. 治理设计原则 (Governance Principles)

### 原则一：实事求是，绝不捏造 (Ground Reality, Zero Simulation)
所有暴露给 Agent 的事实（端口、进程、Systemd、Cron、文件关联）必须源自真实 Linux 宿主机物理状态（`/proc`、`/sys`、`/etc/systemd`、`socket`、`inotify`）。严禁在缺少证据的情况下推测或编造虚拟状态。

### 原则二：悲观红线前置干预 × 乐观直通后置自愈 (Adaptive Guard & Fail-Fast Recovery)
1. 垃圾清理（GC）默认强制 `dry-run` 预览，绝不静默硬删除；强制限定白名单目录（如 `/root/new/tmp/`）。
2. **悲观前置门禁 (`PESSIMISTIC_LOCK`)**：涉及启动监听端口、修改被 Systemd/Cron 依赖文件、高危敏感路径与跨 Agent 排他锁时，通过 `steward_hook_pre` 进行强前置冲突预测与阻断。
3. **乐观直通与碰壁自愈 (`OPTIMISTIC`)**：对可逆/低失败代价操作（如本地客户端直连 `:5432` PostgreSQL、调已安装 CLI 工具），优先通过 `steward_brief` 开工租约零前置查询直接执行；一旦失败由 `steward_hook_post` 在 `< 2ms` 内反查事实图谱注入替代处方。
4. 会话结清（`session_close`）提供可审计的结项清单，让临时资源随用随清；所有大管家在 Agent 之外对宿主对象的修改、修复与清理（`env gc`、`links repair --apply`、`feedback reconcile`、`daemon` 巡检）必须写入结构化行为审计流，确保因果与前后变化 100% 有据可查。

### 原则三：极速与轻量化 (Sub-Millisecond Response)
Host Steward 是每位 Agent 高频调用的基石底座，核心 API（端口探测、规则放置匹配、属性图遍历、FTS 全文检索）端到端响应必须在毫秒级以内（实测基准：放置决策 120 μs，图遍历 498 μs，FTS 检索 805 μs），坚决避免成为 Agent 执行链路的性能瓶颈。

### 原则四：多引擎无缝演进 (Storage Pluggability)
单机研发默认零配置嵌入式 SQLite（免任何外部依赖开箱即用）；跨节点集群或企业级多 Agent 协作时，一键热迁移至 PostgreSQL（原生向量检索 `pgvector` + 全文分词 + JSONB），实现数据零丢失。

### 原则五：纵深安全防御、全链路脱敏与抗投毒 (Defense-in-Depth & Zero-Secret-Leakage)
1. **凭据零落盘与全链路脱敏**：任何进入资产库、`/proc` 进程记录、长期记忆、变更审计流水与本地大模型输入的内容，必须经由统一 `SecretRedactor` 脱敏，严禁明文密钥/URI 口令落盘或跨 Mesh 传输。
2. **安全执行与最小暴露**：Hook 命令插值强制 Shell 安全转义，DuckDB 导出在引擎层锁定白名单目录，Web/Mesh 默认绑定 `127.0.0.1` 且写端点/GC 统一受 `auth_token` 保护。
3. **上下文抗投毒与最小文件权限**：外部仓库与第三方技能摘要在注入 `steward_brief` 前需识别隔离间接提示词注入；`~/.steward/` (`0700`) 与 `steward.db`/`config.toml` (`0600`) 权限纳入 `PreflightChecker` 强制基线。

### 原则六：双域全链路行为审计、防洪折叠与数据驱动自进化 (Dual-Domain Audit & Data-Driven Self-Evolution)
1. **双域全覆盖与可溯存证**：围绕 `IN_AGENT` 与 `OUT_OF_AGENT`（后台主动巡检、被动 Watcher 同步、宿主维护动作）统一构建脱敏审计事件流（`audit_events`），携带 `trace_id`、`before_after_delta` 与 `evidence_sha256`。
2. **去噪防洪与反馈自进化**：对 `inotify` 连续修改实施时间窗折叠避免淹没有效行为，并通过 DuckDB OLAP 分析与滑动窗口自进化引擎反哺 AOPCC 租约升降级、放置规则沉淀与缺陷自上报（详见 [16-dual-domain-audit-and-self-evolution-spec.md](../design_doc/16-dual-domain-audit-and-self-evolution-spec.md)）。

### 原则七：脑体分离、四级自治包络与三层命名兼容 (Brain-Body Separation, Autonomy Envelope & Naming Compatibility)
1. **脑体分离与防膨胀边界**：任意 LLM/客户端均视为瞬态大脑，本体严禁内置主战规划循环或抢夺思考主权，只做亚毫秒级感官、小脑反射、海马体接力与五维稳态代谢。
2. **四级自治权限包络 (`L0~L3`)**：显式区分 `L0_OBSERVE`（只读）、`L1_AUTONOMIC`（可逆稳态反射）、`L2_SUPERVISED`（宿主写操作默认 `dry-run` 或需授权）与 `L3_FORBIDDEN`（痛觉红线禁区），严禁越权自治。
3. **三层命名兼容纪律**：对外品牌与文档/注释全面升维为「Agent 本体」，工程标识（`agent-steward` / `steward_*` / `~/.steward/`）严格保持 100% 向后兼容（详见 [17-agent-embodiment-and-continuity-spec.md](../design_doc/17-agent-embodiment-and-continuity-spec.md)）。

---

## 4. 安全威胁边界与五层纵深防御规约 (Security Governance Matrix)

完整威胁建模（`SEC-GAP-01` ~ `SEC-GAP-05`）、`S1` ~ `S5` 安全规格与演进路线详见 [11-security-spec.md](../design_doc/11-security-spec.md) 与 [roadmap-phase32.md](roadmap-phase32.md)。

| 防御层级 | 核心机制 | 强制基线与对应阶段 |
|---|---|---|
| **L1 采集准入与脱敏** | `is_sensitive_file` + `shallow` 黑名单 + 全链路 `SecretRedactor` | 敏感文件零内容读取；`/proc` `cmdline`、记忆与变更流全脱敏 (`Phase 32.1`) |
| **L2 动作与执行门禁** | `steward_hook_pre` 高危路径告警 + Hook 参数转义 + 导出沙盒 | 拦截 `/etc/shadow`/`~/.ssh/` 高危写；防 `sh -c` 与 SQL 导出注入 (`Phase 32.2, 32.4`) |
| **L3 接口与联邦鉴权** | 默认 `127.0.0.1` + 全量写端点 `check_auth` + Mesh Bearer | 未带合法 Token 返回 `401`；非回环无 Token 拒绝写请求 (`Phase 32.3`) |
| **L4 本地存储与审计** | Unix `0700`/`0600` 最小权限 + `PreflightChecker` 安全探针 | 权限越界预检触发 `WARN` 修复提示；变更流水脱敏可追溯 (`Phase 32.5`) |
| **L5 抗投毒与安全基准** | `PromptInjectionScanner` 清洗 + `W11` / `DQ4` 安全回归 | 识别隔离外部语料越狱指令；现网明文密钥残留 `== 0%` (`Phase 33.1~33.2`) |
