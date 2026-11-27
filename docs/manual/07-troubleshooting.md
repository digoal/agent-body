# 07 — 故障诊断与常见问题（Troubleshooting & FAQ）

> **章节目标**: 收集汇总 Agent Body 在构建、运行、守护和 MCP 调用过程中可能遇到的常见问题、报错排查思路与修复方法。

---

## 1. 编译与构建问题

### Q1: 执行 `cargo build` 时报错 `cargo: command not found`
- **原因**: 终端未加载 Rust Cargo 环境变量。
- **解决方法**:
  ```bash
  export PATH="/root/.cargo/bin:$PATH"
  cargo --version
  ```

### Q2: 编译时出现内存不足（OOM Killer）或卡死
- **原因**: 部分依赖库（如 `reqwest`, `clap`）在并行编译多个 codegen 单元时瞬时占用内存较大。
- **解决方法**:
  限制 Cargo 并行编译任务数：
  ```bash
  cargo build --release -j 2
  ```

---

## 2. 数据库与存储问题

### Q3: 报错 `database is locked (busy_timeout)`
- **原因**: 存在长时间持有写入锁的事务或进程非正常退出导致 WAL 未释放。
- **排查与解决**:
  1. 检查是否有卡死的守护进程：
     ```bash
     ps aux | grep agent-body
     ```
  2. Agent 本体底层已配置 `PRAGMA busy_timeout = 5000;` 和 WAL 模式，正常情况下读写互不阻塞；
  3. 若持续锁定，可尝试运行修复检查：
     ```bash
     sqlite3 ~/.agent-body/body.db "PRAGMA integrity_check;"
     ```

### Q4: 提示 `no such column: embedding`
- **原因**: 数据库是从 v0.1.0 升级上来的旧文件，缺少 v0.2.0 新增的向量字段。
- **解决方法**:
  在终端中执行字段补齐：
  ```bash
  sqlite3 ~/.agent-body/body.db "ALTER TABLE memories ADD COLUMN embedding TEXT DEFAULT '[]';"
  ```

---

## 3. 端口探测与网络问题

### Q5: 为什么 `env port` 检测到端口 OCCUPIED，但 `conflict` 信息显示为 null？
- **原因**:
  - 该端口虽然处于监听状态，但持有该端口的进程属于其他隔离容器/Namespace，或者当前用户无权读取 `/proc/[pid]/fd` 符号链接（普通非 root 用户权限受限）；
  - 或者该端口并非由进程持有，而是处于 TCP `TIME_WAIT` 或内核套接字状态。
- **建议**:
  优先听从Agent 本体给出的 `recommended_free_ports` 建议，改用推荐的空闲端口启动服务。

---

## 4. MCP 通信与 Agent 挂载问题

### Q6: Agent 挂载 MCP 服务后，一直卡在等待响应
- **排查步骤**:
  1. 确保在命令行中单独执行 `./target/release/agent-body mcp` 时能正常进入 stdio 阻塞等待；
  2. 检查 Agent 客户端配置中的路径是否为**绝对路径**（如 `/root/new/work/agent-body/target/release/agent-body`）；
  3. 确认可执行文件具备执行权限：`chmod +x target/release/agent-body`。

### Q7: 混合检索调用 Ollama 超时
- **现象**: `memory search` 能返回结果，但没有显示向量相似度。
- **说明**: 这是Agent 本体的 **全自动优雅降级保护**！当本地 Ollama 服务未启动、模型未拉取或 1.5 秒内未响应时，Agent 本体自动降级为 FTS5 关键词检索，确保 Agent 的业务请求 100% 成功返回而不抛出网络异常。
- **恢复语义增强**:
  ```bash
  # 启动本地 Ollama
  ollama serve
  # 拉取推荐的中文多语言嵌入模型
  ollama pull bge-m3:latest
  ```

---

## 5. 环境预检、多模态元数据标注、运行模式与沙盒清理 (v1.1.0)

### Q8: 部署到新机器或 CI/CD 后如何快速排查缺少哪项系统依赖？
- **排查与解决**:
  使用内置的运行环境预检命令（退出码 `0` 表示满足全部 `REQUIRED` 项，非 `0` 表示缺失关键依赖）：
  ```bash
  agent-body preflight
  agent-body preflight --json
  ```

