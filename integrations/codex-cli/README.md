# Codex CLI

将下面配置合并到用户级 `~/.codex/config.toml` 的顶层，保留现有设置，替换脚本绝对路径。已经配置 notify 时需自行串联，不能再添加第二个同名键。

```toml
notify = ["powershell.exe", "-NoProfile", "-File", "E:/workspace/myapp/notifyredirect/integrations/Send-AgentEvent.ps1", "-Agent", "codex-cli", "-PayloadJson"]
```

notify 将 JSON 作为末尾参数传入；桥接仅处理 `agent-turn-complete`，转发 `last-assistant-message`，标为 TurnEnded。没有最终正文的事件只发送标题。该配置不是 ChatGPT 桌面的配置。

[官方配置参考](https://developers.openai.com/codex/config-reference/)说明 notify 位于用户级配置；项目级 notify 会被忽略。未进行真实 Codex CLI 版本验收，超长 JSON 的 Windows 参数长度限制仍可能影响源工具回调。APP 首次登记来源后需勾选保存。
