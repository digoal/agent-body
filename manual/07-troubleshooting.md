# 07 — 故障诊断与常见问题（Troubleshooting & FAQ）

> **章节目标**: 收集汇总 Agent Host Steward 在构建、运行、守护和 MCP 调用过程中可能遇到的常见问题、报错排查思路与修复方法。

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
     ps aux | grep agent-steward
     ```
  2. 大管家底层已配置 `PRAGMA busy_timeout = 5000;` 和 WAL 模式，正常情况下读写互不阻塞；
  3. 若持续锁定，可尝试运行修复检查：
     ```bash
     sqlite3 ~/.steward/steward.db "PRAGMA integrity_check;"
     ```

### Q4: 提示 `no such column: embedding`
- **原因**: 数据库是从 v0.1.0 升级上来的旧文件，缺少 v0.2.0 新增的向量字段。
- **解决方法**:
  在终端中执行字段补齐：
  ```bash
  sqlite3 ~/.steward/steward.db "ALTER TABLE memories ADD COLUMN embedding TEXT DEFAULT '[]';"
  ```

---

## 3. 端口探测与网络问题

### Q5: 为什么 `env port` 检测到端口 OCCUPIED，但 `conflict` 信息显示为 null？
- **原因**:
  - 该端口虽然处于监听状态，但持有该端口的进程属于其他隔离容器/Namespace，或者当前用户无权读取 `/proc/[pid]/fd` 符号链接（普通非 root 用户权限受限）；
  - 或者该端口并非由进程持有，而是处于 TCP `TIME_WAIT` 或内核套接字状态。
- **建议**:
  优先听从大管家给出的 `recommended_free_ports` 建议，改用推荐的空闲端口启动服务。

---

## 4. MCP 通信与 Agent 挂载问题

### Q6: Agent 挂载 MCP 服务后，一直卡在等待响应
- **排查步骤**:
  1. 确保在命令行中单独执行 `./target/release/agent-steward mcp` 时能正常进入 stdio 阻塞等待；
  2. 检查 Agent 客户端配置中的路径是否为**绝对路径**（如 `/root/new/work/agent-steward/target/release/agent-steward`）；
  3. 确认可执行文件具备执行权限：`chmod +x target/release/agent-steward`。

### Q7: 混合检索调用 Ollama 超时
- **现象**: `memory search` 能返回结果，但没有显示向量相似度。
- **说明**: 这是大管家的 **全自动优雅降级保护**！当本地 Ollama 服务未启动、模型未拉取或 1.5 秒内未响应时，大管家自动降级为 FTS5 关键词检索，确保 Agent 的业务请求 100% 成功返回而不抛出网络异常。
- **恢复语义增强**:
  ```bash
  # 启动本地 Ollama
  ollama serve
  # 拉取推荐的中文多语言嵌入模型
  ollama pull bge-m3:latest
  ```
