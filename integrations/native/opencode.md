# OpenCode · Rust 原生版

复制本目录 `notifyredirect.js` 到项目 `.opencode/plugins/`。启动 OpenCode 前设置环境变量：

```powershell
$env:NOTIFYREDIRECT_NATIVE_BRIDGE='E:/workspace/myapp/notifyredirect/integrations/native/Send-AgentEvent.ps1'
opencode
```

按照 [官方插件事件](https://opencode.ai/docs/plugins/)订阅 `session.idle`、`session.error`、`permission.asked`。分别标为 turn_ended、failed、waiting_input。此示例转发事件名称和项目，不读取完整回复或会话目录；idle 不等于任务成功。手机端不提供权限审批回复。

子进程隐藏窗口、失败不会抛回 Agent；APP 开启入口后首次来源登记仍须勾选。未进行真实 OpenCode 版本验收。桌面版自身产生的 Windows 通知也可直接使用通用监听。


本示例只连接新版 `NotifyRedirect.Native`，端口 17842。安装后须重新配置并启用入口，不使用旧版凭据。示例已按新版字段调整，实际 Agent 版本仍需会话验收。
