# Agent Host Steward (宿主环境大管家) 产品边界与治理准则

## 1. 核心定位 (Core Mission)
**Agent Host Steward** 是常驻于宿主机底层的“物理真实守护者与大管家”（Host Reality Ground-Truth & Governance Engine）。
它专为 Agent 生产级开发设计，解决当前自主 Agent 在裸机或容器内作业时面临的**四大核心顽疾**：
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

---

## 3. 治理设计原则 (Governance Principles)

### 原则一：实事求是，绝不捏造 (Ground Reality, Zero Simulation)
所有暴露给 Agent 的事实（端口、进程、Systemd、Cron、文件关联）必须源自真实 Linux 宿主机物理状态（`/proc`、`/sys`、`/etc/systemd`、`socket`、`inotify`）。严禁在缺少证据的情况下推测或编造虚拟状态。

### 原则二：只读探测为主，破坏性动作强制干预与防线 (Safety First)
1. 垃圾清理（GC）默认强制 `dry-run` 预览，绝不静默硬删除；强制限定白名单目录（如 `/root/new/tmp/`）。
2. Agent 执行动作前通过 `steward_hook_pre` 进行静态冲突预测（如端口已被监听、文件被重要服务依赖）；如果违规给出确定性告警阻断或提示。
3. 会话结清（`session_close`）提供可审计的结项清单，让临时资源随用随清。

### 原则三：极速与轻量化 (Sub-Millisecond Response)
Host Steward 是每位 Agent 高频调用的基石底座，核心 API（端口探测、规则放置匹配、属性图遍历、FTS 全文检索）端到端响应必须在毫秒级以内（实测基准：放置决策 120 μs，图遍历 498 μs，FTS 检索 805 μs），坚决避免成为 Agent 执行链路的性能瓶颈。

### 原则四：多引擎无缝演进 (Storage Pluggability)
单机研发默认零配置嵌入式 SQLite（免任何外部依赖开箱即用）；跨节点集群或企业级多 Agent 协作时，一键热迁移至 PostgreSQL（原生向量检索 `pgvector` + 全文分词 + JSONB），实现数据零丢失。
