# Agent Body — Phase 48 演进路线图：全面更名 `agent-body`（方案 B 彻底硬切 · ISSUE #31）

> **编制**: Antigravity · **日期**: 2026-11-22 (Asia/Shanghai)  
> **决策口径**: **方案 B（彻底硬切、一劳永逸、零历史别名包袱）**；仓库工作目录 `/root/new/work/agent-steward` 暂保持不动，待用户后续手动改名为 `agent-body`。  
> **前置基线**: `v0.4.0`（Commit `ec261c2` · 27 模块 · 43 MCP 工具 · 82 SPI 方法 · 27 表 · 268 测试全绿 · Bench `v3.7` 全绿）

---

## 一、核心标识符映射契约（100% 确定性，防重名与遗漏）

| 维度 | 旧标识 | 新标识（方案 B 彻底硬切） | 关键防坑说明 |
|---|---|---|---|
| **品牌名称** | `Agent Host Steward` | `Agent Body` | 副标题：`脑体分离架构 · 宿主机具身底座` |
| **Cargo 包 / 二进制 / Crate** | `agent-steward` / `agent_steward` | `agent-body` / `agent_body` | `/usr/local/bin/agent-body` |
| **43 个 MCP 工具前缀** | `steward_*`（含 `steward_body_state`） | `body_*`（#43 为 **`body_state`**） | 严防机械替换产生 `body_body_state` |
| **默认数据目录与主库** | `~/.steward/steward.db` | `~/.agent-body/body.db` | 27 张表结构完全不变，300 MB 现网库零损直迁 |
| **Systemd 守护服务** | `agent-steward.service` | `agent-body.service` | 停旧服务使 SQLite WAL 落盘后再拷贝迁移 |
| **客户端 Hook 桥接** | `steward_bridge.py` / `"agent-steward"` | `body_bridge.py` / `"agent-body"` | 重新生成 `~/.gemini/config/hooks.json` |
| **Mesh 零信任 HTTP 头** | `X-Steward-*` | `X-Body-*` | `X-Body-Peer / Timestamp / Nonce / Signature` |
| **技能包目录** | `skills/agent-steward-*` | `skills/agent-body-*` | `git mv` 并同步至 `~/.gemini/antigravity-cli/skills/` 与 `/root/new/skills/` |
| **离线演讲文件名** | `presentation/steward-deck.html` | `presentation/agent-body-deck.html` | 同步更新 `src/web/assets/mod.rs` 与 `presentation/index.html` |
| **Makefile 镜像同步** | 硬编码 `/root/new/work/agent-steward/` | `$(CURDIR)/` → `/root/new/shared_git/agent-body/` | 手动改目录名前后均可直接运行 |
| **仓库目录名** | `/root/new/work/agent-steward` | **本次不动** | 留待用户后续手动 `mv` 改名 |
| **Tier-2 历史归档** | `docs/src_doc/roadmap-phase2..47.md` 等 | **保留历史原貌** | 不篡改历史演进史料，仅刷新全部代码与活跃文档 |

---

## 二、7 步原子实施清单与防翻车验收门禁

