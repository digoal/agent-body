## 未来演进与待探讨 ISSUE (Roadmap & Open Issues)

1. **分布式/多机 Agent Body 跨主机网状协调 (Gossip / Raft / HTTP Mesh)** `✅ Phase 18 已落地轻量 HTTP Peer Mesh`：
   - 当宿主环境扩展为多台机器构成的集群时，将协作锁与事实图谱提升为跨节点分布式网状协调。

2. **智能死锁检测与等待唤醒机制 (Wait-Queue / Push Notification)** `✅ Phase 17 已落地 WFG + Wait-Queue`：
   - 为排他锁提供可订阅的等待队列，当持锁 Agent 释放资源时，主动通知排队中的 Agent，避免轮询开销。

3. **跨平台原生适配深化 (macOS / Windows)** `✅ Phase 18 已落地 PlatformProbe 抽象`：
   - 当前以 Linux（/proc、inotify、Systemd）为首选核心运行环境，未来进一步拓展对非 Linux 平台的底层环境感知。
  
4. **管理范围的扩大** `✅ Phase 16 已落地 Ecosystem & Context 防爆路由`：
   - 还得帮 Agent 管 MCP, SKILL, 可以使用的外部服务等。因为 MCP、SKILL 多了之后啊，Context 容易被打爆，导致大模型注意力下降，影响模型生成质量。 管外部服务的目的则是 Agent 宿主机本机环境的延展，就像销售人员有一本外部联系人名单一样，随时可以按需来 call。
  
5. **Web UI 显示内容优化与交互重构** `✅ Phase 19 已落地 Web UI 2.0 五大工作台与真实拓扑`：
   - Web UI 显示的内容有点简陋，也不美观。需要重新设计一下。

6. **黑屏功能和白屏功能双向同步** `✅ Phase 21 已落地 39 工具 100% Parity 同步（Phase 28 已扩展至 40/40，Phase 30 已扩展至 41/41）`：
   - 黑屏功能和白屏功能要同步，白屏主要体现在WEB端，黑屏体现在 MCP,SKILL,CLI 侧（已升格为开发规约 #16）。

7. **WEB 侧路演与对外分享演示剧场** `✅ Phase 22 已落地「🎬 路演演示剧场」与第 39 工具`：
   - 要在WEB侧额外设计更多演示功能，路演和对外分享时使用，从产品经理和市场营销推广的角度来挑选哪些场景需要被拿来演示（已升格为开发规约 #17）。

8. **元数据定义哪些 Key 与本地 Ollama 小模型自动标注** `✅ Phase 23 已落地三层正交元数据体系与 Ollama 异步双轨标注`：
   - 定义 **L1 通用语义核心 (`purpose`/`domain`/`capabilities`/`entry_point`/`usage_hint`) + L2 类型特化 Facet + L0 管线溯源 (`content_hash`/`annotation_status`/`annotation_model`)** 三层元数据 Key 体系；
   - 采用 **L1 Rust 微秒级确定性语法提取 + L2 本地 Ollama 小模型（`qwen3.5:0.8b`）脱敏异步精炼** 双轨流水线，配合 **Micro (~25 tok) / Standard (~85 tok) / Full (~180 tok)** 三级渐进式 Token 披露，彻底告别 `{"watcher_sync":true}` 空洞元数据且零云端 Token 消耗。

9. **「比较优势」立意源起与 PostgreSQL 调研痛点补完** `✅ Phase 24 已落地 Idea 起源篇与双 HTML 演说融合`：
   把为什么要做 agent-body 项目的以下这些意境和萌芽态的想法，整理后补充到 Idea 文档和路演、演讲 HTML 里。
```
我看到硅谷 101采访一位硅谷创业者，这位创业者居然在办公区（负一楼）将近一周都沉浸在 coding 中，原因是这个园区的物业把创业者的琐事都办了，创业者足不出户，沉浸在主业 coding 中。
企业高管往往都会配置专职或共享幕僚团队，把琐事办掉，高管专一的干本职工作，发挥最大能效。
家庭也一样，很多家庭家庭主妇把老公的一切都包了，老公专心工作赚钱。
这些都是很好的互相搭配，每个人都把主要精力放在最本职、最专长的事情上，有个经济学术语叫比较优势。
Agent 呢，我用久了 Agent 后发现，Agent 缺个Agent 本体或者说Agent 本体。为什么呢？比如我经常用 Agent 调研 PostgreSQL 的代码、功能、或应用场景等，每次给 Agent 提交任务，它都要重新拉取代码、文档，然后编译，安装，验证等。其实本机已经有了环境，或以前已经下载好了代码或文档，素材都是现成的，但 Agent 不知道，如果能给 Agent 配个Agent 本体，先问Agent 本体，这事不就解决了吗？也不会浪费时间、浪费空间折腾。Agent 的效率高了，也省 token 了。
```

10. **路演与演讲 HTML「地层剖面 / 工程蓝图」重构与全功能自检** `✅ Phase 24 已落地 16 页路演版与 22 页演讲版双 HTML + Playwright 自检全绿`：
    路演文档不够美观，也没有配演讲稿，可以改进一下：
````
生成单个 html 格式的演说文件(文件名不要出现中文字符)，请都放到一个文件中包括图片你可以用 base64 嵌入。产生的 html 最重要的是内容的正确性、并且是图文并茂的。一图胜千言，图的目的是帮助受众理解内容(原理图、架构图、使用场景图、最佳实践图等等)。图你可以现场绘制或使用原文中的，图要求美观且易于理解，图中表达的任何模块或文字必须在视觉上无任何遮挡、出界、交叉等问题。整个 slide 还可增加一定的动画演示（得有输入、播放等配套按钮），但只在对受众理解内容有帮助时增加。html 中还可以添加一下快捷键，必含按键“目录 G、本页讲稿 N、全文讲稿 T、自动播放、全屏 F”，其它按键按需提供，全屏后按键也要在页面底部显示。讲稿内容要注重逻辑推演和实证，口语化，逻辑要连绵不绝。讲稿不要弹窗打开，而是采用页内开窗，按一下显示，再按一下关闭，循环，在页面右侧显示，显示讲稿时原页面内容矢量缩放到左侧展示，按 T 显示全部讲稿时自动定位到当前 page，右侧显示讲稿时翻页时也进行联动自动滚动右侧演讲稿到当前页面。
html 内容16比9的比例，采用适合阅读的视觉风格。
```
【视觉风格：地层剖面 / 工程蓝图】
浅色主题，深字浅底。
色系：底 #F7F6F2 暖纸白、次级底 #F1EFE9；墨 #1B2B33、次级字 #46585F、
弱化字 #89938F、描边 #D9D5CB。主色 深靛青 #245A6B（浅 #E5EFF0），
强调 陶橙 #BE653B（浅 #F7E9DE），收益 青绿 #2F7A66，提示 金 #AE8A3E。
地层七阶（深→浅）#1E4B5A #2E6373 #47808D #74A5AD #A4C4C6 #CFE0DC #EBF1EE，
越深＝越底层。语义：靛青＝结构/机制，陶橙＝矛盾/代价/承接，青绿＝正确动作，金＝过渡。
字体：标题宋体 Songti SC / Noto Serif CJK SC，正文 PingFang SC / Noto Sans CJK SC，
数字页码 SF Mono；中文标点用「」『』——。
版式：16:9 固定舞台 1600×900，等比缩放 + letterbox，禁止响应式重排；左边距 64px，
眉标 13px/字距3.4px，标题 44px 宋体，导语 19px，1px 分隔线，图区 1400×430，
底部「承接 · NEXT」20px 宋体 + 3px 陶橙左边框，页脚等宽页码。
图形语法（硬规则）：所有示意图现场绘制为内联 SVG，不用外链位图；规整网格／分层横带；
连线只允许水平或竖直直角，禁止斜线穿过文字；文字与连线分离、框内留白 ≥14px、
按框宽自动缩字号，保证零出界／零遮挡／零交叉；一页一图一命题。
禁止：紫粉渐变、emoji、圆角卡片堆叠、滥用投影、装饰性图形、编造统计数字。
交付：单个 HTML 零外部依赖；
```  
注意，交付之前请自检 html 的各项功能是否都正常：内置动画是否正常、按键是否正常、显示讲稿后画面是否完整、缩放后的画面显示是否正常且完整、全文讲稿页面定位是否正确等。
````
  
