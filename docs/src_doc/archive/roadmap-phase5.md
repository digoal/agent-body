# Agent Body 研发演进路线图 — 第五阶段 (Phase 5)

> **目标**: 宿主机实装部署、常驻守护服务、真实全盘初筛纳管、MCP 实装与开源分享  
> **制定时间**: 2026-11-15  
> **执行模式**: 严格串行分步、测试驱动、断点随时续传、真实环境实操验证  
> **基线状态**: Phase 4 全量通过 (31 unit tests ok, cargo build --release 成功, v3.0 图智能闭环)

---

## 阶段任务状态看板 (Breakpoint Tracking Board)

| 任务编号 | 任务模块 | 核心工作内容 | 状态 | 验证门禁 | 完成时间 |
|---|---|---|---|---|---|
| **P5-1** | Git 提交与 Tag | 暂存所有 Phase 4 代码与文档，提交 commit 并打上 `v0.3.0` Tag | ✅ 已完成 | `git status` 工作区 clean，`git tag` 存在 | 2026-11-15 18:04 |
| **P5-2** | 系统级实装 | 安装二进制到 `/usr/local/bin/agent-body`，执行 `/root/new/` 首次分级全盘扫描 | ✅ 已完成 | `/usr/local/bin/agent-body status` 返回 1000+ 资产 | 2026-11-15 18:07 |
| **P5-3** | 真实拓扑与审计 | 运行 `links audit` 审计宿主机真实断链，并测试 CodeGraph 跨层联动 | ✅ 已完成 | 输出真实健康分与断链报告 | 2026-11-15 18:08 |
| **P5-4** | Systemd 守护托管 | 编写并注册 `/etc/systemd/system/agent-body.service`，启动 daemon 并验证 inotify 监听 | ✅ 已完成 | `systemctl is-active agent-body` 为 active | 2026-11-15 18:09 |
| **P5-5** | MCP 客户端实装 | 将 `agent-body` 注册至本机环境配置，通过 stdio 握手验证 18 个工具 | ✅ 已完成 | `tools/list` 成功返回 18 个工具 | 2026-11-15 18:10 |
| **P5-6** | 开源仓库镜像 | 整理工程并镜像同步至 `/root/new/shared_git/agent-body` | ✅ 已完成 | `/root/new/shared_git/agent-body` 就绪且为独立 git | 2026-11-15 18:11 |
| **P5-7** | 深度技术长文沉淀 | 撰写高质量公众号深度技术长文并配专业 SVG 架构图 | ✅ 已完成 | `docs/markdown/` 下产出图文并茂长文 | 2026-11-15 18:16 |

---

## 断点续传执行协议

1. **原子化串行推进**: 依次执行 P5-1 至 P5-7，严禁跳步或混行。
2. **状态实时落盘**: 每完成一个子任务，必须在此看板中将状态更新为 `✅ 已完成`，并记录完成时间。
3. **真实环境优先**: 每一个部署与系统动作均需在真实 Linux 环境中执行并拿到实际输出，严禁纸上谈兵。
