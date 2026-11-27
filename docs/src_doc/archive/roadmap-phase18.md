# Phase 18 详细实施路线图：跨平台原生探测深化与轻量多机联邦网格

> **编制**: Antigravity · **日期**: 2026-11-19  
> **对齐**: [README.md 待探讨 ISSUE #1 & #3](../../README.md#L276-L284) · [roadmap-phase15-18.md](roadmap-phase15-18.md)  
> **断点检查点**: [progress-phase18.md](progress-phase18.md)

---

## 一、核心设计（遵循规约 #2 递弱代偿 & #5 零捏造）

1. **18.1 跨平台原生探测 (`src/platform/`)**：
   - `PlatformProbe` Trait 统一端口监听枚举与自动化服务/计划任务扫描；
   - `linux.rs`：保留 `/proc` + systemd 零外依赖实现（并修复 `process_net::parse_proc_net_tcp` 的 `fields[3]` 状态列索引）；
   - `macos.rs`：`lsof -nP -iTCP -sTCP:LISTEN -F pcLn` 解析与 `launchd` plist (`Label` / `ProgramArguments`) 解析；
   - `windows.rs`：`netstat -ano -p TCP` + `tasklist /FO CSV /NH` + `schtasks /Query /FO CSV /NH` 解析；
   - 解析纯函数在所有平台均参与编译，以真实 fixture 在 Linux CI 验证跨平台解析正确性。
2. **18.2 轻量级多机联邦 Peer Mesh (`src/mesh/`)**：
   - 不手搓 Raft/Gossip 轮子，复用内嵌 HTTP 服务暴露 `/api/v1/mesh/{ping, facts, locks, locks/acquire}`；
   - 鉴权复用 `[web].auth_token`（写接口强校验 Token）；
   - `MeshClient` 支持 `ping_all`、`facts_all`、`locks_all`、`federated_port_check`、`acquire_on_peer`；
   - 新增 `body_mesh_query` MCP（总数 **38 个**）及 `mesh` / `platform` CLI 子命令。
