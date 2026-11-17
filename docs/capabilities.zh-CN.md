# 能力与源码索引

本表与 Web 能力页共享 `docs/implementation-status.json`。能力按涌现路径分类：涌现机制是北极星目标路径，运行底座支撑个体执行，证据与验证保证可信交付，显式编排脚手架提供确定性协作路径，支撑能力与治理/规模化按需推进。状态描述已提供的功能与缺口；验证列仅适用于该行已提供的部分。平台概览见[当前平台状态](status.zh-CN.md)，分层与约束见[架构设计](managed-agents.zh-CN.md)。

## 涌现机制

| 能力 | 状态 | 验证范围 | 当前行为与限制 |
| --- | --- | --- | --- |
| 对等 Agent 共享任务板 | 已实现 | 原生测试 + Web 联调 | 任务发布无需绑定 Preset 或实例；控制面模型选型并创建实例，Agent 使用统一工具原子认领、上报进度、拆分和读取同根结果。来源成功后激活子任务，全部完成后原任务重新选型与认领；根任务执行成功后等待人类评审，通过后归档，附意见退回后重新选型执行，保留旧 Run 结果与最近 16 次评审摘要。每轮人类返工可补拆一次，整合续作不能重复拆分。独立选型模型支持三类通用协议及百炼 decision-model-preview，失败退避。任务视图已接入信息素痕迹浓度；Agent 读取默认局部邻域（scope=all 可退出）；根任务可选独立验证已接入：全新验证器实例审计冻结结果，通过自动归档（验证器署名）、失败有界返工、unknown 不通过、预算耗尽交回人类；消息招募与同根快照交接已接通；深度产物整合为后续方向，确定失败与未认领任务在有界重试预算（默认 2 次）内自动重排队、新实例接手，预算耗尽才阻塞上游；结果未知不自动重试。没有分布式租约或费用硬预算。自选模式下认领支持 self-selected 路由态。 |
| 信息素痕迹场 | 已实现 | 原生测试 | 根任务作用域痕迹浓度：完成/验收/复用按固定权重自动增强，成功 Run 招募放大其留下的路径信号，确定失败施加负反馈降权（结果未知不惩罚不重试），Agent finding/path/blocker 沉积有界（每 Run 8 次、每根 64 条）；局部感知默认只见浓度≥1.0 的痕迹（min_strength=0 显式全览）；30 分钟半衰期惰性蒸发、六位小数量化、饱和上限 100；台账与事件可审计。振荡抑制已落地：返工链指数冷却（15s×2^n 上限 120s）+链长上限 4，超限拒绝自动返工交回人类，验收清链。板根黑板已重建：CAS 修订+证据支持度自动裁决（强证据取代、弱/平局保留在位者、历史可审计）。 |
| 板根黑板与冲突裁决 | 已实现 | 原生测试 | 根任务作用域共享黑板：Agent 以稳定键+CAS 修订发布发现（每 Run 8 次、每根 64 键），引用最多 8 个痕迹键作为证据支持。冲突写入按证据强度自动裁决而非先到先得：更强支持取代在位者，弱/平局（相对容差防蒸发抖动）保留在位者并记为 superseded 历史；裁决事件可审计。工具 platform.board.notes/note，操作员视图 /api/board/tasks/{id}/notes。黑板内容为不可信证据。 |
| 响应阈值分工与拥挤规避 | 已实现 | 原生测试 | Preset 经验自适应响应阈值（成功×1/1.25、确定失败×1.25，夹[0.05,0.95]基线0.5；结果未知与取消不计经验），并按能量效率缩放（每成功均耗能/预留单位，夹0.25–4）：便宜专家响应更弱刺激，昂贵 Preset 需更强收益证据，经验统计随能力目录进入选型证据，操作员可查 /api/presets/experience。任务供给刺激=浓度/(1+同根活跃工作者数)，拥挤分支降权、跨分支分散；可选种子化概率探索（splitmix64）按刺激+1 抽样防锁死。选型接收最强 8 条痕迹的稀释刺激与拥挤数。阈值是证据不是权威，能力匹配优先。 |
| 个体自选认领（选型下沉） | 已实现 | 原生测试 | ANT3_SELF_SELECT=1 时中央选型分类器完全退位：游走待命池（上限 8）按验证经验繁殖（Laplace 平滑份额，持续失败 Preset 退役），空闲游走消耗群体能量（20/实例/分钟，群体池 20 万，耗尽退役全池）。每个待命个体按自己的成本调整阈值对开放任务做 s²/(s²+θ²) 概率响应（splitmix64 种子化），响应者原子竞争认领（先插 Run 后 UPDATE 门控，败者回滚）；控制面只保留容量/能量/原子性物理法则。自选 Run 幂等认领自己的工作；swarm.self_selected/peer_spawned/peer_retired/colony_starved 事件可审计；操作员视图 /api/swarm/pool。默认关闭，与选型模式可对照。 |
| 预算即能量 | 已实现 | 原生测试 | 根任务能量池（发布可配，默认 100000）：派发原子预留 2000/Run，结算按真实 usage 计费（10 token=1 能量，缓存输入免费；usage 缺失保守计 1500），完成回充 500、独立验收回充 4000——高效路径赚回预算，浪费路径饿死。可用能量不足预留的根停止供给，待认领任务显式淘汰（energy_exhausted）；结算幂等、账本与事件可审计，操作员视图 /api/board/tasks/{id}/energy。选择压力由此成为涌现机制而非会计治理。 |
| 可靠协作消息 | 部分实现 | 原生测试 + Web 联调 | 持久 Outbox/Inbox、稳定消息 ID、幂等去重、投递与消费分离、暂停积压和未知结果人工处置；Agent 使用 platform.messaging 直接交接。作用域=同 fleet 或同板根；板上 Agent 可向任意活跃同伴发起交接，消息接受时把发送者板根传播给收件人（招募台账 board_peer_roots），回复保持在作用域内。没有远程投递或 Grant 授权。 |
| 实例工作区与文件/JSON交接 | 部分实现 | 原生测试 + Web 联调 | 实例独立目录、UTF-8 文件读写、文件/JSON 不可变快照；同板根（邀请或招募）同伴可读取快照作为 stigmergy 交接，私有文件仍隔离；Web 预览/下载保留来源状态和摘要。没有 OS 沙箱或通用代码执行环境。 |
| 涌现评估基线 | 部分实现 | 原生测试 | 单/多 Agent 同任务同验收对照：确定性契约基线报告通过/验收、Run 成本代理、信息复用率、痕迹浓度、收敛时延（发布→验收）与能量净消耗；个体损失鲁棒性场景已验证。能量账本落地后等预算真实模型对照可配置，自动化运行未实现。不预设多 Agent 更优。 |

