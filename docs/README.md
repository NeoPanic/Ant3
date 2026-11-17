# Ant³ 文档

先运行平台，再根据操作需求查阅专题。设计文档描述目标与约束；当前状态和能力索引描述实际落地范围。

## 启动与使用

- [入门指南](getting-started.zh-CN.md)：本地启动、模型配置与任务发布。
- 控制台以"人类观察蚁群"视角组织：观察（工作现场/蚁群/成果库）、工作（任务板/需要你处理）、设置（Preset/插件/研究者视图）；机制参数集中在研究者视图。
- [共享任务板](task-board.zh-CN.md)：任务先发布，控制面动态选型，Agent 认领、拆分与回投。
- [客户端示例](../crates/ant3-cli/examples/)：CLI 计划文件与 MCP 配置。

## 当前状态

- [平台状态](status.zh-CN.md)：可用链路、容量限制与验证边界。
- [路线图](roadmap.zh-CN.md)：唯一前瞻清单，只列未完成方向。
- [能力与源码索引](capabilities.zh-CN.md)：逐项实现状态、当前模块与目标目录映射、完整源码树。修改能力 JSON 后运行 `python3 scripts/render-progress.py` 更新。

## 设计与内核

- [Managed Agents 架构](managed-agents.zh-CN.md)：设计目标、理论基础、对象与状态约束、插件范式和完整目标目录树。
- [Workspace 边界](workspace.zh-CN.md)：当前 crate 职责、依赖检查与源码导入迁移。
- [持久工作流职责](durable-workflows.zh-CN.md)：运行恢复与 Agent 协作的职责边界。
- [运行契约](runtime.md)：检查点、恢复、授权、压缩、投递与追踪。
- [Rust 内核导读](runtime-guide.zh-CN.md)：从输入到执行结果的源码路径与设计推理。
- [控制台设计规范](../DESIGN.md)与[品牌规范](brand/README.md)。

## 协作与扩展参考

| 主题 | 内容 |
| --- | --- |
| [Preset 组合](presets.zh-CN.md) | 固定版本继承、参数和插件绑定 |
| [共享任务板](task-board.zh-CN.md) | 无实例绑定发布、Preset 选型、原子认领与拆分 |
| [信息素痕迹场](traces.zh-CN.md) | 浓度、增强、蒸发与有界 Agent 信号 |
| [涌现评估基线](evaluation.zh-CN.md) | 单/多 Agent 同任务对照与信息复用指标 |
| [预算即能量](energy.zh-CN.md) | 预留、usage 结算、验收回充与饥饿淘汰 |
| [个体自选认领](selfselect.zh-CN.md) | 选型下沉：游走池、阈值响应、原子竞争 |
| [点对点消息](messaging.zh-CN.md) | 投递、派发、消费和恢复 |
| [文件工作区](workspaces.zh-CN.md) | 私有文件与同一个根任务快照共享 |
| [进程插件](process-plugins.zh-CN.md) | 包格式、安装、绑定与执行契约 |

插件示例：[文本统计](../crates/ant3-plugins/examples/text-inspector/)、[HTTP 读取](../crates/ant3-plugins/examples/http-reader/)、[文件交接 Preset](../crates/ant3-plugins/examples/workspace-preset/)。

官方内置插件（文本统计、网页读取、网页搜索）统一放在根目录 [`plugins/`](../plugins/README.md)，与协议、SDK、宿主实现及教学示例分开维护。

开发流程见[参与贡献](../CONTRIBUTING.md)。公共 API 文档通过 `cargo doc --locked --workspace --no-deps --open` 生成。

