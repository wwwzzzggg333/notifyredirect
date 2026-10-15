# Cursor

将片段合并到现有 `.cursor/hooks.json`，替换绝对路径；不要覆盖其他 hooks。

```json
{"version":1,"hooks":{"afterAgentResponse":[{"command":"powershell.exe -NoProfile -File E:/workspace/myapp/notifyredirect/integrations/Send-AgentEvent.ps1 -Agent cursor"}]}}
```

[官方 hooks 文档](https://cursor.com/docs/hooks)说明 afterAgentResponse 输入的 `text` 是最终回复。每次回复可能产生一条消息，标为 TurnEnded；不监听思考内容、不从 stop 事件假定有完整正文。首次来源登记后请勾选保存。

这是本机 Cursor Agent 示例，未进行真实版本验收。普通 Windows 通知仍可以独立转发；同时启用两种入口可能收到两条不同来源的消息。