### Q9: 本地未拉取 `qwen3.5:0.8b` 或 Ollama 离线时，扫描资产会报错吗？
- **说明**: 不会报错！资产扫描默认执行 **Track 1 (L1 Rust 确定性语法提取)**，耗时 `<1ms` 即可完成结构化元数据提取；当本地 Ollama 不可达或超时时，L2 异步标注会自动跳过并优雅降级，待模型就绪后可再运行：
  ```bash
  ollama pull qwen3.5:0.8b
  agent-body asset scan /root/new/work --enrich-with-ollama
  ```

### Q10: 跑完路演演示或 Benchmark 测试后，如何安全清理沙盒记录？
- **说明**: 路演与基准评测默认使用隔离前缀（`demo:*` / `bench:*`），不会污染正式业务资产。如需一键清除测试产生的沙盒锁或基准运行记录，可执行：
  ```bash
  agent-body demo reset
  agent-body bench reset
  ```

### Q11: 本地开启了 `shadow_ab` 后想临时退回原生搜索怎么办？遇到驼峰标识符漏召回怎么办？
- **说明**:
  1. 可在不修改配置文件的情况下随时通过环境变量 `AGENT_BODY_MODE=bypass` 或命令 `agent-body body runtime-mode --mode bypass` 一键旁路（`shadow_ab` 恢复对照，默认 `body_only`）；
  2. 遇到代码驼峰标识符或旧格式资产 `schema_version` 漏召回时，可执行：
     ```bash
     # 查看 Shadow A/B 证伪报告（含 H1~H5 自动自愈次数）
     agent-body body ab-report --window 7d
     # 对存量资产执行纯 Rust 多模态 L1A v2.0 与 Know-How 增量回填
     agent-body asset scan <your_path> --backfill-multimodal --extract-knowhow
     ```

---

## 6. 全栈可观测排障、W3C Trace 链路追踪与 `OBS-01~05` 自诊断 (Phase 74)

### Q12: 请求返回 401/403 或 Web/探针响应变慢，如何端到端串联日志与审计记录？
- **排查步骤**:
  1. 在调用客户端查看 HTTP 响应头 **`X-Trace-Id`** 与 **`traceparent`**（旗舰 Web 端允许调用方入站透传标准 W3C `traceparent`）；
  2. 将守护进程开启 `AGENT_BODY_LOG_FORMAT=json` 后，直接用同一 `trace_id` 检索单行 JSONL 日志、`audit_events` 表或 `agent-body-probe` 的 SIEM RFC 5424 `trace_id="..."` 字段；
  3. 对实时瓶颈执行 `/metrics` 抓取或直接运行自诊断：
     ```bash
     curl -sS -i http://127.0.0.1:8765/metrics | head -n 30
     agent-body body metrics --format json
     agent-body-probe diagnose --limit 20
     ```

### Q13: `agent-body-probe diagnose` 报出 `OBS-01` ~ `OBS-05` 异常怎么处置？
| 规则码 | 级别 | 触发语义 | 推荐处置动作 |
|---|---|---|---|
| `OBS-01` | `warn` | 鉴权或 RBAC/RLS 访问拒绝数达标 | 核对 Bearer Token、mTLS 证书 CN 与租户 `host_labels` 映射 |
| `OBS-02` | `critical` | 外部 FDW 或轨 B 沙盒进程超时 | 检查目标数据源连通性与 `.control.toml` 中 `[sandbox].timeout_ms` |
| `OBS-03` | `critical` | 算子/探针整体错误率超过 10% | 查看 `recent_slow_or_failed` 中的 `trace_id` 与 `detail` 定位源因 |
| `OBS-04` | `warn` | 内核等待事件 P95 延迟 `>= 250ms` | 检查锁争用、外部探针阻塞或后台协程频率 |
| `OBS-05` | `warn` | SIEM 环形缓冲丢弃审计事件 | 调高 `SiemSinkConfig.ring_capacity` 或缩短 flush 间隔 |
