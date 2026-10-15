# NotifyRedirect · Windows 通知转发器

把所选 Windows 应用的通知转发到手机上的飞书、企业微信或已有钉钉群机器人。每个人独立安装、自己配置接收渠道，电脑直接发送，无需运营云服务。当前为 Windows 11 x64 开发验收版。

## 安装及首次使用

安装包位于 `artifacts/NotifyRedirect-0.1.0-x64.msix`，自包含 .NET 运行时；无需安装 SDK。开发包使用自签证书，不能直接当作公众免配置安装包发布。

确认信任开发证书后，在项目目录运行：

```powershell
./scripts/Install-DevPackage.ps1 -TrustDevelopmentCertificate
```

这会把 `artifacts/NotifyRedirect-Development.cer` 加入**当前用户 TrustedPeople** 并注册 APP。脚本不修改系统安全策略。未确认前不执行安装。正式分享应使用可信签名证书或 Microsoft Store 分发。

1. 启动通知转发器，点击“授权读取 Windows 通知”。Windows 会要求你允许通知读取。
2. 让 ChatGPT、Cursor 等发送一条通知，在“通知来源”勾选应用并保存。只有观察到通知后，来源才会出现在列表里。首次授权仅建立现有通知基线，不补发旧通知。
3. 创建接收渠道，填写群机器人 Webhook 和平台要求的签名密钥，保存后点击“发送测试消息”。在手机确认收到后才算该渠道验证成功。平台可能还要求安全关键词、群权限或管理员允许。
4. 保存转发时间。默认暂停；可选择全天、时间段或暂停。示例 `1,2,3,4,5 | 18:30 | 09:00` 表示周一至周五晚间到次日早晨；星期按开始日计算。`6,7 | all` 表示周末全天。时段使用电脑本地时区，包含开始、不包含结束，时段外新消息丢弃，不积攒到下班补发。
5. 关闭主窗口后继续在托盘运行；“退出程序”会停止监听。登录启动需主动勾选并由 Windows 允许。

## 锁屏与通知内容

设计使用保持登录的桌面后台进程，`Win+L` 锁屏时电脑仍须保持唤醒和联网。屏幕可以关闭；睡眠、休眠、注销、关机期间不能实时转发。锁屏通知与隐藏正文设置的实际行为仍需在安装后验证，不能把构建通过视为锁屏验证成功。

Windows 通知只能提供源应用放入通知里的标题和正文。Cursor 如果只显示“打开 Cursor 查看输出”，转发也只有这些文字；不会自动读取完整对话。普通通知不推断任务是否成功。需要完整最终回复时，可选下面的 Agent 接入口。

## Agent 接入

在“规则与记录”启用本机入口后，仅监听 `127.0.0.1:17841`，POST `/v1/events` 需要本用户 DPAPI 保存的 Bearer token。最多 128 KiB 请求体。CLI 自动读取凭据，令牌不用写进配置或命令行。关闭入口停止接收。

包内 CLI 位于安装目录 `tools/notifyredirect-cli.exe`：

```powershell
$package = Get-AppxPackage NotifyRedirect.Desktop
$cli = Join-Path $package.InstallLocation 'tools/notifyredirect-cli.exe'
& $cli send --source custom --title '开发流程完成' --body '测试通过，结果已保存' --state Succeeded
```

也支持 `send --stdin-json`。首次事件会登记来源，在来源页勾选并保存，再发送事件才能自动转发。返回“已接收”表示本机接受，仍受来源、时间和渠道规则限制，不表示手机已送达。

适配示例：[Claude Code](integrations/claude-code/README.md)、[OpenCode](integrations/opencode/README.md)、[Codex CLI](integrations/codex-cli/README.md)、[Cursor](integrations/cursor/README.md)。示例不会覆盖既有配置、不会扫描对话文件，源 Agent 实际版本仍需验收。Gemini CLI、Copilot CLI、Aider 等可以调用通用 CLI，尚无专用适配器。Codex CLI 是命令行编程工具；ChatGPT 桌面通知通过 Windows 监听接入，不使用 Codex CLI 配置。

## 可靠性与数据

队列按渠道和分段保存；可重试失败最多重试 5 次，消息接收后 24 小时过期，每次自动发送前再次检查暂停状态和时段。超时、进程退出时的发送结果标记“不确定”，不自动重试；手动重发可能重复。平台接口“成功”表示接受请求，仍应在手机确认。

用户本地 `NotifyRedirect/notifyredirect.db` 保存配置、正文和历史。MSIX 环境通常位于 `%LOCALAPPDATA%/Packages/NotifyRedirect.Desktop_*/LocalCache/Local/NotifyRedirect/`。凭据使用 Windows 当前用户 DPAPI；**数据库正文未整体加密**。正文保留 7 天，状态保留 30 天。导出排除凭据及历史，导入停用渠道和本机入口。清空历史会取消待发队列。程序不记录平台完整响应或明文密钥，后台错误仅显示类型或脱敏原因。

## 构建与验收

```powershell
./scripts/Verify.ps1
./scripts/Build-Msix.ps1 -SelfContained
./scripts/Sign-DevPackage.ps1
```

需要 .NET 10 SDK 和 Windows SDK MakeAppx/SignTool。脚本优先使用项目 `.tools/dotnet`，不替换全局 SDK。依赖固定在 `packages.lock.json`。开发签名只生成文件，不信任证书、不安装程序；未启用时间戳服务。升级沿用包身份，数据保留仍需安装验收。签名私钥不提交。

[验收记录](docs/validation/release-checklist.md)明确区分自动测试、打包验证与待实机验收项目。飞书及企业微信为待手机实测的 Webhook 实现；钉钉仅兼容已有旧 Webhook，新机器人方案尚未接入。Windows 10 和 ARM64 未声明支持。