## 运行底座

| 能力 | 状态 | 验证范围 | 当前行为与限制 |
| --- | --- | --- | --- |
| Ant 执行循环 | 已实现 | 原生测试 | 模型→工具→模型循环、独立会话、观察与持久回执；运行内核可脱离平台使用。 |
| 不可变 Preset 发布 | 已实现 | 原生测试 | 用途、目标、指令、模型协议与插件按版本锁定；保存原始发布定义、单继承来源和能力摘要。 |
| Preset 继承与参数化 | 部分实现 | 原生测试 + Web 联调 | 固定版本单继承最多 16 层，参数 schema/默认值与创建时模板展开，最多 16 个插件的能力锁及冲突校验。没有主体权限收窄、预算/资源锁或在线升级。 |
| 有限批量实例化 | 已实现 | 原生测试 + Web 联调 | HTTP 以 202 接纳创建 Operation，协调器分批创建，每批最多 8 个实例；256 容量含待创建预留，幂等重试复用原回执。 |
| 实例生命周期与 TTL | 部分实现 | 原生测试 + Web 联调 | 支持暂停、恢复、终止、排空和 TTL，Agent 池支持 finite/maintain 扩缩容。没有空闲策略、stop/start、配置升级或单实例 Operation。 |
| Agent 池与异步 Operation | 部分实现 | 原生测试 + Web 联调 | Agent 池提供异步创建/扩缩容/终止、持久槽位与代际、容量预留和重启协调；没有排空期限、Operation 单独取消或远程执行配置。 |
| 受限 Agent 派生 | 部分实现 | 原生测试 + Web 联调 | Agent 按批准 Preset/参数派生子任务，宿主绑定父任务、Run 和实例；最多 6 层、单次 8 项、每根 48 个后代，板根共 64 项。没有任意子树授权、迁移或分布式租约。 |
| 持久队列与取消 | 已实现 | 原生测试 + Web 联调 | 持久 Run 队列提供 8 个本地并发槽、单实例串行、幂等提交与协作取消。 |
| SQLite 与恢复 | 部分实现 | 原生测试 | SQLite 事务、检查点、事件和 daemon 独占锁；恢复时保留已知回执，未知结果进入中断。没有完整备份运维、外部对账或事件分页。 |
| 三类模型协议 | 部分实现 | 原生测试 | 提供 Chat Completions、Responses、Anthropic 三类非流式工具循环；Responses 经过真实服务验证。没有流式输出或分片中断契约。 |
| 内置工具与提示词包 | 已实现 | 原生测试 | 内置 text.inspect、web.fetch、web.search；搜索需配置 Brave API 密钥。插件页仅展示内置能力，Preset 绑定固定版本与摘要。 |