11. **WEB UI 美化与白天/夜间双色调实时切换** `✅ Phase 25 已落地「☀️ 白天蓝图 / 🌙 夜间深空」双主题系统与面板美化`：
    - 彻底清除硬编码暗色值，建立完整的 `[data-theme="light"]`（暖纸工程蓝图 `#F7F6F2` + 深靛青 `#245A6B` + 陶橙 `#BE653B`，与路演/演讲 HTML 品牌同源）与 `[data-theme="dark"]`（深空指挥舱）两套语义 CSS Token 体系；
    - 顶栏提供一键切换按钮（支持 `T` 快捷键与 `localStorage` 记忆），切换瞬间同步重绘 Canvas 拓扑图配色，并将端口防撞、Pre-Hook、放置推导、DuckDB 热力图与趋势、宿主 GC 等面板升级为「可视化卡片 + 可折叠原始 JSON」。
  
12. **WEB UI「路演演示剧场」三层故事化直观重构** `✅ Phase 25 已落地三层渐进式故事舞台与 6 大专属可视化组件`：
    - 告别主舞台大段裸露 JSON 调试框：顶部增设经济学「比较优势」立意导览与 6 幕场景摘要卡片；
    - 每幕由真引擎返回 **💡 一句话生活比喻 (`analogy`) + 🎯 核心用意 (`intent_summary`) + 3 张高冲击 ROI 指标卡 (`hero_metrics`)**，左屏呈现盲人 Agent 三步翻车故事线，右屏渲染 **6 套专属可视化组件**（黑白名单阻断漏斗+武器库墙、Pre-Hook 1ms 红绿灯、3 跳蝴蝶效应因果链、双 Agent 死锁环路图、Token 99.9% 对比尺、1 秒简报仪表+安全自洁盾），原始 JSON 收纳至底部默认折叠的技术专家核验抽屉。  

13. **实证 Benchmark 对照评测体系与生产效能存证账本（确保使用效果有据可查）** `✅ Phase 27~28 已落地双轨实证引擎与 40/40 同步（Phase 30 扩展至 41/41，Phase 31~42 持续演进至 Benchmark v3.5）`：
    - 构建「实验室 A/B 可复现对照基准（Without Agent Body 裸奔物理探针基线 vs With Agent Body 真引擎实测 + 5 大内核微基准 SLA）」与「生产运行态每次调用效能存证账本（Token 节省、轮次缩减、端口/锁/断链/乱放冲突拦截、黑名单噪音阻断实时带出处入账）」双轨闭环；
    - 通过原生 MCP 工具 `body_benchmark`、CLI `agent-body bench` 与 Web UI「📊 实证基准与账本」工作台实现黑白屏 100% 双向同步与 SHA-256 存证签名，让每一项收益均可量化、可复现、可审计（已升格为开发规约 #18）。

14. **实际使用反馈闭环与痛点持续治理 (`user_feedback.md` & Feedback-to-Memory Loop)** `✅ Phase 30 已落地`:
    - 建立 [user_feedback.md](user_feedback.md) 作为主理人与 Agent 实际使用体验的一手反馈总账（覆盖 `PRO-XX` 优点、`CON-XX` 痛点、`BENCH-XX` 评测失真与 `REQ-XX` 新需求），打通「使用反馈 → 根因修复 → 记忆沉淀 → 回归门禁」双闭环；
    - Phase 30 已攻克现网实测暴露的 `body_brief` 474ms 冷启动瓶颈、`LinksEngine` 76.2% 假阳性断链、`effectiveness_ledger` 全渠道记账漏失、`memories` 长期记忆饥饿与 91GB `target/` 满盘预警问题。

15. **Benchmark 标准持续迭代机制 (`Benchmark Spec v1.0 → v2.0+ Six-Dimension Matrix`)** `✅ Phase 31 已落地`:
    - 引入 `spec_version` 基准协议版本化，从算术总耗时比演进为**几何平均加速比（Geometric Mean Speedup）+ 分组加速比**，新增 **D6 认知质量与准确率维度 (`Recall@K / Precision / 覆盖率`)**；
    - 完成 `W2/W3/W4/W7` 物理等价基线校准，并上线 `W9`（向量+属性混合检索召回）、`W10`（模板抗假阳性回归）与 `DQ1~DQ3`（现网宿主机数据健康度）基准负载（详见 [roadmap-phase30.md](docs/src_doc/archive/roadmap-phase30.md)）。

16. **Agent 本体安全纵深防御、全链路脱敏与抗投毒体系演进 (`docs/design_doc/11-security-spec.md` & Phase 32~33)** `✅ Phase 32~33 已落地`:
    - 补齐安全架构规格（[11-security-spec.md](docs/design_doc/11-security-spec.md)）与顶层安全规约 #19，围绕代码审计暴露的 `SEC-GAP-01~05` 完成全链路 `SecretRedactor`、Hook/DuckDB 防注入、Web 全量写鉴权、`PromptInjectionScanner` 抗投毒、`0600/0700` 权限收敛与 `W11`/`DQ4` 安全回归基准落地（详见 [roadmap-phase32.md](docs/src_doc/archive/roadmap-phase32.md)）。
  
17. **零重扫项目演进指南与架构防漂移门禁 (`EVOLUTION_GUIDE.md` + 自建 CodeGraph + 清单自检)** `✅ Phase 34 已落地`:
    - 建立方便后续项目演进的零重扫导航体系，避免新会话每次重复扫描 25 个模块（3.8 万行源码）和 48 份历史 `docs/src_doc/` 文档；
    - Phase 34 已落地四位一体防重扫体系：根目录 O(1) 演进活地图 [EVOLUTION_GUIDE.md](EVOLUTION_GUIDE.md)（8 大演进场景变更触点矩阵 + `Tier-0/1/2` 文档分级禁扫令）、本仓 `.codegraph/` 符号地图、确定性架构检查脚本 `scripts/evolution_manifest.py`（`make evolution-check`）及 `SKILL` 零重扫开工协议（详见 [roadmap-phase34-35.md](docs/src_doc/archive/roadmap-phase34-35.md)）。
  
18. **热门开源竞品横向对标与 Benchmark `v3.0` 轨道 C (`Horizontal Competitor Benchmark`)** `✅ Phase 35 已落地`:
    - Benchmark 不仅和裸奔自己比，也与外部主流产品同场实测比对；
    - Phase 35 已构建四大赛道竞品对标体系：**C1 长期记忆**（Mem0 / Letta / PowerContext / Shodh-Memory）、**C2 上下文数据库与路由**（OpenViking / 全量静态注入）、**C3 Agent 文件与协同治理**（AgentFS / OpenEnv）、**C4 宿主探针与代码依赖**（`rg`+`psutil`+`ss`+`git` / CodeGraph），在 Benchmark `v3.0` 上线 `COMP-C1~C4` 同机同数据集真跑负载与 12 维源码能力矩阵（严格区分 `LIVE_BENCHMARKED` 与 `ARCH_VERIFIED`，详见 [12-competitor-benchmark-spec.md](docs/design_doc/12-competitor-benchmark-spec.md)）。

19. **Benchmark `v3.1`「既省 Token × 上下文质量不降反升」帕累托双门禁演进 (`Token-Quality Pareto Dual-Gate`)** `✅ Phase 36 已落地`:
    - 关于 Benchmark 的核心修正准则：不能只体现省 Token，**既要省 Token，同时上下文质量不能下降甚至应该有显著提升**；
    - Phase 36 已消除 `W1~W8` 质量评分缺口（`BENCH-06`），落地确定性四维上下文质量模型 $Q = 40\% R_{\text{fact}} + 30\% P_{\text{signal}} + 20\% A_{\text{action}} + 10\% S_{\text{safety}}$ 与**有效信息密度倍数（`Facts / 1k Tokens`）**，为 `W1~W11` 与 `COMP-C1~C4` 全量实装真值事实断言、`Baseline Q vs Agent Body Q (+ΔQ)` 成对比较及防截断帕累托 SLA 硬门禁（详见 [roadmap-phase36.md](docs/src_doc/archive/roadmap-phase36.md)）。

