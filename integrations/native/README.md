# Rust 原生版 Agent 接入

先运行新版 APP，在“规则与记录”开启本机入口。CLI 从新版数据库读取 DPAPI token，无需在 Hook 配置里保存密钥。仅监听 127.0.0.1:17842。

安装目录中的 `tools/notifyredirect-cli.exe` 用法：

```powershell
$package = Get-AppxPackage -Name NotifyRedirect.Native
$cli = Join-Path $package.InstallLocation 'tools/notifyredirect-cli.exe'
& $cli send --source custom --title '任务完成' --body '测试通过' --state succeeded
```

首次事件只登记新来源，在“通知来源”勾选并保存后，再发送后续事件。成功退出码仅表示本机接收，仍受来源/时段/渠道约束。

新 JSON 格式（也可用 `send --stdin-json`）：

```json
{"version":1,"event_id":"unique-event-id","source":"custom","title":"一轮回复结束","body":"最终内容","state":"turn_ended"}
```

可选字段 `occurred_at`（RFC3339）、`project`、`session_id`。重试同一事件须复用 event_id；不同事件用不同 Id。state 可为 unknown、turn_ended、process_exited、succeeded、failed、waiting_input，不自动把响应结束当成任务成功。

接入示例：[Claude Code](claude-code.md)、[OpenCode](opencode.md)、[Codex CLI](codex-cli.md)、[Cursor](cursor.md)。请合并配置，不覆盖已有 Hooks；四类实际 Agent 会话尚待验收。桥接失败返回 0，不影响源 Agent。脚本不扫描会话文件。

未安装、手工运行新版时，可以给桥接脚本传 `-CliPath` 指向构建后的 CLI；如使用自定义数据目录，直接调用 CLI 并传 `--data-dir`。