## 支撑能力

| 能力 | 状态 | 验证范围 | 当前行为与限制 |
| --- | --- | --- | --- |
| 可执行插件契约与 SDK | 部分实现 | 原生测试 + Web 联调 | 版本化 manifest/RPC、严格 schema 子集、握手、健康、进度、取消、错误分类、Rust SDK 与 Python 示例；示例覆盖统计、HTTP 读取和文件交接。没有配置/权限/依赖锁、多能力包或完整对账协议。 |
| 外部插件与热加载 | 部分实现 | 原生测试 + Web 联调 | 底层兼容接口支持本地进程包安装、内容寻址、调用前摘要检查、版本并存、Preset 快照、健康检查及禁用/取消。需显式启用，以系统用户运行；没有 MCP loader、watch、进程池、rollout、沙箱或作用域授权。 |
| Web 执行证据检查 | 已实现 | 原生测试 + Web 联调 | 控制台以"人类观察蚁群"视角组织：观察（工作现场默认/蚁群花名册+一句话群体动态+开销收敛/成果库）、工作（任务板人话状态+被参考次数/需要你处理聚合人类决策点带角标）、设置（Preset/插件/研究者视图）。机制参数（痕迹场/能量账本/裁决历史/原始 runs/events/实例/池/实现进度）默认折叠于研究者视图；旧 hash 重定向。任务板展示信息素痕迹浓度徽章与根任务痕迹场面板（按强度排序、沉积次数、半衰期），随轮询刷新；能力页按涌现路径分类展示进度。 |
| API / CLI / MCP 入口 | 部分实现 | 原生测试 + Web 联调 | HTTP/Web、共享本地客户端、CLI 与 MCP stdio 共用状态；40 个 MCP 工具，默认开放 20 个查询工具。没有 MCP resources、Streamable HTTP、细粒度授权、并发取消或 CLI --wait。 |
| 实时事件与查询 | 部分实现 | 原生测试 | 持久事件游标与 Web 轮询；没有 SSE、通用列表游标或统一事件订阅契约。 |
| 发行、生态与验收套件 | 部分实现 | 原生测试 | 提供社区文档、CI 和源码本地启动；没有完整发行物/容器交付或插件分发生态；单/多 Agent 确定性对照基线已落地，真实模型对照未实现。 |
| 运行时机制参数 | 已实现 | 原生测试 + Web 联调 | 痕迹半衰期、上限与感知阈值，探索排序，自选池容量与待命消耗，能量默认值与回充等参数持久化；保存后从下一次调度、沉积或结算生效。 |

## 治理与规模化