20. **生产运行态深度对齐、拓扑零误报硬化与 Benchmark `v3.2` 等价校准 (`Production Polish & Topology Hygiene`)** `✅ Phase 37 已落地`:
    - **CLI `brief` 与 `BriefAssembler` 统一 & 进程命令去噪**：消除 `SessionStart` 钩子仅输出旧版浅表进程列表的分裂问题（规约 #16），并规范化/折叠超长多行内联脚本进程（`node -e` / `python3 -c`），同时重置 Unix `SIGPIPE` 彻底消灭管道截断 Panic（规约 #12）；
    - **`LinksEngine` 测试/注释/文档示例免疫**：排除 `.rs` 测试夹具/`#[cfg(test)]`/`placeholder` 与 `.md` 示例代码路径导致的假断链自污染，使现网 `links audit` 恢复 `100/100` 健康分；
    - **Systemd 守护工具链探测 & Benchmark `v3.2`**：让 `PreflightChecker` 自动识别 NVM/Cargo 路径下的 `codegraph`；对齐 `DQ3` 物理健康度口径并校准 `COMP-C4` 等价采集基线（详见 [roadmap-phase37.md](docs/src_doc/archive/roadmap-phase37.md)）。
  
21. **全生命周期信息新鲜度治理与现实对账体系 (`Information Freshness & Reality Reconciliation`)** `✅ Phase 38 已落地`:
    - 彻底治理七大陈旧漏斗（`STALE-01~07`）：文件修改后写穿重置旧向量 (`embedding = '[]'`, `embedding_status = "stale"`) 与旧 L2 标注 (`needs_re_enrich`)；文件删除后 FTS 强制 `status = 'active'` 零僵尸召回并级联清理 `fact_nodes` / `links` / `auto_sync` 记忆；`AutoSyncEngine` 对同路径摘要记忆幂等去重归档；`PropertyGraphOrganizer` 与 `FreshnessReconciler` 自动剪除已关闭的幽灵端口节点、核销 `scripts/backfill_embeddings.py` 回填态 (`"ready"`) 并核验 `.codegraph/codegraph.db` 滞后度；现网 `DQ5` 抽检 `598` 项达成 **`100.0%` ✅**（详见 [roadmap-phase38-39.md](docs/src_doc/archive/roadmap-phase38-39.md)）。
  
22. **内生对比探针驱动的闭环自进化引擎 (`Probe-Driven Closed-Loop Self-Evolution`)** `✅ Phase 39 已落地`:
    - 在 Agent 执行回路的 5 个关键切面（`P1 开工 Brief` / `P2 事前 Pre-Hook` / `P3 检索效用` / `P4 事后漂移` / `P5 周期回归与图谱`）部署微秒级对比探针，以物理真值、SLA 基线与历史 Benchmark 批次为参照系量化输出数字差值（Before/After/Delta），并集成去重幂等的 `FeedbackEngine::add_item_deduped` 自动驱动 `user_feedback.md`、自愈对账与 Benchmark `v3.3` (`W12` + `DQ5`) 闭环自进化；首轮部署即由 `P3` 探针真实捕获 `stale_vec=3` 缺口并自愈至 **`5/5 ✅`**，现网 `v3.3` 全量评测 `D1 -99.65%`、`D6 37.6 → 99.8 (+62.1)` 全面通过（详见 [13-freshness-and-self-evolution-spec.md](docs/design_doc/13-freshness-and-self-evolution-spec.md)）。
  
23. **图谱增强：多维证据分层、有向加权与隐式语义关系挖掘** `✅ Phase 40 已落地（对应规约 #5, #15, #23）`:
    > 原始探讨：图谱增强，也就是说事实、服务等之间存在的关系，关系的强弱，关系的方向等。这个似乎除了明面上的联系，实则还存在看似断连确存在语义上的联系，例如两个项目，一个服务和一个项目，两个服务之间都可能存在关系。
    - **现网实证动机**：实测 `/root/.agent-body/body.db` 中 `fact_nodes` 已达 `355` 节点（`354` 个拥有向量），但 `fact_edges` 仅 `49` 条边，`project` 节点 `214/215` 孤立（**99.5%**），`links` 表 `5,116` 条文件引用未上卷，服务单元与运行进程同体分裂；
    - **演进方案**：已落地 **`L1_EXPLICIT`（物理硬依赖, `0.85~1.00`） / `L2_STRUCTURAL`（文件引用上卷 + 服务桥接 + 工具依赖, `0.55~0.84`） / `L3_SEMANTIC`（向量余弦 $\ge 0.78$ + 标签混合发现, `0.35~0.65`, `Top-K=3` 剪枝）** 三层有向加权关系引擎与跳数衰减爆炸半径推演（详见 [14-graph-intelligence-and-mesh-protocol-spec.md](docs/design_doc/14-graph-intelligence-and-mesh-protocol-spec.md) 与 [roadmap-phase40-41.md](docs/src_doc/archive/roadmap-phase40-41.md)）。

24. **Mesh 协同协议：基于签名信封、防重放认证的跨机对话、沙盒文件投递与图谱联邦** `✅ Phase 41 已落地（对应规约 #2, #16, #19, #24）`:
    > 原始探讨：mesh, 除了能通信，是否还要设计通信协议，基于通信的功能点，例如传送文件，交换对话，对话间的身份认证安全措施等。
    - **演进方案**：遵循递弱代偿原则（不造 Raft/Rsync/聊天软件 IM），已落地 **`MeshEnvelope v1.0`** 线协议（`HMAC-SHA256` 规范签名 + `+-300s` 时间窗与 `NonceCache` 防重放 + `Peer Scopes` 最小权限），在此之上交付 **跨机图谱联邦探查 (`mesh/graph`)**、**双向脱敏抗投毒结构化对话 (`mesh/dialogue`)** 与 **限额 `<=10MB` + SHA-256 校验 + `~/.agent-body/mesh_inbox/` 隔离沙盒投递 (`mesh/transfer`)**，同步补齐 `W13`/`W14`/`DQ6` Benchmark `v3.4`（详见 [14-graph-intelligence-and-mesh-protocol-spec.md](docs/design_doc/14-graph-intelligence-and-mesh-protocol-spec.md) 与 [roadmap-phase40-41.md](docs/src_doc/archive/roadmap-phase40-41.md)）。

25. **Agent 并发控制：乐观直通 + 悲观门禁自适应控制 (AOPCC)** `✅ Phase 42 已落地（对应规约 #8, #18, #22, #25）`:
   > 原始探讨：我从数据库悲观锁和乐观锁想到一个问题，如果你没次都从agent-body获取（执行回路中强制）这不就悲观锁么，但这一定好吗？有时乐观锁可能更合适，例如我要做个postgresql验证，我可以假设本机5432已经默认有一个可用的实例，且无需密码，直接就连了。不需要从agent-body先获取一次事实信息啊。你觉得呢？这个想法对接下来的agent-body产品优化有什么作用么？

   - **核心收敛判断**：在 Agent 执行回路中强制事前查 `agent-body` 本质是**悲观并发控制（PCC）**——即便 `agent-body` 响应仅 `~1 ms`，也会迫使 Agent 多走一整轮 LLM 对话（重发 `5k~30k` 上下文 + `1.5s~4s` 推理延迟，即**整轮对话税 $C_{\text{turn}}$**）。面向本机 PostgreSQL `:5432` 等高置信、可逆、低回滚代价的只读/试探动作，应采用**乐观直通（`OPTIMISTIC`）**；而涉及破坏性写操作、端口绑定冲突、多 Agent 写锁与生产路径变更时，仍强制**悲观门禁（`PESSIMISTIC_LOCK`）**；介于两者之间的依赖写入采用**轻量条件校验（`CAUTIOUS_CAS`）**。
   - **已落地三层引擎**：(1) `OptimisticLeaseEngine` 在 `SessionStart (body_brief)` 签发 `<=120 tok` 的 **MVCC 短时乐观直通契约 (`Optimistic Leases`)**；(2) `ConcurrencyPolicyClassifier` 精确区分客户端连接（如 `psql -p 5432`）与服务端端口绑定（如 `uvicorn --port`）；(3) `OptimisticFailureDiagnoser` 在 `PostToolUse(success=false)` 与 `body_bridge.py` 中捕获 `ECONNREFUSED`、`password authentication failed`、`command not found`、`EADDRINUSE` 并在毫秒级内反查宿主机注入替代处方且自动降级失效租约，同步交付 Benchmark `v3.5`（`W15` + `PR-11`，详见 [15-adaptive-optimistic-pessimistic-concurrency-spec.md](docs/design_doc/15-adaptive-optimistic-pessimistic-concurrency-spec.md) 与 [roadmap-phase42.md](docs/src_doc/archive/roadmap-phase42.md)）。

