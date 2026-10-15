# Claude Code

依据 [官方 hooks 文档](https://code.claude.com/docs/en/hooks) 的 Stop 输入 `last_assistant_message`，只转发最后回复，不读取 transcript。Stop 代表一轮响应结束，状态设为 TurnEnded，不当作开发任务成功。

将下列片段合并到你现有的 `.claude/settings.json`，将路径替换为本仓库的绝对路径；不要覆盖已有 hooks。桥接脚本使用 Windows PowerShell，失败始终返回 0，不阻止 Claude。

```json
{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"powershell.exe -NoProfile -File E:/workspace/myapp/notifyredirect/integrations/Send-AgentEvent.ps1 -Agent claude-code","timeout":10}]}]}}
```

先在 APP 开启本机入口。第一次事件仅登记 `agent:claude-code`，勾选保存后再测试。未进行真实 Claude Code 会话验收。
