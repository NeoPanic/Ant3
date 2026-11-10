# 迁移到 Ant³ 统一标识

此次调整统一命令、包名、环境变量、协议与目录，属于接口名称变更。数据库结构没有因此改版；历史任务、会话、回执和不可变快照保留原值。

| 原标识 | 新标识 |
| --- | --- |
| `harness3` 命令、Rust crate | `ant3` |
| `harness3-*` Cargo 包与源码目录 | `ant3-*` |
| `Harness` Rust 执行器类型 | `Ant` |
| `.harness3/` 默认数据目录 | `.ant3/` |
| `HARNESS3_*` 环境变量 | `ANT3_*` |
| `harness3.plugins/v1` | `ant3.plugins/v1` |
| `harness3-plugin-rpc/v1` | `ant3-plugin-rpc/v1` |
| `harness3_token` 本地认证 Cookie | `ant3_token` |
| MCP 服务名称 `harness3` | `ant3` |
| 模型工具名编码前缀 `h3_` | `ant3_` |

## 数据与服务

1. 停止旧 daemon，并备份整个旧数据目录，包含 SQLite 的 WAL/SHM、token、插件缓存和工作区文件。
2. 默认路径用户将 `.harness3` 整体重命名为 `.ant3`。目标目录必须不存在；若两者都有数据，应分别保留并使用 `--data-dir` 选择，不能合并数据库或覆盖目录。自定义路径继续通过 `--data-dir` 指定。
3. 更新服务脚本、环境文件与 MCP 配置中的命令、包名和 `ANT3_*` 变量。命令不再读取旧环境变量。
4. 使用 `cargo run --locked -p ant3-cli -- up` 启动，重新登录控制台。token 文件内容保留，Cookie 名称已变化。

新程序不会自动重命名旧目录；未迁移时默认 `.ant3/` 会创建独立空工作区。显式 `--data-dir` 仍可指向旧目录读取数据。

## 插件、母体与会话

本地进程插件的 manifest 与 worker 必须一起升级协议字符串，重新计算 manifest 中修改文件的 SHA-256，并发布新插件版本。仓库附带示例已经同步更新。

历史插件快照与锁定版本保持不可变，旧协议插件需要升级，不能假定旧快照会自动转换。使用更新后的插件版本发布新 Preset 和实例，旧实例与运行记录继续用于历史检查。需要继续执行的工作先检查旧 Run 是否已结算；外部工具副作用不应自动重放。

模型工具别名前缀也已更新。旧供应商续接会话包含旧工具名，应使用新实例开始新会话，避免混用不同版本的工具契约。原有任务与结果可保留用于人工核对。

## Rust 调用

```toml
[dependencies]
ant3 = { git = "https://github.com/NeoPanic/Ant3.git", branch = "main" }
```

```rust
use ant3::{Ant, Limits, Registry};
let agent = Ant::new(Registry::default(), Limits::default());
```

本地 checkout 的父目录名称不影响 Cargo 或默认数据目录解析，无需为编译而移动仓库本身。