26. **双域全链路行为审计、因果溯源与数据驱动自进化 (`IN_AGENT` + `OUT_OF_AGENT`)** `✅ Phase 43 已全量落地（对应规约 #4, #8, #18, #22, #26）`:
   > 原始探讨：agent-body 在接入agent后，行为的审计和有据可查，还要可供产品的自进化和数据分析。 agent-body 在 agent 之外的行为，例如主动或被动维护宿主机的对象或其他行为的审计和有据可查。

   - **现网实证动机**：实测 `/root/.agent-body/body.db` 中 `changes` 表共 `10,727` 条，其中 `watcher:file_modified` 占 `10,159` 条（`94.7%` 未折叠 I/O 洪泛），而 `agent_hook` 仅捕获含 `path` 的工具调用、`pre_tool_call` 与 19 个无 ROI 规则的 MCP 工具静默无审计；更关键的是 `BackgroundScheduler` 巡检、`FreshnessReconciler` 剪枝、`HostGarbageCollector` 删除、`LinkRepairer --apply` 改写文件、`PlacementLearner` 规则自学习与过期锁 GC 等 **Agent 之外的宿主机变更行为 0 审计记录**。
   - **演进方案**：建立 **`IN_AGENT`（Brief/Hook/MCP/CLI/Web/Mesh）+ `OUT_OF_AGENT`（`PROACTIVE_DAEMON` / `REACTIVE_WATCHER` 60s 折叠防洪 / `GOVERNANCE_MAINTENANCE`）** 双域统一行为审计引擎（`audit_events` 表 + `SecretRedactor` 脱敏 + `before_after_delta` + `evidence_sha256` 存证），并打通 **Audit → AOPCC 租约动态升降级 / Placement 聚类 / 反馈自动闭环** 与 **DuckDB 多维 OLAP 导出**，同步交付 Benchmark `v3.6`（`W16` + `DQ7` + `PR-12`，详见 [16-dual-domain-audit-and-self-evolution-spec.md](docs/design_doc/16-dual-domain-audit-and-self-evolution-spec.md) 与 [roadmap-phase43.md](docs/src_doc/archive/roadmap-phase43.md)）。
  
27. **产品定位升维：「脑体分离 · Agent 本体」及其功能/物料重塑** `✅ Phase 44~45 已全量落地（对应规约 #1, #2, #3, #8, #11, #13, #17, #27）`:
    > 核心探讨：为了让未接触项目的用户直观理解本系统定位（既非管理/驱动多个 Agent 的编排框架，也非面向人类用户的个人助理），从第一性原理彻底确立为“为 Agent 服务的物理宿主机具身底座——**Agent 本体（Agent Body）**”。所有感知盘点、规则防撞与跨脑连续性均统一收拢为本体的六大生理器官，并全面同步文档、代码注释、Web UI 与 HTML 演说物料。

    - **核心收敛判断（方案 B）**：确立 **「脑体分离（Brain-Body Separation）」** 第一性原理——LLM/客户端是可热切换的瞬态大脑，本系统是永驻宿主机的 **Agent 本体（感官图谱 × 小脑反射 × 海马体记忆与接力 × 规约脊柱 × 代谢稳态 × 多体联邦）**；严守递弱代偿不内置主战规划循环，并将工程标识（`agent-body` / `body_*` / `~/.agent-body/`）全面对齐为 Agent 本体语义。
    - **规划演进路线**：**Phase 44** 新增第 27 模块 `src/body/` 与第 27 张表 `body_checkpoints`，落地五大本体原语（`BodyManifest`、`ContinuityCapsule <=300 tok`、`HomeostasisVitals`、`AutonomyEnvelope L0~L3`、多体联邦名片发现）、第 43 原生工具 `body_state` 与 Benchmark `v3.7` (`W17` + `DQ8` + `PR-13`)；**Phase 45** 全面刷新注释/文档/`SKILL`/Web 路演剧场第 7 幕，并为 `#28` → `#29` 铺平零返工衔接序（详见 [04-agent-body-philosophy.md](docs/idea_doc/04-agent-body-philosophy.md)、[17-agent-embodiment-and-continuity-spec.md](docs/design_doc/17-agent-embodiment-and-continuity-spec.md) 与 [roadmap-phase44-45.md](docs/src_doc/archive/roadmap-phase44-45.md)）。
  
28. **体系化演讲 HTML 矩阵与既有双 Deck 归拢终刷新 (`presentation/`)** `✅ Phase 46 已全量落地`:
    > 原始需求：新建 presentation 目录，把agent-body-deck.html presentation.html git mv 进去，同时规划新的演讲 HTML，风格与它们保持一致，内容则可以从当前项目中提取整理成体系化的 HTML，每一个HTML讲30分钟左右，但要细、展开，所以每个HTML不要太多的点进去，最好一个点说透，所以需要体系化。另外agent-body-deck.html presentation.html最后再刷新一下，把当前项目状态放进去，现在似乎有点老了。
    - **已落地交付**：新建 `presentation/` 并 `git mv` 迁入 `agent-body-deck.html` 与 `presentation.html`（同步更新 `src/web/assets/mod.rs`、`README.md`、`docs/manual/08-web-dashboard.md`）；按完全一致的「浅色纸基 · 地层剖面 · 工程蓝图」视觉规范构建 `scripts/deck_engine.py` + `scripts/build_decks_part{1,2}.py`，交付 **`01~06` 共 6 讲 × 30 分钟单点说透专题 HTML** + **`presentation/index.html`**，并将旧版双 Deck 刷新至 `v0.4.0` / Agent 本体 / 43 工具 / 82 SPI / 268 测试 / 7 大剧目 / Bench `v3.7` 全绿现状，全部 9 份 HTML 通过 `scripts/verify_presentations.py` Playwright 无头验真（详见 [roadmap-phase46.md](docs/src_doc/archive/roadmap-phase46.md)）。
  
29. **根目录文档结构规整（收拢至 `docs/`）、全仓路径刷新与 CodeGraph 重建** `✅ Phase 47 已全量落地`:
    > 原始需求：当前的目录结构有点乱，主要是文档目录，都在一级目录里，感觉上有点乱，如果要改的话用 git mv 改名或改层级。并且移动完后，需要把项目中的文件(除了target )都扫一遍，涉及到旧路径的都要刷新为新路径。 最后还要刷新 codegraph.
    - **已落地交付**：使用 `git mv` 将 7 个分散一级文档目录（`arch/`、`design_doc/`、`idea_doc/`、`manual/`、`markdown/`、`releases/`、`src_doc/`）统一收拢至 `docs/` 并新增 [docs/README.md](docs/README.md) 索引，根目录精简为 `docs/`、`presentation/`、`scripts/`、`skills/`、`src/`、`tests/` 6 大一级目录；全仓扫描（排除 `target/.git/.codegraph`）更新 90 个文件的旧路径与相对层级链接（27 份核心活跃文档 0 断链），同步刷新 3 套全局 `SKILL`、9 份 `presentation/*.html`、`make evolution-check`（0 drift）与 `.codegraph/` 符号地图。

30. **排查并根治 `agent-body daemon` CPU 飙升至 600% 与 1.3G 内存峰值问题** `✅ 已根治验证`:
    > 原始反馈：agent-body daemon 进程 CPU 居高不下（最高 600%），需要排查解决。
    - **实证根因**：并非本地 Ollama Embedding 引起（Ollama 跑在独立 `ollama.service` cgroup，daemon 调用为单线程阻塞 HTTP）；真正元凶是 Web UI 打开「治理/审计」页并发触发 `DuckDbAnalyticsEngine::from_synced_storage()` → `SyncPipeline::sync_all()`：每次请求都新建 8 线程无上限内存 DuckDB 实例，对 ~3.5 万行数据（21,249 `assets` + 11,577 `changes` + 1,148 `audit_events` 等）执行 **~3.5 万次独立自动提交事务**（单次耗 `174s`，并发请求直接打满 600%~800% CPU 与 `1.3G` 内存峰值并导致前端 `Broken pipe`）。此外 `FileWatcher` 递归监听曾持有 `7,825` 个含 `target/.git` 的无效 inotify 句柄。
    - **三层根治**：① `DuckDbStorageProvider` 默认收敛 `SET threads = 2; SET memory_limit = '256MB'`，并新增显式事务 API；② `SyncPipeline::sync_all()` 改为单事务批量提交，新增 `sync_audit_events_only()` / `from_synced_audit_events()`（审计汇总/导出不再拷贝 2 万+ 资产），`from_synced_storage()` 增加 15s 单飞快照缓存；③ `FileWatcher` 改为剪枝非递归监听（自动跳过 `target/.git/node_modules/.codegraph` 并动态补挂新建子目录）。

