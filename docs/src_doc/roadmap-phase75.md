# Agent Body Phase 75 演进路线图 (`ISSUE #54` — 一等公民交叉验证与全端相互补齐)

> **定位**：基于 Phase 74 (`8cb6261`) 全仓一等公民交叉验证结果，补齐 **门禁交叉防漂移 × CI/Release/Docker 加固 × 旗舰 `body metrics` 四端 1:1:1 × 6 卷官方手册 × 9 份演说动态 SSoT 重生 × 公众长文与架构 SVG 升维**，彻底消除多轮迭代后一等公民局部掉队。

---

## 检查点追踪表 (Checkpoints)

| 阶段 | 任务清单 | 对应文件 | 状态 |
|---|---|---|:---:|
| **Phase 75-A** | `evolution_manifest.py` 升级交叉验证（Check D~G）；加固 `Makefile`、`ci.yml`、`release.yml`、`Dockerfile` (`--target probe`)、`docker-compose.yml`、`config.example.toml` | [scripts/evolution_manifest.py](../../scripts/evolution_manifest.py) · [ci.yml](../../.github/workflows/ci.yml) · [Dockerfile](../../Dockerfile) | ✅ 已完成 |
| **Phase 75-B** | 清零根目录/`INSTALL.md`/`docs/arch/`/`skills/`/`development.md`/`design_doc/{05,06,18,20}` 漂移与缺失互链 | [README.md](../../README.md) · [INSTALL.md](../../INSTALL.md) · [02-for-developers.md](../arch/02-for-developers.md) · [agent-body-developer SKILL](../../skills/agent-body-developer/SKILL.md) | ✅ 已完成 |
| **Phase 75-C** | 补齐旗舰 `agent-body body metrics` CLI 与 `body_state(action="metrics")` MCP（维持 43 工具），补全 6 卷官方手册 | [src/cli_args.rs](../../src/cli_args.rs) · [body_handlers.rs](../../src/api/mcp_tools/body_handlers.rs) · [docs/manual/](../manual/README.md) | ✅ 已完成 |
| **Phase 75-D** | 4 套演说生成器彻底去硬编码并纳入 Phase 74~75；重生成 9 份 HTML 并通过 Playwright 验真；升级架构 SVG 与公众长文 | [scripts/build_flagship_roadshow.py](../../scripts/build_flagship_roadshow.py) · [docs/markdown/svg/architecture.svg](../markdown/svg/architecture.svg) · [why-agent-needs-agent-body.md](../markdown/why-agent-needs-agent-body.md) | ✅ 已完成 |

---

## 关联文档与验证入口
- 交叉防漂移门禁：`make quality-gate`
- 全工作区串行测试：`RUST_TEST_THREADS=1 cargo test --workspace`
- 演说无头视觉验真：`make verify-decks`