| 步骤 | 原子任务范围 | 精确触点 | 防错要点 & 验收门禁 (DoD) |
|:---:|---|---|---|
| **48.1** | **Cargo、Crate 与配置/数据路径层硬切** | `Cargo.toml`, `src/config/mod.rs`, `config.example.toml`, `src/main.rs`, `tests/*.rs`, `src/watcher/mod.rs`, `src/preflight/mod.rs` | `name = "agent-body"`；`use agent_steward::` → `use agent_body::`；默认路径 `~/.agent-body/body.db`（Watcher 黑名单同步更新 `/.agent-body/`）；`cargo check --tests` 先过一轮 |
| **48.2** | **43 个 MCP 工具、Hooks、Mesh、Benchmark 协议层硬切** | `src/api/mcp_tools.rs`, `src/api/mcp.rs`, `src/hooks/client_gen.rs`, `src/mesh/{auth,mod}.rs`, `src/benchmark/{ledger,workloads,report,mod}.rs`, `src/brief/assembler.rs`, `src/demo/mod.rs`, `src/feedback/mod.rs` | **先特判 `steward_body_state` → `body_state`**，再将剩余 42 工具切为 `body_*`；Hook 桥切换 `body_bridge.py`；Mesh 头切为 `X-Body-*`；`cargo test --lib` 全绿 |
| **48.3** | **Web UI、黑白屏 43/43 矩阵与构建脚本硬切** | `src/web/{server,parity_api,assets/mod,mod}.rs`, `Makefile`, `Dockerfile`, `docker-compose.yml`, `scripts/evolution_manifest.py` | 43/43 Parity 矩阵全切 `body_*` + `agent-body ...`；`DECK_HTML` 指向 `presentation/agent-body-deck.html`；`scripts/evolution_manifest.py` 正则切换 `body_[a-z0-9_]+`；全量 `cargo test` 268/268 通过 |
| **48.4** | **SKILL 重命名与 9 份 HTML 演讲体系全面刷新** | `skills/agent-steward-*` (`git mv` → `skills/agent-body-*`), `presentation/steward-deck.html` (`git mv` → `agent-body-deck.html`), `scripts/{deck_engine,build_decks_part1,build_decks_part2}.py`, `presentation/*.html` | 重跑 `build_decks_part{1,2}.py` 并刷新 `presentation.html` / `agent-body-deck.html` / `index.html`；`python3 scripts/verify_presentations.py` **9/9 全页 0 溢出 0 重叠** |
| **48.5** | **全部活跃文档刷新与残留扫描门禁** | `README.md`, `EVOLUTION_GUIDE.md`, `INSTALL.md`, `ISSUE.md`, `docs/README.md`, `docs/arch/*`, `docs/manual/*`, `docs/idea_doc/04*`, `docs/design_doc/{06,10,17}*`, `docs/src_doc/{development,progress}.md` | `make evolution-check` (`0 drift`)；全仓代码/活跃文档/HTML 扫描残留 `agent-steward` / `steward_` / `X-Steward-` 为 0（除历史归档与变更说明） |
| **48.6** | **本机生产环境原子平滑切换（零数据丢失）** | `/usr/local/bin/agent-body`, `/etc/systemd/system/agent-body.service`, `/root/.agent-body/body.db`, `~/.gemini/config/{mcp_config.json,hooks.json,hooks/body_bridge.py}`, 全局 `skills/` | **严格顺序**：① `make build` 先编出新 Release 二进制 → ② `systemctl stop & disable agent-steward.service`（确保 WAL 完整落盘）→ ③ 复制 `/root/.steward` 到 `/root/.agent-body`（`steward.db`→`body.db`，旧目录保留为 `/root/.steward.bak-20261122` 备份）→ ④ 安装启动 `agent-body.service` 并验证 `agent-body body manifest` + `DQ1~DQ8` 全绿 → ⑤ 更新本机 `mcp_config.json`、`hooks.json`、`body_bridge.py` 与全局 `skills/agent-body-*`（清理旧二进制与旧 unit 文件） |
| **48.7** | **终验、开源镜像同步与 CodeGraph 刷新** | `make lint-panic`, `make evolution-check`, `make test`, `agent-body bench run --suite full --save true`, `make sync-mirror`, `codegraph sync` | 全量单测/集成测试 0 失败、Bench `v3.7` 全绿存证入 `body.db`、Playwright 9/9 PASS、`codegraph sync` 刷新完成 |

---

## 三、切完后的手工收尾提示（留给用户）

1. **手动重命名仓库目录**：确认并 `git push` 后，直接执行 `mv /root/new/work/agent-steward /root/new/work/agent-body`（`Makefile` 已使用 `$(CURDIR)`，改名后零配置继续用）。
2. **重启 AI 客户端会话**：由于本机 `mcp_config.json` 与 `hooks.json` 已切换至 `agent-body`，新开对话窗口即可加载新的 `agent-body` MCP 43 工具与 `body_bridge.py` 钩子。
3. **更新全局规则文案（如 `~/.gemini/GEMINI.md`）**：如有自定义全局提示词引用 `agent-steward` / `steward_*`，可同步替换为 `agent-body` / `body_*`。