31. **全面更名 `agent-body`（方案 B 彻底硬切 · 代码/MCP/CLI/Web/Hooks/Mesh/SKILL/HTML/活跃文档/宿主机生产平滑切换）** `✅ Phase 48 已全量落地`:
    > 原始需求：项目名要改成 agent-body ，相关的代码、文档、WEB UI、MCP、SKILL、HTML 等配套都要修改。
    - **已落地交付**：Crate 与二进制全面切为 `agent-body` / `agent_body`；43 个 MCP 工具全面切为 `body_*`（第 43 工具特判为 `body_state`）；默认数据目录切为 `~/.agent-body/body.db`（300 MB 现网库零损直迁并留存 `/root/.agent-body.bak-20261122` 备份）；Mesh 协议头切为 `X-Body-*`；Hook 桥接切为 `body_bridge.py`；Systemd 守护切为 `agent-body.service`；`skills/agent-body-*` 与 `presentation/agent-body-deck.html` 全部 `git mv` 重命名并刷新；全量 268 项测试、Playwright 9/9、Bench `v3.7` 与 `make evolution-check`（0 drift）全绿（详见 [roadmap-phase48.md](docs/src_doc/archive/roadmap-phase48.md)）。当前仓库目录名保留不动，待用户后续手动改名。

32. **本体内生等待事件 (Wait Events)、`/proc/self` 内核探针、AWR 快照黄金基线差分与 ADDM 自优化 (`Phase 50`)** `✅ Phase 50 已全量落地（对应规约 #2, #8, #9, #16, #27, #28）`:
    > 原始探讨：数据库有等待事件、统计信息采样快照、ebpf、AWR等分析性能的手段，是否可以借鉴到当前项目？目的是设计一个可以用于项目自身性能基准抓取或设定,性能诊断,优化的功能。
    - **演进方案**：遵循单二进制零外部依赖与 43 工具零膨胀原则（不引入重型 `libbpf/OTel`），在 `src/body/perf_awr.rs` 落地四大内生性能工程支柱：① **无锁等待事件直方图 (`WaitEventRegistry`)**：划分 `Lock` / `IO` / `Net` / `CPU` / `IPC` / `Idle` 6 大类 18 事件 + 8 档对数分位桶 + RAII `WaitEventGuard`（单次开销 `<100ns`）；② **零特权 `/proc/self` 内核探针 (`KernelSelfTelemetry`)**：直读 `VmRSS`、`VmHWM`、`open_fds`、`voluntary/nonvoluntary_ctxt_switches`、物理读写字节与 SQLite WAL 尺寸；③ **AWR 快照、黄金基准 (`Golden Baseline`) 与差分报告**：支持抓取快照、设定黄金基线、计算 `Body Time` 分解与资源漂移率；④ **ADDM 5 大瓶颈诊断规则与 `L1` 一键自调优 (`perf_tune`)**：联动 SQLite `PRAGMA optimize/wal_checkpoint/shrink_memory` 与 Linux `malloc_trim(0)`，内聚挂载于 `body_state` (`perf_snapshot` / `perf_awr` / `perf_tune`) 实现 CLI/MCP/Web 1:1:1 同步（详见 [18-wait-events-and-awr-perf-diagnostics-spec.md](docs/design_doc/18-wait-events-and-awr-perf-diagnostics-spec.md) 与 [roadmap-phase50.md](docs/src_doc/archive/roadmap-phase50.md)）。

33. **代码质量治理、`W10`/`DQ5` SLA 全绿修复、位置无关性硬化与五重质量门禁 (`Phase 49`)** `✅ Phase 49 已全量落地（对应规约 #9, #11, #13, #28）`:
    > 原始反馈：WEB UI 中实证基准与账本中执行真机运行A/B基准（全量套件）时，我看到打叉的 SLA FAIL 以及打叉的 W10 · 复杂模板依赖抽取抗假阳性 FAIL · 省 88.9% · 质量 56→90 (+34.0) · 事实 2/2 · 密度 9.0x。改过名字后，似乎代码质量下降了，请治理一下代码质量。
    - **实证根因**：① `W10` 在生产基准代码中硬编码了编译期宏 `env!("CARGO_MANIFEST_DIR")`，仓库改名为 `agent-body` 后旧二进制指向失效的 `/root/new/work/agent-body/README.md`，导致 `.md` 示例免疫失效产生 1 条假阳性；② 目录改名后现网遗留 `247` 个失活资产、`98` 个幽灵节点与 `10` 条冗余记忆，使 `DQ5` 跌至 `45.3%`；③ `Dockerfile`/`docker-compose.yml`/`config.example.toml` 残留 `/.agent-body` 路径断链，且缺少代码卫生/位置无关/吞错门禁。
    - **已落地治理**：移除生产路径 `CARGO_MANIFEST_DIR` 实现 100% 位置无关二进制；修复 Docker/配置/CLI Help/Web UI 残留；治理 `scheduler`/`watcher`/`probes`/`property_graph`/`links` 核心自治路径静默 `let _ =` 吞错；新增 `scripts/code_hygiene.py` 与 `make quality-gate` 五重门禁；模块化拆分 `mcp_schemas.rs`、`cli_args.rs`、`benchmark/workloads/{mod,advanced}.rs`；收敛 SQLite 页缓存 + 巡检后 `malloc_trim(0)` 压降守护进程 RSS；升级 README 规约 #11/#13 并新增规约 #28（详见 [roadmap-phase49.md](docs/src_doc/archive/roadmap-phase49.md)）。

34. **README.md 项目结构中根目录核心文件逐项描述与防漂移门禁 (`Phase 51`)** `✅ Phase 51 已全量落地（对应规约 #20）`:
    > 原始需求：README.md 项目结构中，根目录核心文件中的每个文件都要有描述。
    - **已落地交付**：将 `README.md`「项目结构」根目录全部 **15 个核心文件**（含补齐的 `.gitignore` 与 `.dockerignore`）逐一登记并附精准职责说明，校准 2 处计数微漂移（`270+` 测试 / `Benchmark v3.7`）；升级 `scripts/evolution_manifest.py --check` 新增根目录文件自文档化防漂移校验并同步升级 `README.md` 规约 `#13/#14/#17/#20`（详见 [roadmap-phase51.md](docs/src_doc/archive/roadmap-phase51.md)）。

35. **旗舰双演说 HTML (`presentation.html` & `agent-body-deck.html`) Decks-as-Code 推翻重写与 Playwright 门禁 (`Phase 51`)** `✅ Phase 51 已全量落地（对应规约 #13, #17）`:
    > 原始需求：presentation/presentation.html, agent-body-deck.html 已经不能完全传达现在的项目了，请先吸收内容，然后基于当前项目的代码和其他文档，推翻重写。
    - **已落地交付**：彻底告别 Phase 24 手工打补丁旧骨架，构建确定性代码生成器 `scripts/build_flagship_roadshow.py` 与 `scripts/build_flagship_techdeck.py`（复用 `scripts/deck_engine.py`「地层剖面 / 工程蓝图」视觉体系与 `G/N/T/F/P/A/S/R` 交互），推翻重写 **16 页对外路演版 (`presentation/presentation.html`)** 与 **24 页深度技术交流版 (`presentation/agent-body-deck.html`)**，完整传达脑体分离六大器官、L1~L3 分层图谱、AOPCC 乐观-悲观并发控制、写穿新鲜度与换脑不失忆、`MeshEnvelope v1.0`、双域审计与 `P1~P5` 自进化、内生 Wait Events/AWR/ADDM、Benchmark `v3.7` 帕累托双门禁与四赛道竞品实测；同步刷新 `presentation/index.html`、新增 `make verify-decks` Playwright 无头验真 9/9 全绿并重编重启服务同步 Web UI 内嵌演说（详见 [roadmap-phase51.md](docs/src_doc/archive/roadmap-phase51.md)）。

---

### 方案 A · `v1.0.0 GA` 工业级收口演进总账（Phase 52 ~ Phase 57 · 2026-11-22 立项）