| 能力 | 状态 | 验证范围 | 当前行为与限制 |
| --- | --- | --- | --- |
| 预算预留与账本 | 规划中 | 待实现 | 能量账本已落地（见预算即能量条目）：预留/结算/回充/淘汰。尚未提供：跨根团队费用硬上限、供应商限价对账、进行中请求的结算边界。 |
| 身份、授权与审批 | 部分实现 | 原生测试 | loopback、本地 token 与浏览器会话认证；没有 Principal/Grant、持久审批、租户隔离或插件作用域授权。 |
| 监督 Worker 与执行隔离 | 规划中 | 待实现 | 使用嵌入式本地 Worker；没有独立进程、所有权 fencing、跨进程租约或执行沙箱。 |
| 远程控制面与 PostgreSQL | 规划中 | 待实现 | 没有多机器 Worker、网络分区对账、分布式背压或滚动升级兼容契约。 |

## 当前模块 → 目标目录

| 能力 | 当前落点 | 目标目录 |
| --- | --- | --- |
| Ant 执行循环 | `crates/ant3-core/src/engine.rs`<br>`crates/ant3-core/src/session.rs` | `crates/ant3-core/src/` |
| 不可变 Preset 发布 | `crates/ant3-domain/src/lib.rs`<br>`crates/ant3-platform/src/storage/sqlite.rs`<br>`crates/ant3-platform/src/control/presets/mod.rs` | `crates/ant3-domain/src/presets/` |
| Preset 继承与参数化 | `crates/ant3-platform/src/control/presets/mod.rs`<br>`web/src/features/presets/presets.js`<br>`crates/ant3-server/tests/presets.rs` | `crates/ant3-domain/src/presets/` |
| 有限批量实例化 | `crates/ant3-platform/src/control/fleets.rs`<br>`crates/ant3-platform/src/storage/sqlite.rs` | `crates/ant3-platform/src/control/` |
| 实例生命周期与 TTL | `crates/ant3-platform/src/control/lifecycle.rs`<br>`crates/ant3-platform/src/storage/sqlite.rs` | `crates/ant3-platform/src/control/`<br>`crates/ant3-domain/src/agents/` |
| Agent 池与异步 Operation | `crates/ant3-platform/src/control/fleets.rs`<br>`web/src/features/agents/fleets.js`<br>`crates/ant3-server/tests/fleets.rs` | `crates/ant3-domain/src/operations/`<br>`crates/ant3-platform/src/control/` |
| 受限 Agent 派生 | 尚无实现模块 | `crates/ant3-domain/src/agents/` |
| 持久队列与取消 | `crates/ant3-platform/src/storage/sqlite.rs`<br>`crates/ant3-worker/src/runner.rs` | `crates/ant3-platform/src/scheduling/` |
| SQLite 与恢复 | `crates/ant3-platform/src/storage/sqlite.rs`<br>`crates/ant3-storage/migrations/sqlite/schema.sql`<br>`crates/ant3-storage/src/lib.rs`<br>`crates/ant3-storage/tests/database.rs` | `crates/ant3-storage/src/` |
| 三类模型协议 | `crates/ant3-models/src/lib.rs` | `crates/ant3-models/src/` |
| 内置工具与提示词包 | `plugins/src/lib.rs`<br>`plugins/text/mod.rs`<br>`plugins/web-fetch/mod.rs`<br>`plugins/web-search/mod.rs`<br>`crates/ant3-server/tests/builtin_plugins.rs` | `plugins/` |
| 可执行插件契约与 SDK | `crates/ant3-plugin-protocol/src/lib.rs`<br>`crates/ant3-plugin-sdk/src/lib.rs`<br>`crates/ant3-plugin-sdk/tests/contract.rs`<br>`crates/ant3-plugins/examples/text-inspector/manifest.json`<br>`crates/ant3-plugins/examples/http-reader/manifest.json`<br>`crates/ant3-plugins/examples/http-reader/test_worker.py`<br>`crates/ant3-plugins/examples/workspace-preset/preset.json` | `crates/ant3-plugin-protocol/src/`<br>`crates/ant3-plugin-sdk/src/` |
| 外部插件与热加载 | `crates/ant3-plugins/src/lib.rs`<br>`crates/ant3-plugins/src/package.rs`<br>`crates/ant3-plugins/src/process.rs`<br>`web/src/features/plugins/plugins.js`<br>`crates/ant3-server/tests/process_plugins.rs` | `crates/ant3-plugins/src/loaders/`<br>`crates/ant3-plugins/src/reload/` |
| 对等 Agent 共享任务板 | `crates/ant3-platform/src/scheduling/board/mod.rs`<br>`crates/ant3-platform/src/scheduling/board/tools.rs`<br>`crates/ant3-storage/migrations/sqlite/board.sql`<br>`crates/ant3-platform/src/scheduling/routing.rs`<br>`crates/ant3-platform/src/scheduling/decision.rs`<br>`web/src/features/board/board.js`<br>`crates/ant3-server/tests/board.rs`<br>`crates/ant3-server/tests/routing.rs`<br>`crates/ant3-server/tests/decision.rs`<br>`crates/ant3-platform/src/scheduling/audit.rs`<br>`crates/ant3-server/tests/audit.rs` | `crates/ant3-platform/src/collaboration/`<br>`crates/ant3-platform/src/scheduling/` |
| 信息素痕迹场 | `crates/ant3-domain/src/traces.rs`<br>`crates/ant3-platform/src/scheduling/traces.rs`<br>`crates/ant3-storage/migrations/sqlite/traces.sql`<br>`crates/ant3-platform/src/scheduling/board/tools.rs`<br>`crates/ant3-server/tests/traces.rs`<br>`crates/ant3-server/tests/evaluation.rs` | `crates/ant3-platform/src/scheduling/` |
| 板根黑板与冲突裁决 | `crates/ant3-domain/src/notes.rs`<br>`crates/ant3-platform/src/scheduling/notes.rs`<br>`crates/ant3-server/tests/notes.rs` | `crates/ant3-platform/src/scheduling/` |
| 响应阈值分工与拥挤规避 | `crates/ant3-domain/src/division.rs`<br>`crates/ant3-platform/src/scheduling/division.rs`<br>`crates/ant3-platform/src/scheduling/routing.rs`<br>`crates/ant3-server/tests/division.rs` | `crates/ant3-platform/src/scheduling/` |
| 个体自选认领（选型下沉） | `crates/ant3-domain/src/division.rs`<br>`crates/ant3-platform/src/scheduling/selfselect.rs`<br>`crates/ant3-server/tests/selfselect.rs` | `crates/ant3-platform/src/scheduling/` |
| 预算即能量 | `crates/ant3-domain/src/energy.rs`<br>`crates/ant3-platform/src/scheduling/energy.rs`<br>`crates/ant3-server/tests/energy.rs` | `crates/ant3-platform/src/scheduling/` |
| 可靠协作消息 | `crates/ant3-platform/src/collaboration/messaging/mod.rs`<br>`crates/ant3-platform/src/collaboration/messaging/tools.rs`<br>`web/src/features/messages/messages.js`<br>`crates/ant3-server/tests/messaging.rs`<br>`crates/ant3-server/tests/messaging.rs` | `crates/ant3-storage/src/inbox_outbox/`<br>`crates/ant3-domain/src/collaboration/` |
| 实例工作区与文件/JSON交接 | `crates/ant3-platform/src/workspaces/mod.rs`<br>`crates/ant3-platform/src/workspaces/tools.rs`<br>`web/src/features/workspaces/workspaces.js`<br>`crates/ant3-server/tests/workspaces.rs` | `crates/ant3-platform/src/workspaces/`<br>`crates/ant3-domain/src/artifacts/` |
| Web 执行证据检查 | `web/src/app/app.js`<br>`web/src/features/studio/studio.js`<br>`web/src/features/studio/studio.css`<br>`web/src/features/agents/fleets.js`<br>`web/src/features/messages/messages.js`<br>`web/src/features/workspaces/workspaces.js` | `web/src/features/` |
| API / CLI / MCP 入口 | `crates/ant3-server/src/api/mod.rs`<br>`crates/ant3-client/src/lib.rs`<br>`crates/ant3-cli/src/main.rs`<br>`crates/ant3-cli/src/commands.rs`<br>`crates/ant3-server/src/mcp.rs`<br>`crates/ant3-cli/tests/entrances.rs`<br>`crates/ant3-cli/examples/mcp.json` | `crates/ant3-client/src/`<br>`crates/ant3-cli/src/`<br>`crates/ant3-server/src/mcp.rs` |
| 实时事件与查询 | `crates/ant3-server/src/api/mod.rs` | `crates/ant3-server/src/events/` |
| 预算预留与账本 | 尚无实现模块 | `crates/ant3-platform/src/accounting/`<br>`crates/ant3-domain/src/budgets/` |
| 身份、授权与审批 | `crates/ant3-server/src/api/mod.rs` | `crates/ant3-platform/src/authorization/`<br>`crates/ant3-domain/src/identity/` |
| 监督 Worker 与执行隔离 | 尚无实现模块 | `crates/ant3-worker/src/` |
| 远程控制面与 PostgreSQL | 尚无实现模块 | `crates/ant3-storage/src/`<br>`crates/ant3-worker/src/backends/` |
| 涌现评估基线 | `crates/ant3-server/tests/evaluation.rs`<br>`docs/evaluation.zh-CN.md` | `tests/evaluation/` |
| 发行、生态与验收套件 | `.github/workflows/check.yml`<br>`docs/getting-started.zh-CN.md` | `deploy/`<br>`examples/`<br>`tests/`<br>`docs/` |
| 运行时机制参数 | `crates/ant3-platform/src/settings.rs`<br>`web/src/features/researcher/researcher.js` | `crates/ant3-platform/src/settings.rs` |

