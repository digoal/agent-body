# Agent Host Steward 研发演进路线图 — 第五阶段 (Phase 5)

> **目标**: 宿主机实装部署、常驻守护服务、真实全盘初筛纳管、MCP 实装与开源分享  
> **制定时间**: 2026-11-15  
> **执行模式**: 严格串行分步、测试驱动、断点随时续传、真实环境实操验证  
> **基线状态**: Phase 4 全量通过 (31 unit tests ok, cargo build --release 成功, v3.0 图智能闭环)

---

## 阶段任务状态看板 (Breakpoint Tracking Board)

| 任务编号 | 任务模块 | 核心工作内容 | 状态 | 验证门禁 | 完成时间 |
|---|---|---|---|---|---|
| **P5-1** | Git 提交与 Tag | 暂存所有 Phase 4 代码与文档，提交 commit 并打上 `v0.3.0` Tag | ⏳ 进行中 | `git status` 工作区 clean，`git tag` 存在 | - |
| **P5-2** | 系统级实装 | 安装二进制到 `/usr/local/bin/agent-steward`，执行 `/root/new/` 首次分级全盘扫描 | ⬜ 未开始 | `/usr/local/bin/agent-steward status` 返回正常 | - |
| **P5-3** | 真实拓扑与审计 | 运行 `links audit` 审计宿主机真实断链，并测试 CodeGraph 跨层联动 | ⬜ 未开始 | 输出真实健康分与断链报告 | - |
| **P5-4** | Systemd 守护托管 | 编写并注册 `/etc/systemd/system/agent-steward.service`，启动 daemon 并验证 inotify 监听 | ⬜ 未开始 | `systemctl is-active agent-steward` 为 active | - |
| **P5-5** | MCP 客户端实装 | 将 `agent-steward` 注册至本机环境配置，通过 stdio 握手验证 16 个工具 | ⬜ 未开始 | `tools/list` 成功返回 16 个工具 | - |
| **P5-6** | 开源仓库镜像 | 整理工程并镜像同步至 `/root/new/shared_git/agent-steward` | ⬜ 未开始 | `/root/new/shared_git/agent-steward` 就绪且为独立 git | - |
| **P5-7** | 深度技术长文沉淀 | 撰写高质量公众号深度技术长文并配专业 SVG 架构图 | ⬜ 未开始 | `markdown/` 下产出图文并茂长文 | - |

---

## 断点续传执行协议

1. **原子化串行推进**: 依次执行 P5-1 至 P5-7，严禁跳步或混行。
2. **状态实时落盘**: 每完成一个子任务，必须在此看板中将状态更新为 `✅ 已完成`，并记录完成时间。
3. **真实环境优先**: 每一个部署与系统动作均需在真实 Linux 环境中执行并拿到实际输出，严禁纸上谈兵。