36. **`[P0 · Phase 52]` 安全止血、Dockerfile 编译阻断修复、Tokio/边界 Panic 排雷与 P0 损毁文档/SKILL 急救** `✅ Phase 52 已全量落地`:
    - 修复 `Dockerfile` (`rust:1.88-bookworm`、补 `presentation/`、`EXPOSE 8765`、`HEALTHCHECK`)、`docker-compose.yml`、`.dockerignore`、`build.rs`；
    - 消除 MCP `run_stdio` 阻塞 Panic、`contacts`/`embedding` Tokio drop panic 及 `placement`/`port`/`powercontext`/`state_machine` 边界缺陷；
    - 修复 `body_bridge.py` 无端口破坏性命令绕过、Web `GET` 写库免鉴权、Mesh `read:facts` scope、`SKILL.md` 抗注入、`gc`/`port` 脱敏与 `AutonomyEnvelope` fail-closed；
    - 纠错 `ledger.rs` 死锁误报/漏记、`handle_mesh_graph` URL 解码、`cli_args.rs` `bool` 与错误模板 CLI，并急救修复 `04`/`17` 损毁文档与 3 套 `SKILL.md` 断链/镜像（详见 [roadmap-phase52.md](docs/src_doc/roadmap-phase52.md)）。

37. **`[P0 · Phase 53]` 存储三后端（SQLite/PG/DuckDB）语义对齐、全量迁移与 OLAP 管道真修复** `✅ Phase 53 已全量落地`:
    - 扩展 SPI `82 → 87`（新增 `upsert_scan_policy`/`list_all_memories`/`delete_benchmark_run`/`list_all_benchmark_runs`/`list_created_at_map`），对齐 SQLite/PG/DuckDB 冲突与时间戳瞬间比较、移除 PG 强依赖 `pgvector` 阻断、修复 `awr:%` 泄漏与淘汰孤儿清理，并打通 20 实体全量幂等热迁移、10 表真实时间 OLAP 同步与 `Preflight` 后端探活（详见 [roadmap-phase53.md](docs/src_doc/roadmap-phase53.md)）。

38. **`[P1 · Phase 54]` Phase 50 18/18 WaitEvent 全埋点、黑白屏动作级真 Parity、TUI 升级与 Benchmark `v3.8`** `✅ Phase 54 已全量落地`:
    - 新增 SPI #88 `count_expired_coordination_locks`（SQLite/PG/DuckDB 全实现）修复 `vitals` 过期锁检测，补齐 14/18 未埋点 `WaitEventId`（18/18 全覆盖使 `ADDM-05` 可达），修复 `estimate_percentile_from_buckets` / `run_l1_auto_tune` 缺陷并落地 `daemon-auto` 周期快照与 `delete_snapshot`/`delete_awr_report`；
    - 实现 AWR 报告库 CLI/MCP/Web 删除与查阅对齐、统一 `body_lock_release` 三端标识、修复 `parity/matrix` CLI 映射，升级 Web Benchmark 至 `v3.8` + `DQ1~DQ9` 渲染，扩展 `codex`/`hermes`/Claude `.claude/settings.json` Hook，升级 `src/tui/mod.rs` 并交付 Benchmark `v3.8` (`W18`/`M6`/`DQ9`/`PR-14`)（详见 [roadmap-phase54.md](docs/src_doc/roadmap-phase54.md)）。

39. **`[P1 · Phase 55]` 开源可移植性解耦（去 `/root/new/` 硬编码）、死配置真相化与跨平台/网络硬化** `✅ 已全量落地 (Phase 55)`:
    - 抽象 `[workspace]` (`WorkspaceConfig`) 与宿主自适应默认路径、`$USER` 动态 `default_owner` 及 SQLite/PG/DuckDB 动态种子，彻底清除生产代码 `/root/new/` 与 `user:digoal` 硬编码；
    - 清理失效 `api_port`/`api_bind` 并保持旧 `config.toml` 兼容，正式接通 `AGENT_BODY_CONFIG`、`[memory]` (`MemoryFactory`)、`[[hooks]]`/`[[pipelines]]` (`EventBus` 消费)、`assets.ignore_patterns`、`embedding.fallback_fts_only=false` 与 `[mesh]` 完整校验；
    - 完成 `malloc_trim` GNU glibc 限定、非 Linux `KernelSelfTelemetry` `sysinfo` 回退、Windows 多语言本地化 `netstat` 状态支持及 Web 32KB 有界 HTTP 头循环读取（详见 [roadmap-phase55.md](docs/src_doc/roadmap-phase55.md)）。

40. **`[P2 · Phase 56]` God File 模块化拆分（`>1500L` 清零）、吞错清零、集成测试补盲与五重门禁升级** `✅ 已全量落地 (Phase 56)`:
    - 拆分 `web/assets/mod.rs`（`index.html` + `style.css` + 5 份 `js/*.js`）、三存储 Provider（`{assets,facts,memory,locks,audit}.rs`）、`main.rs`（`cli_handlers/*`）、`parity_api.rs` 与 `perf_awr.rs`（`>1500L` 清零 `0` 个）；压降静默 `let _ =` (`115 -> 43`, 核心自治模块 `= 0` 并增设 `RunningFlagGuard`)；新增 5 套跨模块集成测试，升级门禁新增活跃断链、`18/18 WaitEventId` 覆盖、非基准 `/root/new/` 拦截与 `verify_presentations.py` `P/A/S/R/F`+`index.html` 页数校对（详见 [roadmap-phase56.md](docs/src_doc/roadmap-phase56.md)）。

41. **`[P1 · Phase 57]` 全文档/手册/SKILL/演说矩阵大一统、历史归档与 `v1.0.0 GA` 正式发布** `✅ 已全量落地 (Phase 57 · v1.0.0 GA)`:
    - 全量更新 `docs/manual/01~10`、`docs/arch/01~03`、`docs/design_doc/07,17,18`、`INSTALL.md`、`README.md`、`skills/agent-body-*`、`docs/markdown/`（含 5 张更新 SVG）与 `presentation/01~06`+双旗舰路演；去重重定向旧手册并归档 55 份历史 Phase 日志至 `docs/src_doc/archive/`；升级 `Cargo.toml` 至 `1.0.0` 并发布 [docs/releases/v1.0.0.md](docs/releases/v1.0.0.md)。

42. **多模态对象的理解和元数据 KV 对的增强 (`v1.2 · Phase 59`)** `✅ Phase 59 已全量落地`:
    - 落地三层级多模态理解 (`src/scanner/multimodal.rs`)：统一注入 `desc_kv["modality"]`，支持图片（PNG/JPEG/WebP/GIF/BMP 宽高色深 + SVG 文本关键词）、音视频（WAV/MP4/ffprobe）、PDF（页数/标题/文本摘录）、Office（docx/pptx/xlsx 幻灯片/工作表数与核心属性）及二进制数据库/列存（SQLite/Parquet/DuckDB 真实库表列 Schema）；
    - 新增 `[annotation]` 可选 `vision_enabled`/`vision_model`/`vision_max_image_bytes`，解除 `OllamaAnnotator` `3000/5000` 截断并打通多模态向量化文本拼接；
    - 新建 `AssetSearchEngine` (`src/scanner/asset_search.rs`) 实现 FTS5 + `assets.embedding` 余弦 RRF 混合检索及 `modality`/`domain`/`kv_key`/`kv_value` 过滤，1:1:1 透出至 MCP `body_asset_search`、CLI `asset search` 与 Web `/api/v1/assets/search`（详见 [roadmap-phase59.md](docs/src_doc/roadmap-phase59.md)）。

43. **三态运行模式（`bypass` / `shadow_ab` / `body_only`）与真实世界 A/B 数据收集分析** `✅ Phase 58 已全量落地`:
    - 原始需求：要有一个配置项目，3种配置模式，作用是：旁路 agent-body，该实时搜文件还是实时搜索文件就好像 agent-body 不存在；同时使用实时搜索和 agent-body，这个模式用于真实使用过程中的A/B测试的数据收集存储和未来分析，看看 agent-body 到底效果怎么样，是不是在帮倒忙？最后一种是默认的仅使用 agent-body。
    - 已落地交付：新增 `[runtime]` 配置（`mode` / `mcp_profile` / `shadow_primary` / `shadow_max_live_ms`）与 `AGENT_BODY_MODE` 热覆盖；落地 `ShadowAbEngine`（实时工作区扫描 vs FTS 索引双轨对照、陈旧命中检测、漏召回/帮倒忙率分析并持久化至 `audit_events` `category="ab_shadow"`）；同步落地 MCP `core`/`dev`/`ops`/`full` Schema 减税档位、原生 `agent-body hooks bridge` 及 CLI/MCP/Web 1:1:1 入口（详见 [roadmap-phase58.md](docs/src_doc/roadmap-phase58.md)）。