## 当前源码目录

以下从实际文件生成，列出 workspace 各 crate、独立 Web 源码与客户端/插件示例，不包含构建产物。架构设计中的目标树表达职责拆分，不要求预先创建空包。

```text
crates/
├── ant3-cli/
│   ├── examples/
│   │   ├── README.md
│   │   ├── board-task.json
│   │   ├── composed-preset.json
│   │   ├── fleet.json
│   │   ├── mcp.json
│   │   ├── parameterized-fleet.json
│   │   ├── process-fleet.json
│   │   └── process-preset.json
│   ├── src/
│   │   ├── commands.rs
│   │   └── main.rs
│   ├── tests/
│   │   ├── entrances.rs
│   │   └── startup.rs
│   └── Cargo.toml
├── ant3-client/
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── ant3-core/
│   ├── examples/
│   │   └── quickstart.rs
│   ├── src/
│   │   ├── context.rs
│   │   ├── driver.rs
│   │   ├── engine.rs
│   │   ├── interaction.md
│   │   ├── lib.rs
│   │   ├── policy.rs
│   │   ├── prompt.rs
│   │   ├── registry.rs
│   │   ├── session.rs
│   │   ├── skills.rs
│   │   ├── trace.rs
│   │   ├── transport.rs
│   │   └── work.rs
│   └── Cargo.toml
├── ant3-domain/
│   ├── src/
│   │   ├── board.rs
│   │   ├── division.rs
│   │   ├── energy.rs
│   │   ├── fleets.rs
│   │   ├── lib.rs
│   │   ├── limits.rs
│   │   ├── messaging.rs
│   │   ├── notes.rs
│   │   └── traces.rs
│   └── Cargo.toml
├── ant3-models/
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── ant3-platform/
│   ├── src/
│   │   ├── collaboration/
│   │   │   ├── messaging/
│   │   │   │   ├── mod.rs
│   │   │   │   └── tools.rs
│   │   │   └── mod.rs
│   │   ├── control/
│   │   │   ├── presets/
│   │   │   │   └── mod.rs
│   │   │   ├── fleets.rs
│   │   │   ├── lifecycle.rs
│   │   │   ├── mod.rs
│   │   │   └── organization.rs
│   │   ├── domain/
│   │   │   └── mod.rs
│   │   ├── scheduling/
│   │   │   ├── board/
│   │   │   │   ├── mod.rs
│   │   │   │   └── tools.rs
│   │   │   ├── audit.rs
│   │   │   ├── decision.rs
│   │   │   ├── division.rs
│   │   │   ├── energy.rs
│   │   │   ├── mod.rs
│   │   │   ├── notes.rs
│   │   │   ├── routing.rs
│   │   │   ├── selfselect.rs
│   │   │   └── traces.rs
│   │   ├── storage/
│   │   │   ├── mod.rs
│   │   │   ├── plugins.rs
│   │   │   └── sqlite.rs
│   │   ├── workspaces/
│   │   │   ├── mod.rs
│   │   │   └── tools.rs
│   │   ├── lib.rs
│   │   └── settings.rs
│   ├── tests/
│   │   └── support/
│   │       └── routing.rs
│   └── Cargo.toml
├── ant3-plugin-protocol/
│   ├── src/
│   │   └── lib.rs
│   └── Cargo.toml
├── ant3-plugin-sdk/
│   ├── examples/
│   │   └── text.rs
│   ├── src/
│   │   └── lib.rs
│   ├── tests/
│   │   ├── fixtures/
│   │   │   └── text-inspector.manifest.json
│   │   └── contract.rs
│   └── Cargo.toml
├── ant3-plugins/
│   ├── examples/
│   │   ├── http-reader/
│   │   │   ├── README.md
│   │   │   ├── manifest.json
│   │   │   ├── test_worker.py
│   │   │   └── worker.py
│   │   ├── text-inspector/
│   │   │   ├── manifest.json
│   │   │   └── worker.py
│   │   └── workspace-preset/
│   │       ├── README.md
│   │       └── preset.json
│   ├── src/
│   │   ├── lib.rs
│   │   ├── package.rs
│   │   └── process.rs
│   └── Cargo.toml
├── ant3-server/
│   ├── src/
│   │   ├── api/
│   │   │   ├── auth.rs
│   │   │   ├── handlers.rs
│   │   │   ├── mod.rs
│   │   │   ├── routes.rs
│   │   │   └── status.rs
│   │   ├── bootstrap.rs
│   │   ├── lib.rs
│   │   └── mcp.rs
│   ├── tests/
│   │   ├── fixtures/
│   │   │   └── process-plugin.py
│   │   ├── audit.rs
│   │   ├── board.rs
│   │   ├── builtin_plugins.rs
│   │   ├── decision.rs
│   │   ├── division.rs
│   │   ├── energy.rs
│   │   ├── evaluation.rs
│   │   ├── fleets.rs
│   │   ├── messaging.rs
│   │   ├── notes.rs
│   │   ├── platform.rs
│   │   ├── presets.rs
│   │   ├── process_plugins.rs
│   │   ├── routing.rs
│   │   ├── selfselect.rs
│   │   ├── settings.rs
│   │   ├── traces.rs
│   │   └── workspaces.rs
│   └── Cargo.toml
├── ant3-storage/
│   ├── migrations/
│   │   └── sqlite/
│   │       ├── board.sql
│   │       ├── routing.sql
│   │       ├── schema.sql
│   │       └── traces.sql
│   ├── src/
│   │   ├── error.rs
│   │   └── lib.rs
│   ├── tests/
│   │   └── database.rs
│   └── Cargo.toml
└── ant3-worker/
    ├── src/
    │   ├── lib.rs
    │   ├── recovery.rs
    │   ├── runner.rs
    │   └── scheduler.rs
    ├── tests/
    │   └── support/
    │       └── model.rs
    └── Cargo.toml
plugins/
├── src/
│   ├── lib.rs
│   └── network.rs
├── text/
│   └── mod.rs
├── web-fetch/
│   └── mod.rs
├── web-search/
│   └── mod.rs
├── Cargo.toml
└── README.md
web/
├── src/
│   ├── app/
│   │   ├── app.css
│   │   └── app.js
│   └── features/
│       ├── agents/
│       │   └── fleets.js
│       ├── board/
│       │   └── board.js
│       ├── colony/
│       │   └── colony.js
│       ├── inbox/
│       │   └── inbox.js
│       ├── messages/
│       │   └── messages.js
│       ├── plugins/
│       │   └── plugins.js
│       ├── presets/
│       │   └── presets.js
│       ├── researcher/
│       │   └── researcher.js
│       ├── studio/
│       │   ├── studio.css
│       │   └── studio.js
│       └── workspaces/
│           └── workspaces.js
├── README.md
└── index.html
```

维护方式：修改能力 JSON 或源码目录后运行 `python3 scripts/render-progress.py`；CI 使用 `--check` 检查文档同步与来源路径。
