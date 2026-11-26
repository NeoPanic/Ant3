# ACP 外部执行器

Ant³ daemon 可以为蚂蚁装配 `external.acp` 系统工具。工具通过 Agent Client Protocol（ACP）v1 将工作委派给本机 stdio 执行器，并将回复和停止原因交还给发起委派的蚂蚁。普通任务板 Run 认领后即可调用，已加入根任务的同伴也可在处理交接消息时调用。Preset 无需额外绑定。

## 注册执行器

安装支持 ACP v1 stdio 的适配器。例如，可以用 `npm install -g @agentclientprotocol/codex-acp@2.12.0` 安装 Codex 适配器。保存以下 JSON，并按运行环境调整命令路径。

```json
[
  {
    "id": "codex",
    "command": "codex-acp",
    "args": [],
    "inherit_env": ["CODEX_HOME"]
  }
]
```

给原有 daemon 启动命令增加 `--acp-executors /path/to/executors.json`，保留 Ant³ 自身的模型参数。daemon 启动时加载清单。增加执行器时追加一条记录，再重启 daemon。每条记录使用唯一的 `id`，`command` 可以是绝对路径或 daemon 的 `PATH` 上的程序，`args` 是传给程序的参数数组。

执行器按 daemon 的系统用户运行。它会获得常用的运行环境变量和 `inherit_env` 明确列出的变量，可以使用对应的本机配置和认证。需要 ACP 协议认证时，`auth_method` 填适配器声明的方法 ID。认证值通过执行器自己的配置或环境读取。有权限请求时 Ant³ 选择拒绝，执行器可以在其自身权限策略内运行。

## 会话与任务记录

蚂蚁首次调用传入执行器名和任务提示。

```json
{"executor":"codex","prompt":"检查当前工作区中的 README，列出缺少的启动步骤。"}
```

继续交互时使用返回的 `session_id`，并传入同一个执行器名。

```json
{"executor":"codex","session_id":"<返回的会话 ID>","prompt":"按现有命令核对第一项，给出可验证的证据。"}
```

平台把工作目录设为当前实例的工作区。会话句柄只在所属 Run 内有效，允许在同一 Run 内启动最多八个会话，Run 结束时回收进程。执行期间的计划、工具活动和回复进度类型记为 `execution.acp_progress`，可从 `ant3 events` 查询。回执包含回复正文、停止原因和截断标记。执行器产生的文件可用现有工作区工具核查和发布。

取消 Run 时，平台向进行中的轮次发送 `session/cancel`。如果执行器在提交提示后断连、协议出错或未能确认取消，平台保留 `acp_outcome_unknown`，供检查可能已经发生的工作区改动。