44. **弱向量补偿、行业 Know-How 知识网络与全信号 Omni-RRF 多路融合召回 (`✅ Phase 61~62 Stage 1 & Stage 2 已全量落地`)**:
    > 原始需求：不知道项目是否已经考虑过这个场景，我本地的llm模型参数量太小，实际上生成embedding可能也不好，召回率不一定高。或者支持的文本长度有限，未切分的文本或者即使切分，也容易在语义上失真，所以语义可能不够，还需要更多图的支持，把知识变成网络，变成行业know how.
    > 补充要求：如果是文本，应该结合语义、模糊查询、事实标量、bm25、全文检索、属性图关联片段、层级、文档之间的联系等的RRF融合查询。
    - **现网实证 GAP**：当前 `EmbeddingClient` 对输入文本 `chars().take(2000)` 硬截断、`fact_nodes`/`fact_edges`（SQLite `CHECK`）仅限于 7 类物理/工程拓扑、`L3_SEMANTIC` 语义边反过来强依赖小模型余弦 `>= 0.78`，且 `HybridSearchEngine`/`AssetSearchEngine` 仍停留在 0-hop 默认排序 FTS + 单向量（缺少 `bm25` 加权、模糊查询、事实标量调制、层级回溯与文档互链融合）；同时 `shadow_ab` 本机实测暴露代码驼峰标识符 FTS 漏召回问题。
    - **架构方案（方案 B · Omni-RRF）**：保持 27 表与 43 MCP 工具零膨胀，采用 **L1 确定性骨架（Heading-Aware 章节切片 + 别名归一 + SVG/标题术语提取 + PMI 共现）+ L2 小模型原子 Know-How 抽取（`qwen3.5:0.8b` `"think": false` + 原文锚定校验防幻觉）+ 全信号 Omni-RRF 融合召回**（整合 **BM25 全文 + Fuzzy 模糊 n-gram/编辑距离 + 可选 Semantic 向量 + 属性图 1~2 跳关联片段 + 章节/目录层级 Hierarchy + `links` 文档间联系 + 事实标量 `Phi_scalar` 过滤与时效/重要度调制**，输出 `matched_channels`/`reasoning_paths`），配套毛线球剪枝与写穿级联失效，分 Phase 61 与 Phase 62 实施（详见 [19-knowledge-network-and-graph-recall-spec.md](docs/design_doc/19-knowledge-network-and-graph-recall-spec.md)）。

45. **`[P1 · Phase 63]` 多模态零外依纯 Rust L1A 升级、`schema_version=2.0`、`W19/DQ10` Benchmark v3.9 与 `H1~H5` Shadow A/B 自主自愈** `✅ 已全量落地`:
    - 升级 `src/scanner/multimodal_ext.rs` 实现 SVG/PNG/JPEG/WebP/GIF/PDF/Office(OOXML) 与 7+3 二进制/文本格式纯 Rust 零外依解析，统一注入 `schema_version="2.0"`、12 项多模态指标、全链 `asset_chunk` 分块与 `backfill-multimodal` 存量回填；
    - `BenchmarkEngine` 升级至 **`v3.9`**（新增 **`W19` Multimodal Metadata & Know-How Backfill** 与 **`DQ10` Shadow A/B Falsification & Zero-Panic Probe**，门禁扩大至 `W:19, M:6, DQ:10, COMP:4, PR:14`）；
    - `ShadowAbEngine` 落地 `H1~H5` 自愈分类引擎，在 `ab_compare`/`ab_report` 中直接统计与触发驼峰切词/索引追平/记忆权重校准自愈（详见 [roadmap-phase63.md](docs/src_doc/roadmap-phase63.md)）。

46. **`[P0 · Phase 64 · v1.1.0]` 一等公民全景齐步走、Web UI 交互补全与版本对齐** `✅ 已全量落地`:
    - `Cargo.toml` 正式升版为 **`1.1.0`**，发布 [docs/releases/v1.1.0.md](docs/releases/v1.1.0.md) 与 [roadmap-phase64.md](docs/src_doc/roadmap-phase64.md)；
    - 补齐 Web UI 2.0（`index.html`、`02_topology_facts.js`、`05_body.js`）Phase 58~63 三态模式切换、`mcp_profile` 减税档位、Shadow A/B 对照面板及资产多维过滤（`modality`/`asset_type`/`kv_filter`/`fuzzy`/`include_links`/`backfill_multimodal`），统一默认端口 `8765`；
    - 升级 `scripts/evolution_manifest.py --check` 对活跃文档强制拦截过时 `Benchmark v3.8` / `W1~W18` / `DQ1~DQ9` 与错位 `docs/README.md` 指针，并完成 3 SKILL + 全量 `docs/manual/` + `docs/arch/` + `docs/design_doc/{06,10}*` + `INSTALL.md` + `EVOLUTION_GUIDE.md` + `README.md` 同步。

47. **`[P0 · Phase 65]` Shadow A/B 代码标识符召回/延迟优化与 `DQ2`/`DQ6`/`vitals` 可信度量修复** `✅ 已全量落地`:
    - 统一 `DQ2` 的 `target_pct` 与判定阈值为 `95.0%`；修复 `DQ6` 与 `HomeostasisVitalsEngine` 在十万级 `fact_edges` 下因 `LIMIT 4000/5000` 截断导致工程节点误报 `0/12` 与感官分 `60.0 [critical]` 的失真；
    - `expand_graph_for_query` 主键命中跳过全表 `LIKE` 扫表、限制 Hub 2-hop 爆炸；`AssetSearchEngine` 新增 `PreparedFuzzyQuery` 预计算与 `exact_id` 精确标识符 RRF 通道，解决 `ShadowAbEngine` 延时拖拽 (`257ms`) 与泛词挤掉精确文件问题（详见 [roadmap-phase65-66.md](docs/src_doc/roadmap-phase65-66.md)）。

48. **`[P1 · Phase 66]` macOS / Linux 跨平台体验打磨与 GitHub Actions CI/CD 工业化** `✅ 已全量落地`:
    - `preflight` 与 `daemon` 按平台分支探测与展示 `FSEvents` / `LaunchAgent` / `libproc`（彻底消除 macOS 下 Systemd `WARN` 与 `max_user_watches = 未知`）；`Makefile` 支持 `Darwin`/`Linux` 双平台安装、重启与无硬编码镜像同步；
    - 新增 `.github/workflows/ci.yml`（Ubuntu + macOS 全门禁 + Clippy + 全量测试）与 `.github/workflows/release.yml`（Tag 自动发布双平台二进制包与 SHA-256）。

49. **`[P1 · Phase 67]` 全仓生产文件 `< 1000L` 模块拆分、`prod_slice` 修复与 `let _ = <= 15` 吞错治理** `✅ 已全量落地`:
    - 拆分全部 `>= 1000L` 生产文件（`mcp_tools`、`benchmark/workloads`、`multimodal_ext`、`config`、`metadata`、`knowledge_graph`、`cli_args`、`awr_engine`、`preflight`），全仓所有生产 `.rs` 文件严格 `< 1000L`；
    - 修复 `scripts/code_hygiene.py` `prod_slice` 原始字符串误判并收紧门槛至 `>1000L = 0`、`let _ = <= 15`，清理全仓非 benchmark/核心路径静默吞错（详见 [roadmap-phase67.md](docs/src_doc/roadmap-phase67.md)）。

50. **`[P0 · Phase 68]` 一等公民 SSoT 交叉对齐与防漂移门禁闭环** `✅ 已全量落地`:
    - `src/web/parity_api/ops_endpoints.rs` 消除 `body_audit_query`/`body_state` 模糊前缀路由并精确锁定 `/api/v1/audit/events`、`/api/v1/body/manifest`，同时在 `src/web/mod.rs` 新增 43 工具 MCP ↔ CLI ↔ `server.rs` 交叉契约单测；
    - 升级 `scripts/evolution_manifest.py` 增加 `Cargo.toml` 版本对齐、活跃文档/SKILL 反引号源码路径 0 断链、`parity_api` 43 路由精确命中、9 份演说与 4 套生成脚本动态 SSoT 对齐四重硬门禁；
    - 重构 4 套 `scripts/build_*.py` 从 `evolution_manifest.py` 动态读取版本/规模/Bench 快照重生成全套 `presentation/*.html`，同步修复 `EVOLUTION_GUIDE.md`、`docs/arch/02`、3 份 `SKILL.md`、`README.md`、`INSTALL.md`、`docs/manual/{03,10}` 与 `v1.1.0.md` 漂移（详见 [roadmap-phase68.md](docs/src_doc/roadmap-phase68.md)）。

