# Ant³ 本地平台使用指南

Ant³ 在本地管理 Preset、实例、任务和工具执行。默认流程从共享任务板发布工作开始：控制面动态选择 Preset 并启动实例，Agent 自行认领、执行或拆分。已有 Agent 池与单实例 Run 继续可用。入口共用同一个 daemon 与持久状态；运行时 crate `ant3` 也可独立使用。

## 启动与登录

要求 Rust 1.99+，建议使用最新稳定版。在仓库根目录执行，无需 Node、外部数据库或前端开发服务器：

```sh
cargo run --locked -p ant3-cli -- up
```

打开 [http://127.0.0.1:7333](http://127.0.0.1:7333)，将 `.ant3/token` 的内容粘贴到登录框。平台 token 用于本地认证，与模型 API Key 分开。Web 使用 HttpOnly / SameSite=Strict 会话 cookie；API 支持 Bearer token。

SQLite、token、插件缓存、实例文件和 daemon 独占锁保存在数据目录，默认 `.ant3/`。使用 `--data-dir` 和 `--port` 可调整目录和端口；同一数据目录只能运行一个 daemon。默认监听 loopback。首次启动不创建 Preset、实例、任务或演示数据；未配置模型也可查看空白控制台，但不能执行模型任务。

Ctrl-C 停止派发新工作，并等待已开始运行协作取消、收尾。重启保留已提交状态和已知回执，排队工作可以继续处理；中断且结果未知的运行不会自动重放或续跑。备份时应包含数据库和工作区文件，不能只复制 SQLite。

## 配置模型

平台支持 OpenAI-compatible、OpenAI Responses 和 Anthropic。以 Responses 为例，通过环境变量设置服务根路径和模型，并用自己的凭证加载方式设置 `ANT3_OPENAI_RESPONSES_API_KEY`：

```sh
export ANT3_OPENAI_RESPONSES_BASE_URL=https://api.openai.com/v1
export ANT3_OPENAI_RESPONSES_MODEL=gpt-6-luna
cargo run --locked -p ant3-cli -- up
```

| 协议 | 环境变量前缀 | 追加 Endpoint | 默认 BASE_URL |
| --- | --- | --- | --- |
| OpenAI-compatible | `ANT3_OPENAI_COMPATIBLE_` | `/chat/completions` | `https://api.openai.com/v1` |
| OpenAI Responses | `ANT3_OPENAI_RESPONSES_` | `/responses` | `https://api.openai.com/v1` |
| Anthropic | `ANT3_ANTHROPIC_` | `/messages` | `https://api.anthropic.com/v1` |

每个前缀读取 `MODEL`、`BASE_URL`、`API_KEY`。`MODEL` 必需；不要求认证的本地服务可以省略 API Key。`BASE_URL` 包含 `/v1` 等服务根路径。密钥不应放入 Preset、插件指令或 Git。

也可引用已有的 Responses 配置目录：

```sh
cargo run --locked -p ant3-cli -- up \
  --codex-config-dir /path/to/codex-config \
  --model gpt-6-luna
```

该方式读取 `config.toml` 选定 provider 的 `base_url`、`wire_api` 和 `env_key`；没有 `env_key` 时从同目录 `auth.json` 读取 `OPENAI_API_KEY`。仅接受 `wire_api = "responses"`，不修改原配置或复制密钥到数据库，不使用 OAuth token 替代 API Key。

配置在 daemon 启动时固定，切换需重启。模型或端点改变后，有历史会话的旧实例可能拒绝继续运行；创建新实例可隔离供应商 continuation。UI 的“已配置”表示配置已读取，实际调用结果需从 Run 检查。

适配器支持非流式文本、函数工具调用、回传、用量和错误状态。Responses 保留必要 output items 与 encrypted reasoning continuation。流式、多模态、供应商内置搜索和通用自动重试不在当前适配范围。

## 先发布一个任务

1. 先配置模型，在“Presets”发布自己的 Preset；再打开“任务板”，填写标题、工作要求和验收标准并发布，无需手动创建实例。
2. 查看控制面的选型记录、启动实例和 Agent 认领。进入“Agent 工作现场”观察实际执行与任务关系。
3. Agent 拆分工作后，子任务进入同一任务板；控制面为它们继续选型和启动实例。子任务完成后，普通实例可重新认领原任务进行整合。

模型与 Preset 均需开发者显式配置，服务不自动生成研究团队或通用 Preset。没有可用模型时不会回退到模拟响应。选型可以独立使用低延迟模型，详见[共享任务板](task-board.zh-CN.md)。

## 对象与侧栏分层

侧栏按“场景 → 协作 → 运行 → 配置 → 开发”组织。场景用于完成用户目标；协作区检查分工与共享证据；运行区管理执行身份和生命周期；配置区定义 Preset 及能力。

| 对象 | 用途 |
| --- | --- |
| Preset（Preset） | 不可变版本，定义用途、目标、模型协议、指令、参数和插件绑定 |
| Agent 池（API/CLI 名称 Fleet） | 管理同一 Preset 配置下的实例数量、创建和回收，不代表协作分工 |
| Agent 实例 | 独立身份、配置快照和持久会话，同一时刻最多一个已接纳 Run |
| 任务板任务 | 发布时无实例绑定，控制面选型后由 Agent 认领，记录拆分来源与结果 |
| Run | 一次输入及其模型/工具循环，保存请求、回执、事件、输出和用量 |
| 工作区快照 | 不可变文本与文件/JSON 交付，以及实例自己的可变文件目录 |
| 消息 | 同板根成员之间的持久点对点交接，投递与消费可恢复 |
| Operation | Agent 池创建、扩缩容等异步操作的接纳与完成状态 |

普通 Run 只执行单个 Agent。任务板按任务按需启动实例；成员之间通过板根招募与显式消息交接协作。批量创建 Agent 池本身不会产生任务。

## Agent 池与单实例运行

在“Presets”发布固定版本，再从“Agent 池”选择 Preset、数量和参数：

- `finite` 创建有限批次，成员终止后不自动补足。
- `maintain` 持续维持启用槽位，成员终止后以新身份、独立会话和递增 generation 补建；暂停成员仍占槽位。
- 扩缩容要求匹配 `expected_revision`。缩容排空大编号槽位，已接纳的运行结算后回收；缩到零仍可扩容。终止整个池后不可恢复。
- TTL 从每个实例实际创建时开始，可为 1–2,592,000 秒或省略。到期取消排队 Run，并请求活动 Run 协作取消；已知成功结果仍保留。

创建返回 HTTP 202 与 `fleet_id`、`operation_id`，表示意图和容量已持久化；实例分批创建。Operation 完成表示实例创建完成，业务结果需查看 Run 或任务板任务。

实例详情可暂停、恢复、终止或“排空并终止”。排空拒绝新任务、等待已接纳工作结算，不能撤回；终止后保留历史会话和回执。

```sh
./target/debug/ant3 fleets create --file crates/ant3-cli/examples/fleet.json --key pool-create-1
./target/debug/ant3 fleets show FLEET_ID
./target/debug/ant3 operations show OPERATION_ID
./target/debug/ant3 fleets scale FLEET_ID --count 3 --expected-revision 1 --key pool-scale-1
./target/debug/ant3 runs submit AGENT_ID --file input.txt --key run-submit-1
./target/debug/ant3 runs show RUN_ID
```

## 拆分与评审

任务运行中的 Agent 可以把工作拆分为子任务；子任务进入同一任务板，由控制面继续选型。拆分来源成功后激活子任务，子任务全部完成后原任务重新可认领以整合结果。失败、取消或结果未知的子任务会阻断父任务整合，兄弟任务继续结算。

根任务执行成功后进入待评审。发布时启用独立验证的任务先由全新验证器实例按验收标准审计冻结结果：通过自动归档，退回附意见有界返工，证据不足或预算耗尽交回人类评审。Run 成功只证明执行成功；人类核对结果与验收标准后提交评审，`review_state=accepted` 才归档。

取消停止后续认领、取消排队工作，并等待活动 Run 收尾，不撤销已发生的工具效果。退回修改按评审意见重新启动选型与执行；历史结果与评审记录保留。

## HTTP、CLI 与 MCP

需要认证的 HTTP 请求使用 `Authorization: Bearer <平台 token>`。新任务板入口为 `/api/board`，已有平台资源路径前缀为 `/api/v1`。创建与其他幂等写操作使用 `Idempotency-Key`：同作用域、同键、同内容返回原接纳回执，改内容返回 409。接纳后通过对象 ID 查询状态；超时重试保持原键与原内容。

CLI 从 `--data-dir` 读取平台 token，只调用 loopback HTTP，不直接写数据库。各命令支持 `--port` 和 `--data-dir`，输出 JSON：

```sh
./target/debug/ant3 snapshot
./target/debug/ant3 board publish --file crates/ant3-cli/examples/board-task.json --key study-1
./target/debug/ant3 board list
./target/debug/ant3 board show TASK_ID
./target/debug/ant3 presets list
./target/debug/ant3 events --after 0
```

MCP stdio 连接已启动的 daemon：

```sh
./target/debug/ant3 mcp serve --transport stdio --data-dir /path/to/.ant3
# 需要写入时追加 --allow-write
```

客户端配置见 [mcp.json](../crates/ant3-cli/examples/mcp.json)。适配器仅读取平台 token；stdout 用于逐行 JSON-RPC，诊断使用 stderr。默认只公布查询工具，`--allow-write` 授予本地操作者写权限。支持协议版本 2024-11-05、2025-03-26、2025-06-18、2025-11-25；没有 resources、Streamable HTTP、并发调用或协议级请求取消，取消 Run/任务使用显式工具。

HTTP JSON 请求和 CLI 文件上限为 128 KiB，用户输入上限为 64,000 UTF-8 字节。列表有返回窗口；完整 Run、任务、Operation 通过 ID 查询，事件按游标补读。平台不自动重试写入。

## 专题指南

| 需要完成的工作 | 文档 |
| --- | --- |
| 发布任务，动态选型、认领、拆分与回投 | [共享任务板](task-board.zh-CN.md) |
| 定义 Preset、继承、多个插件和实例参数 | [Preset 与参数](presets.zh-CN.md) |
| 安装自定义本地工具 | [进程插件](process-plugins.zh-CN.md) |
| 读取信息素痕迹、浓度与蒸发 | [信息素痕迹场](traces.zh-CN.md) |
| 点对点交接、消费及未知结果处置 | [协作消息](messaging.zh-CN.md) |
| 写文件、发布文件/JSON 快照并交接 | [文件工作区](workspaces.zh-CN.md) |
| 理解持久执行与 Temporal 的边界 | [Durable Workflow](durable-workflows.zh-CN.md) |

设计目标见[架构提案](managed-agents.zh-CN.md)，范围见[当前建设范围](status.zh-CN.md)，代码目录和能力状态见[实现清单](capabilities.zh-CN.md)。

## 本地运行边界

全局最多 256 个非终结实例及预留容量，8 个本地 Worker；每个板根任务树最多 64 项任务，活动根任务同样受该上限约束。递归拆分最多 6 层、一次 8 个子任务；这些是规模限制，当前不设团队费用预算账本。

平台采用单 daemon、SQLite 与本地进程，未提供远程 Worker、多租户、任意任务图重写或操作系统沙箱。本地 token 具有整个平台访问权。进程插件以 daemon 所在 OS 用户权限执行，需显式启用 `--allow-process-plugins`；插件版本和能力绑定固定在 Preset，安装新版本不会改写已有实例。

持久接纳和幂等回执避免重复派发，不保证外部工具效果恰好一次。工具断连、超时或进程中断可能产生未知结果；平台保留证据并停止依赖传播，操作者需先核对实际效果。独立验证核查已有证据，不能自动证明外部事实。

## 开发检查

```sh
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked --example quickstart
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
```

集成测试使用本地服务与确定性模型，不要求供应商账号。真实模型连通与兼容性应使用自己的配置单独验证。
