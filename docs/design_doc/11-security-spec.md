# 11 — 安全架构、威胁建模与纵深防御规格说明书 (Security & Defense-in-Depth Spec)

> **Agent Body 产品设计 · 交付件 11**  
> **版本**: v0.4.0 → Phase 32~33 安全设计基线 · **日期**: 2026-11-20  
> **对齐约束**: [README.md 规约 #2、#5、#6、#8、#12、#18、#19](../../README.md) · [product-boundaries-and-governance.md](../src_doc/product-boundaries-and-governance.md) · [roadmap-phase32.md](../src_doc/archive/roadmap-phase32.md)

---

## 一、定位与安全边界：Agent 本体管什么安全、不管什么安全？

依据 [README.md 规约 #2（递弱代偿与产品边界）](../../README.md#L237-L239) 及 [product-boundaries-and-governance.md](../src_doc/product-boundaries-and-governance.md)，**Agent Body** 面向具备真实宿主操作权限的 AI Agent 提供系统认知底座。Agent 本体不越界替代基础设施级安全组件，但必须对自己纳管的全部数据链路与接口暴露面承担 **100% 纵深防御责任**。

| 安全职责维度 | Agent 本体必须兜底 (In Scope) | Agent 本体明确不替代 (Out of Scope) |
|---|---|---|
| **凭据与隐私防泄露** | 在文件扫描、进程 `/proc` 采集、MCP 配置解析、外部服务登记、长期记忆写入、变更审计流水与本地 Ollama 推理前，执行全链路 `SecretRedactor` 脱敏与敏感文件阻断。 | **不替代** HashiCorp Vault / 1Password；不负责企业级 KMS 密钥轮转与密码保管库托管。 |
| **宿主执行与路径保护** | 通过 `AgentHookEngine::pre_tool_call` 对高危系统路径与破坏性动作提供事前告警门禁；`HostGarbageCollector` 严格限制 `/tmp` 白名单与默认 `dry-run`；Hook 分发防命令注入与导出防穿越。 | **不替代** Docker / gVisor / Firecracker 容器沙箱或 Linux SELinux / AppArmor 内核强制访问控制。 |
| **接口访问控制与鉴权** | 默认绑定 `127.0.0.1` 回环地址；非回环暴露强制校验 `auth_token`；Web REST 写端点、敏感动作与 Peer Mesh 节点互通统一实施 Bearer Token 鉴权。 | **不替代** 企业级 OAuth2 / OIDC 网关或操作系统 PAM/RBAC 多租户权限系统。 |
| **本地存储最小权限** | 保证 `~/.agent-body/` 目录（`0700`）与 `body.db`、`config.toml`（`0600`）权限收敛，并通过 `PreflightChecker` 启动预检；全量状态纯本地存储，零遥测外传。 | **不替代** 全盘加密（LUKS）或外部关系数据库的 TDE 透明数据加密。 |
| **上下文抗投毒防线** | 对外部克隆源码（`/root/new/src/`）、第三方 `SKILL.md` 摘要及记忆入库语料识别并清洗间接提示词注入（Indirect Prompt Injection），保护注入 `body_brief` 的上下文纯净度。 | **不替代** 上层大模型推理框架自身的系统提示词安全护栏与输出对齐。 |

---

## 二、现网代码审计与威胁建模 (Threat Model & SEC-GAP-01~05)

基于对当前 `v0.4.0`（Phase 31 基线）源码的逐文件核查，识别出既有的零散安全防线与 **5 大真实安全缺口（`SEC-GAP-01` ~ `SEC-GAP-05`）**：

### 2.1 现有已落地防线盘点（Positive Baseline）
1. **敏感文件与黑名单目录阻断**：`src/scanner/metadata.rs` 的 `is_sensitive_file()` 拦截 `/.ssh/`、`/.gnupg/`、`.env*`、`id_rsa`、`id_ed25519`、`*.pem`/`*.key`/`*.p12`/`*.pfx` 及含 `secret`/`credential`/`password` 的配置文件；黑名单目录（`node_modules`、`.git`、`.agent-body`、`target` 等）通过 `shallow` 策略绝不下钻。
2. **局部脱敏能力**：`SecretRedactor::redact()` 在 `src/scanner/ollama_annotator.rs` 送入本地小模型前执行脱敏；`src/ecosystem/scanner.rs` 扫描客户端 MCP 配置时剥离 `env` 明文键值；`src/ecosystem/contacts.rs` 的 `validate_no_secret_leak()` 拒绝含 `sk-`/`ghp_`/`gho_`/`xoxb-`/`xoxp-`/`aiza` 及未参数化 `password=` 的服务登记，并将 PG URL 密码替换为 `***`。
3. **局部鉴权与 GC 红线**：`src/env/gc.rs` 强制拒绝不包含 `/tmp` 或 `temp` 的 GC 路径；`src/web/locks_api.rs`、`src/web/parity_api.rs` 的锁申请/等待与 `src/mesh/mod.rs` 的 `/api/v1/mesh/locks/acquire` 支持 `auth_token` Bearer 校验（其中 Mesh 写操作未配置 Token 时直接返回 `403 Forbidden`）。

### 2.2 待治理的 5 大安全缺口清单 (`SEC-GAP-01` ~ `SEC-GAP-05`)

| 缺口编号 | 严重度 | 源码事实定位 | 攻击面与泄露/破坏路径 |
|:---:|:---:|---|---|
| **`SEC-GAP-01`** | **P0 / CRITICAL** | `src/fact/process_net.rs:268-280`<br/>`src/env/mod.rs:120-149`<br/>`src/memory/mod.rs:29-94`<br/>`src/scanner/metadata.rs:216-263` | **进程命令行、记忆与变更流未全链路脱敏**：<br/>1. `/proc/<pid>/cmdline` 原文直采并写入 `active_processes.command`，进程启动参数里的 `--password=xxx`、`postgres://u:p@host`、`Bearer sk-...` 会明文落库并暴露给 `body_brief`、Web 拓扑与 `/api/v1/mesh/facts`；<br/>2. `MemoryManager::add` / `add_filtered` 与变更记录 `record_change` 未调用 `SecretRedactor::redact()`；<br/>3. `SecretRedactor` 尚不支持 URI 内嵌口令（`://user:pass@`）、私钥头（`-----BEGIN ... PRIVATE KEY-----`）、Bearer JWT 与 `--password`/`--token` 参数掩码。 |
| **`SEC-GAP-02`** | **P0 / CRITICAL** | `src/hooks/dispatcher.rs:147-185`<br/>`src/storage/duckdb_analytics.rs:238-274` | **Hook 命令注入与 DuckDB 导出底层校验缺失**：<br/>1. `HookDispatcher::dispatch()` 对 `HookAction::Exec` 采用简单字符串替换 `template.replace("{path}", path)` 后交由 `sh -c` 执行，若受监听目录出现恶意命名文件（如含 `;`、`$()`、反引号或换行符），将直接触发本地 Shell 命令注入；<br/>2. `DuckDbAnalyticsEngine::export_to_parquet()` 仅依赖 Web 层 `src/web/parity_api.rs:515` 检查 `/root/new/tmp/`，而引擎底层直接把 `output_path` 拼入 DuckDB SQL `COPY (...) TO '{}'`，CLI/MCP 或含单引号路径存在 SQL 拼接注入与任意路径覆盖写风险。 |
| **`SEC-GAP-03`** | **P0 / HIGH** | `src/web/server.rs:182-272`<br/>`src/web/parity_api.rs:546-686`<br/>`src/config/mod.rs:137-172` | **Web REST API 写操作与敏感端点鉴权覆盖不全**：<br/>`route_request_arc` 仅对锁操作（`/api/v1/locks/*`）与 Mesh 锁申请校验了 `auth_token`，而 `POST /api/v1/assets/scan`、`POST /api/v1/assets/register`、`POST /api/v1/memories/add`、`POST /api/v1/session/close`、`POST /api/v1/analytics/export`、`POST /api/v1/feedback`、`POST /api/v1/demo/run|reset`、`POST /api/v1/benchmark/run|reset`、`GET /api/v1/env/gc` 等写/重负载端点均未统一调用 `check_auth`；且当 `web.host = "0.0.0.0"` 而 `auth_token` 为空时无启动阻断或强告警。 |
| **`SEC-GAP-04`** | **P1 / HIGH** | `src/api/hooks.rs:68-159`<br/>`src/scanner/metadata.rs:420-577`<br/>`src/memory/intake.rs:26-90` | **高危路径门禁缺位与外部语料间接提示词注入**：<br/>1. `AgentHookEngine::pre_tool_call` 目前仅检测协作锁、端口冲突、Automation 依赖与放置规范，对修改 `/etc/shadow`、`/etc/sudoers`、`~/.ssh/authorized_keys`、`/boot/` 或破坏性命令参数缺少明确 `CRITICAL` 阻断/强告警规则；<br/>2. 扫描 `/root/new/src/` 外部开源仓库或第三方 `SKILL.md` 提取 `purpose`/`headings` 并注入 `body_brief` 时，缺少对隐藏恶意提示词（如 `Ignore previous instructions`、`<system>` 伪造标签）的清洗隔离。 |
| **`SEC-GAP-05`** | **P1 / MEDIUM** | `src/preflight/mod.rs:123-152`<br/>`src/config/mod.rs`<br/>`src/benchmark/` | **存储最小权限预检与安全基准测试缺失**：<br/>`PreflightChecker` 未检查 `~/.agent-body/`（应为 `0700`）及 `body.db` / `config.toml`（应为 `0600`）的文件属主与权限位；Benchmark `v2.0` 的 `W1~W10` 与 `DQ1~DQ3` 尚未覆盖脱敏零泄露、防命令注入与权限合规门禁。 |

---

## 三、五层纵深防御安全架构 (Defense-in-Depth Architecture)

```mermaid
flowchart TD
    In["外部输入与物理采集源<br/>(文件扫描 / /proc cmdline / MCP 配置 / 记忆 / Web 请求)"] --> L1

    subgraph L1["第一层: 采集准入与全链路脱敏网关 (Intake & Redaction)"]
        L1A["敏感文件阻断<br/>(is_sensitive_file + shallow 黑名单)"]
        L1B["全链路 SecretRedactor<br/>(KV / Token / URI口令 / 私钥 / cmdline)"]
        L1C["抗投毒清洗器<br/>(PromptInjectionScanner 隔离外部指令)"]
    end

    L1 --> L2

    subgraph L2["第二层: 动作与执行安全门禁 (Execution Guardrails)"]
        L2A["pre_tool_call 高危拦截<br/>(/etc/shadow / .ssh / 破坏性动作告警)"]
        L2B["Hook 参数化安全执行<br/>(POSIX 引号转义 / 禁用裸拼接)"]
        L2C["GC 与导出沙盒白名单<br/>(/tmp 限定 + dry-run + SQL 路径校验)"]
    end

    L2 --> L3

    subgraph L3["第三层: 接口与多机联邦鉴权 (Access & Transport Control)"]
        L3A["本地最小暴露<br/>(默认 127.0.0.1 / 0.0.0.0 强制 Token)"]
        L3B["Web 全量写接口鉴权<br/>(POST 写端点 & GC 统一 check_auth)"]
        L3C["Peer Mesh 双向令牌<br/>(Bearer auth_token + 超时熔断)"]
    end

    L3 --> L4

    subgraph L4["第四层: 本地存储与审计基线 (Storage & Audit)"]
        L4A["最小文件权限收敛<br/>(~/.agent-body 0700 / db & toml 0600)"]
        L4B["脱敏后审计流水<br/>(changes 表 & effectiveness_ledger)"]
    end

    L4 --> L5

    subgraph L5["第五层: 运行时预检与实证安全基准 (Verification & SLA)"]
        L5A["Preflight 安全基线探针<br/>(权限 / 绑定地址 / Token 检查)"]
        L5B["Benchmark 安全回归门禁<br/>(W11 脱敏防注入 + DQ4 安全卫生度)"]
    end
```

---

## 四、核心安全规格定义 (`S1` ~ `S5`)

### `S1` 全链路凭据零落盘与统一脱敏管道
1. **统一脱敏引擎扩展（`SecretRedactor`）**：
   - 在现有 KV 键值对（`api_key`/`password`/`auth_token` 等）与令牌前缀（`sk-`/`ghp_`/`gho_`/`glpat-`/`xoxb-`/`AKIA`）基础上，扩展覆盖：
     - **URI 内嵌凭据**：`scheme://user:password@host` → `scheme://user:<REDACTED>@host`；
     - **CLI 敏感标志**：`--password=...`、`--token=...`、`-p ...`（针对已知数据库/认证客户端）→ `--password=<REDACTED>`；
     - **Authorization / Bearer 头与 JWT**：`Bearer eyJ...` → `Bearer <REDACTED_TOKEN>`；
     - **PEM 私钥块**：`-----BEGIN ... PRIVATE KEY-----` 至结束标记整段剥离替换为 `<REDACTED_PRIVATE_KEY>`。
2. **强制挂载节点**：
   - `ProcessNetTopology::resolve_proc_details()`（`src/fact/process_net.rs`）与 `EnvCollector::collect_and_sync_processes()`（`src/env/mod.rs`）在构造 `command` 字段后立即执行 `SecretRedactor::redact_single_line()`；
   - `MemoryManager::add()` 与 `add_filtered()`（`src/memory/mod.rs`）在持久化前执行脱敏（或对高敏感内容强制拒绝入库）；
   - `Storage::record_change()` 写入 `changes` 表前对 `description` 与 `details` 执行脱敏，确保审计流水本身不成为二次泄露源。

### `S2` 宿主高危路径写保护与 Hook 安全执行规范
1. **Hook 分发防命令注入**：
   - `HookDispatcher::interpolate_command()`（`src/hooks/dispatcher.rs`）禁止直接把裸 `{path}` 字符串拼入 `sh -c`；必须对 `{path}` 与 `{event}` 执行严格的 POSIX Shell 单引号转义（`'` → `'\''`），并拒绝包含控制字符（`\0`、`\r`、`\n`）的路径。
2. **DuckDB 分析导出沙盒化**：
   - `DuckDbAnalyticsEngine::export_to_parquet()`（`src/storage/duckdb_analytics.rs`）必须在底层校验：
     - 规范化解析路径（禁止 `..` 与符号链接逃逸）；
     - 强制限定仅允许导出到 `/root/new/tmp/`、系统 `std::env::temp_dir()` 或配置的白名单目录；
     - 对 SQL 路径字面量执行单引号转义（`'` → `''`），杜绝 SQL 注入。
3. **Pre-Hook 高危路径与破坏性指令门禁**：
   - `AgentHookEngine::pre_tool_call()`（`src/api/hooks.rs`）增加静态高危路径清单（`/etc/shadow`、`/etc/sudoers`、`~/.ssh/`、`/boot/`、`/root/.agent-body/body.db` 直接文件写入），命中时注入 `[CRITICAL · 安全红线]` 告警与安全替代建议；识别到 `rm -rf /`、`mkfs`、`dd if=... of=/dev/`、`chmod -R 777 /` 等模式时触发高危预警。

### `S3` 接口暴露面收敛与统一写鉴权网关
1. **默认绑定最小化**：
   - `[web].host` 与 `api_bind` 默认严格保持 `"127.0.0.1"`。
   - 当配置 `host = "0.0.0.0"`（或非回环 IP）且 `[web].auth_token` 为 `None` 或空串时：
     - `Config::validate()` 与 `PreflightChecker` 明确发出 `WARN/FAIL` 安全告警；
     - `WebServer` 对外暴露模式在未配置 `auth_token` 时**强制拒绝所有写端点与 `/api/v1/env/gc` 请求（返回 `403 Forbidden`）**。
2. **全量写操作与敏感端点统一鉴权**：
   - 在 `src/web/server.rs` 与 `src/web/parity_api.rs` 中，对以下端点统一前置执行 `check_auth(req, auth_token)`：
     - 资产与扫描：`POST /api/v1/assets/scan`、`POST /api/v1/assets/register`
     - 记忆与反馈：`POST /api/v1/memories/add`、`POST /api/v1/feedback`
     - 会话与垃圾回收：`POST /api/v1/session/close`、`GET /api/v1/env/gc`
     - 分析与导出：`POST /api/v1/analytics/export`
     - 协作锁：`POST /api/v1/locks/acquire`、`POST /api/v1/locks/wait`、`POST /api/v1/locks/release`
     - 路演与基准重置/执行：`POST /api/v1/demo/run`、`POST /api/v1/demo/reset`、`POST /api/v1/benchmark/run`、`POST /api/v1/benchmark/reset`
     - 联邦节点：`POST /api/v1/mesh/locks/acquire`（已强制校验）
   - 保持常量时间比较（或明确 Bearer 格式解析），并防止 `auth_token` 出现在任何日志、简报与 API 响应中。

### `S4` 本地存储最小权限与 `Preflight` 安全基线检查
1. **文件与目录权限收敛（Unix）**：
   - 初始化与启动阶段确保：
     - 数据目录 `~/.agent-body/` 权限位为 `0700`；
     - 数据库文件 `body.db`（及 `-wal`/`-shm`）、配置文件 `config.toml` 权限位为 `0600`（仅属主可读写，禁止同组或其他用户读取）。
2. **`PreflightChecker` 安全探针**：
   - 新增 `PreflightChecker::check_security_posture(report, data_dir, config)`：
     - 检出 `config.toml` / `body.db` 存在 group/other 读权限（`mode & 0o077 != 0`）时报告 `WARN` 并给出 `chmod 600` 修复命令；
     - 检出 `web.host` 非回环且无 `auth_token` 时报告 `WARN`/`FAIL`；
     - 检出配置文件或环境变量模板中误填硬编码 Token 时提示安全风险。

### `S5` 外部语料与跨会话记忆抗投毒防线 (`PromptInjectionScanner`)
1. **间接提示词注入风险控制**：
   - 外部源码（`/root/new/src/`）、用户文档或第三方 `SKILL.md` 的头部注释、Frontmatter、`description` 等字段最终会进入 `assets.description` 并注入 `body_brief` 的上下文窗口。
   - 新增轻量确定性扫描器 `PromptInjectionScanner`（纯正则/规则微秒级实现，遵循规约 #2 不引入 LLM 开销）：
     - 检测典型越狱与指令劫持模式（如 `ignore previous instructions`、`disregard all prior rules`、`<system>` / `</system>` 伪造分隔符、`you are now` 角色劫持、隐藏零宽控制字符 `\u{200B}`-`\u{200F}`）；
     - 命中时在 L1 元数据打标 `security_flags: ["prompt_injection_suspected"]`，剥离危险控制字符并对可疑摘要进行截断隔离标记（`[EXTERNAL_UNTRUSTED_TEXT]`），防止直接作为高可信指令注入 `BriefAssembler`。

---

## 五、实证安全基准规格 (Benchmark v2.1 Security Extensions)

严格遵循 [README.md 规约 #18](../../README.md#L307-L311) 与 [10-benchmark-spec.md](10-benchmark-spec.md)，将安全防御纳入可复现量化评测：

| 基准 ID | 名称 | 测试逻辑（Without vs With / 现网采样） | 量化 SLA 门禁 |
|:---:|---|---|---|
| **`W11`** | `security_redaction_and_guardrail`<br/>（全链路脱敏与防注入拦截） | 构建隔离对抗样本集：含带密码 `cmdline`（`--password=secret123` / `postgres://u:pwd@127.0.0.1`）、带 `; rm -rf` 特殊文件名 Hook 事件、及含 `Ignore previous instructions` 的外部 `README.md`；对比无防护基线直接拼接泄露/执行 vs Agent Body 脱敏转义与抗投毒清洗。 | 凭据脱敏率 `== 100%`<br/>Hook 注入字符逃逸数 `== 0`<br/>误杀正常路径 `< 5%`<br/>单条处理耗时 `< 500 μs` |
| **`DQ4`** | `secret_hygiene_and_perm_gate`<br/>（现网库密文卫生与权限合规率） | 采样现网 `~/.agent-body/` 目录与 `body.db` 权限位（`0700`/`0600`），并抽检 `active_processes`、`memories`、`assets`、`external_services` 表是否残留未脱敏的敏感前缀或密码串。 | 明文密钥残留率 `== 0%`<br/>敏感文件权限合规率 `== 100%` |

---

## 六、关联规约与实施入口
- **顶层规约**：[README.md 规约 #8 & #19](../../README.md) · [product-boundaries-and-governance.md](../src_doc/product-boundaries-and-governance.md)
- **代码规约引擎**：[src/placement/conventions.rs](../../src/placement/conventions.rs)（`"security"` 分类 3 大强制红线）
- **专项演进路线图**：[docs/src_doc/roadmap-phase32.md](../src_doc/archive/roadmap-phase32.md)（Phase 32 ~ Phase 33 串行原子任务清单）
- **反馈总账跟踪**：[user_feedback.md](../../user_feedback.md)（`CON-07`, `REQ-06`, `REQ-07`）
