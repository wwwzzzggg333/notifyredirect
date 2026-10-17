# NotifyRedirect Rust 原生版

这是全新版本，无需 .NET，不迁移旧配置、密钥或历史。包身份 NotifyRedirect.Native，可与旧版独立安装；使用新版前建议退出旧版，以免两者各自转发相同通知。

## 构建和验证

Windows 11 x64，Rust 1.96 或更高版本、MSVC C++ 构建工具、Windows SDK MakeAppx/SignTool。Cargo.lock 固定依赖；Release 静态链接 MSVC CRT，用户无需另装 VC++ 运行库。

```powershell
./scripts/Verify-Rust.ps1
./scripts/Build-RustMsix.ps1
./scripts/Sign-RustDevPackage.ps1
```

构建产物：`artifacts/rust/NotifyRedirect-Native-1.0.0-x64.msix`、对应开发证书与 `size-report.json`。沙箱内可用 `Verify-Rust.ps1 -SkipUserSession -Offline` 验证不依赖真实用户凭据/Explorer 的部分；这不代替完整桌面验收。

开发包为自签证书，不能直接作为公众安装包分享。确认信任开发证书后，由用户自行将对应 .cer 导入当前用户 TrustedPeople，再安装 MSIX；脚本不会信任证书或安装。签名不带可信时间戳。正式分发需要可信证书或 Store。

## 首次使用

1. 启动“通知转发器 · Rust 原生版”，点击“授权读取通知”。首次成功读取只建立基线，不转发已有通知。
2. 让目标应用产生新通知，在“通知来源”选中并保存；可配置包含/排除关键词。
3. 添加飞书、企业微信或已有钉钉旧 Webhook，填写密钥并保存，点击测试，在手机确认收到。
4. 在“规则与记录”选择全天、时间段或暂停，保存时间。默认暂停、空来源。星期 1–7 表示周一至周日，跨午夜归开始日。
5. 关闭窗口后继续在托盘运行；从托盘退出才停止。登录启动需用户主动勾选并获 Windows 允许。

数据位于当前用户 LocalAppData 下 `NotifyRedirectNative/notifyredirect.db`；MSIX 常见位置为 `%LOCALAPPDATA%/Packages/NotifyRedirect.Native_*/LocalCache/Local/NotifyRedirectNative/`。密钥使用当前用户 DPAPI；消息正文存于本地数据库，未整体加密。正文保留7天、状态30天；清空历史也取消待发。导出排除密钥和历史，导入后渠道及入口停用。

## 转发与故障

时段外新消息不积攒补发。队列每渠道每分段独立，接收后24小时过期，最多5次自动重试。超时或进程退出前未确认结果记为不确定，不自动重试；手动重发可能重复，请先看手机。平台成功表示接受请求，不保证手机已读。

电脑锁屏后仍须保持用户登录、唤醒和联网。睡眠、休眠、注销和关机期间不能实时转发。Windows 通知只能转发源应用实际提供的文本，不读取完整会话。权限与锁屏、平台手机收件仍需实机验收。

新版 [Agent 接入](../integrations/native/README.md) 使用端口17842、新 snake_case JSON 和小写状态，不兼容旧示例。默认入口关闭，开启后只接受本机 Bearer 鉴权。

[验收记录](validation/rust-release-checklist.md)区分自动验证和待实机项目。