51. **`[P1 · Phase 69]` Top-5 高水位生产模块 `<700L` 预防性拆分与 SQLite/DuckDB 共享 SPI 契约测试** `✅ 已全量落地`:
    - 预防性拆分 `src/hooks/client_gen.rs`、`src/fact/property_graph.rs`、`src/scanner/multimodal.rs`、`src/benchmark/competitors.rs`、`src/api/mcp_schemas.rs` 为 10 个 `<700L` 高可读模块，彻底消除后续迭代触及 `1000L` 门禁风险；
    - 升级 `scripts/evolution_manifest.py` 兼容 `mcp_schemas_ops.rs` 与 `competitor_tracks.rs` 提取；
    - 新增 [tests/storage_spi_contract.rs](tests/storage_spi_contract.rs) 共享 12 域 `StorageProvider` SPI 契约测试套件，确保 SQLite 与 DuckDB 在放置/扫描/资产/向量/记忆/图谱/锁/审计/胶囊等核心方法上行为严格等价（详见 [roadmap-phase69.md](docs/src_doc/roadmap-phase69.md)）。

52. **`[P1 · 架构演进]` 借鉴 PostgreSQL 插件体系的微内核 (`kernel/`) 与周边 (`contrib/`) 彻底分离重构、双轨扩展与多发行版组装** `✅ Phase 70~73 (K1~K5) 已全量落地`:
    > 原始探讨：评估 `agent-body` 在省级政务云 AIOps 等场景的契合度与 GAP 后，确立彻底的架构拆分铁律——借鉴 PostgreSQL `src/backend/` + `contrib/` 设计哲学，**微内核 (`kernel/`) 与周边 (`contrib/`) 必须物理与逻辑彻底分离，周边功能绝不进内核；现网已识别出的行业/运维 GAP 绝不在重构前硬塞进内核，而是在完成微内核底座重构与存量周边剥离后，统一在独立 `contrib/` 目录下作为周边模块实现**。

    - **现网源码实证耦合与 GAP**：
      1. **内核与周边混杂、SPI 膨胀**：[src/storage/spi.rs](src/storage/spi.rs) 达 88 个业务耦合方法；[src/storage/schema.sql](src/storage/schema.sql) 用 `CHECK` 静态锁死实体/关系；[src/hooks/mod.rs](src/hooks/mod.rs) `BodyEvent` 封闭；[src/api/mcp_tools/mod.rs](src/api/mcp_tools/mod.rs) 静态路由分发；个人放置规则、Ollama 多模态、DuckDB（含 C++ `libstdc++6`）、Web 路演与 Benchmark 全焊死在主干（[Cargo.toml](Cargo.toml)）。
      2. **运维、政务云与安全合规 GAP（均应待重构后由 `contrib/` 承接）**：[src/web/server.rs](src/web/server.rs) 只读接口免鉴权且缺 mTLS/租户 RLS 与 SIEM 外发（[src/audit/mod.rs](src/audit/mod.rs)）；[src/scanner/redactor.rs](src/scanner/redactor.rs) 硬编码脱敏且 Java `-D` 等号易截断、[src/fact/automation.rs](src/fact/automation.rs) 未脱敏且漏扫 `/usr/lib/systemd/system`、`*.service.d`、RHEL/麒麟/openEuler `/var/spool/cron/*` 及 Unit 实时态；[src/env/mod.rs](src/env/mod.rs) 180 字符截断 Java 参数、[src/fact/process_net.rs](src/fact/process_net.rs) 缺 `started_at`/`ppid`/`ESTABLISHED` 且同端口 `dedup_by` 吞实例；[src/body/manifest.rs](src/body/manifest.rs) 缺 `host_labels`。
    - **核心架构方案（`kernel/` 纯机制微内核 + `contrib/` 独立周边目录 + 四大发行版）**：
      - **物理分层与准入红线**：`kernel/`（`body-sdk` + `body-protocol` + `body-kernel`）仅保留 100% 领域/OS/中间件/行业无关的纯粹机制——**8 张 `body_cat_*` 系统目录表**、5 张通用基表（含 `epoch_seq` 脑体增量漂移顺风车同步机制）、`ScopedSqlExecutor` 私有表前缀沙盒（`ext_<name>__*`）、**7 大生命周期 Hook 插槽**（`client_auth` / `object_access` RLS / `planner_rewrite` / `executor_start` / `executor_end` / `tuple_redact` / `emit_audit`）与 **6 大底层 Routine SPI**（`Type/Op`、`TableAmRoutine` ~18 原语、`IndexAm/OpClass` + `RrfMergeExecutor`、`FdwRoutine::poll_drift` O(1) 指纹短路游标、`ProcRoutine` 单点四端自动派生、`BgWorkerRoutine`）。
      - **`contrib/` 标准五件套与双轨加载**：所有周边模块迁入或新建于顶层 `contrib/<ext>/`（必备 `Cargo.toml` + `.control.toml` + `sql/` + `body_module_magic!` + `tests/regress.rs`，仅单向依赖 `body-sdk`）；支持 **轨 A 编译期静态 Crate** 与 **轨 B 进程外 `.control.toml` + JSON-RPC/UDS `L0` 只读沙盒**（强制出站 `TupleRedactHook` + 抗投毒清洗）。
      - **四大发行版谱系（`distributions/`）**：`kernel-minimal`（0 `contrib` 纯内核防腐包）、`agent-body`（个人旗舰全功能版，100% 兼容 `v1.1.0`）、`agent-body-probe`（政务云极简探针版，`<12MB`、`<20MB RSS`、零 DuckDB/C++/Ollama/WebUI）、`agent-body-dba`（数据库诊断版）；IAM/CMDB 控制面/堡垒机/K8s/SLB 严格留在外部平台侧（详尽规格见 [20-plugin-microkernel-architecture-spec.md](docs/design_doc/20-plugin-microkernel-architecture-spec.md)）。
    - **严格四阶段演进顺序（`Phase K1 → K4`，禁止抢跑塞入内核）**：
      - `Phase K1`：搭建 `kernel/body-sdk` + `body-kernel`（8 张目录表 + 7 大 Hook 插槽 + 6 大 Routine），**0 业务新功能混入内核**；
      - `Phase K2`：落地 `TableAmRoutine` 瘦身、`body_cat_proc` 自动四端派发与轨 B 沙盒代理；
      - `Phase K3`：建立 `contrib/` 目录，将存量 14 个周边模块（`duckdb`/`ollama`/`placement`/`mesh`/`web`/`demo`/`benchmark` 等）逐批剥离出内核，验证 `kernel-minimal` 独立全绿与 `personal-flagship` 100% 向后兼容；
      - `Phase K4`：在 `contrib/` 下全新开发 `probe_linux_ops`、`probe_jvm_middleware`、`hook_auth_mtls_rbac`、`hook_gov_compliance`、`hook_audit_siem`、`fdw_cmdb_ticket` 填平运维与政务云 GAP（内核 0 业务改动），组装并实证验收 `aiops-probe` 与 `dba-sentinel`。
    - **实证验收数据（详见 [roadmap-phase70-73.md](docs/src_doc/roadmap-phase70-73.md)）**：
      - 全工作区测试（`RUST_TEST_THREADS=1 cargo test --workspace`）：**387 passed, 0 failed**（根包 `agent-body` 349 + `kernel/` 3 crates 22 + `contrib/` 9 扩展包 12 + `distributions/` 3 发行版 4），`cargo clippy --workspace --all-targets -- -D warnings` 零告警；
      - 六重质量门禁（`make quality-gate`：`evolution-check`、`arch-check`、`lint-panic`、`lint-hygiene`、`lint-strict`）全绿（`kernel` 3 个核心 crate 单向依赖 0 业务词污染、`contrib` 9 个标准五件套 100% 合规、`0` `.unwrap()`/`.expect()`/`let _ =`、`0` 文件 `>1000L`）；
      - 三大精简 Release 发行版实测体积与 `self-check` 全绿：`agent-body-kernel-minimal` **3.8 MB**（`catalog=8, base=5, parity=1, audit=4`）、`agent-body-probe` **6.5 MB**（`ext=7, tuples=5, audit=4`）、`agent-body-dba` **6.1 MB**（`ext=5, db_instances=3, audit=1`）；`otool -L target/release/agent-body-probe` 仅链接 `libSystem.B.dylib` / `libiconv.2.dylib` / `CoreFoundation`，零 `libc++` / `libstdc++` / DuckDB 运行时依赖。
