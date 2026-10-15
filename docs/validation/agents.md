# Agent 接入验收范围

2026-10-14 核对官方接口描述：Claude Code Stop、Cursor afterAgentResponse、OpenCode plugin events、Codex CLI 用户级 notify。示例只增加用户选择的片段，不修改现有配置、不读取完整对话目录。各源工具版本和真实事件尚未实机验收。

| 来源 | 数据与状态 | 验证状态 |
| --- | --- | --- |
| ChatGPT/Cursor Windows 通知 | 源通知标题和正文；Unknown | 监听代码已编译，安装授权/锁屏待测 |
| Claude Code | Stop 最后回复，TurnEnded | 官方字段核对；实际 hook 待测 |
| Cursor Agent | afterAgentResponse text，TurnEnded | 官方字段核对；实际 hook 待测 |
| Codex CLI | agent-turn-complete 最后回复，TurnEnded | notify 配置核对；实际版本与长参数待测 |
| OpenCode | idle/error/permission 事件元信息 | 不含完整回复，实际插件待测 |
| Gemini CLI/Copilot CLI/Aider/其他 | 通用 CLI 或 HTTP JSON v1 | 无专用适配器 |

首次 Agent 事件先登记来源，随后在 APP 勾选保存。hook 发送失败不改变源 Agent 的控制结果。完整回复可能包含项目内容，启用前应自行决定合适的接收群。
