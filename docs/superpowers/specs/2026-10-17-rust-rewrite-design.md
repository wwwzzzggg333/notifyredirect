# NotifyRedirect Rust 全新版本设计

日期：2026-10-17

状态：用户已确认 Rust 原生界面方向，并修正为全新版本；无需兼容旧版。本文据此更新，产品代码尚未开始实现。

## 目标与范围

交付 Windows 11 x64 的轻量通知转发工具，采用 Rust、中文原生设置窗口和系统托盘，用户无需安装开发工具或 .NET。保留通知读取、来源和关键词过滤、飞书/企业微信/已有钉钉 Webhook、时间规则、持久发送队列、历史记录和 Agent 接入等产品功能。

这是独立的新版本：不迁移旧数据库、配置、密钥或历史，不维持旧 JSON、枚举数值、时间格式、去重标识、CLI 参数或 API 协议。用户重新授权通知、选择来源、配置渠道；新配置导入导出只服务本版本。原 C# 源码留在仓库供参考，不成为运行或验证依赖。

当前旧 MSIX 为 132,549,351 字节（约 126.4 MiB），仅作体积基线。新版本 Release 主程序、CLI、资源合计目标 <=20 MiB，MSIX 目标 <=15 MiB；均待实测。不能用删减功能达标，未达时报告依赖分析和实际值。不引入 Electron、WebView、Chromium、OpenSSL 或 .NET 运行时。

## 工程与依赖

`native/` 为 Cargo workspace，包含 `notifyredirect-core`（模型、规则、事件、文本）、`notifyredirect-storage`（SQLite、队列）、`notifyredirect-platform`（Windows 通知、DPAPI、WinHTTP、启动项）、`notifyredirect-app`（协调器、API、原生 UI/托盘）、`notifyredirect-cli`（本机提交工具）。

使用 windows crate 调用系统接口；rusqlite 内置 SQLite；serde/serde_json 处理 JSON，chrono 处理时间。依赖精确固定并提交 Cargo.lock。Release 开启大小优化、LTO、单 codegen unit、去除调试符号。业务核心无 Windows 依赖，存储和系统层不互相依赖。

## 独立安装与数据

MSIX Identity 为 `NotifyRedirect.Native`，Application Id 为 `App`，初始包版本 `1.0.0.0`，开发 Publisher 使用 `CN=NotifyRedirect Development`。主程序为 `NotifyRedirect.exe`，CLI 为 `tools/notifyredirect-cli.exe`；这些是新版本自身的文件名，不承诺旧集成可继续调用。

命名互斥为 `Local\NotifyRedirect.Native`，启动项 Id 为 `NotifyRedirectNativeStartup`。新旧版本可独立安装；新程序不发现、读取、覆盖或接管旧版数据。若两者均运行并开启同样来源，可能各自转发，使用说明提示用户退出旧版以免重复收件。

通过当前进程 LocalAppData 路径下 `NotifyRedirectNative/notifyredirect.db` 保存数据；MSIX 下允许系统正常重定向。CLI 只从新版固定目录、当前新版包身份对应的目录或显式 `--data-dir` 定位，不扫描旧包。

新数据库使用独立 schema v1，应用只维护本版本的版本号。配置 JSON 为 snake_case，枚举为字符串，时间为 RFC3339 UTC；无需拟合旧版格式。较高 schema 不可破坏性打开，错误/损坏文件不自动删除重建。凭据以当前用户 DPAPI 加密，不能导出到普通配置中；不实现旧密文读取和接管备份。

## 界面和执行模型

四个中文页面：首页（权限和状态、授权/刷新/暂停/退出、预览、启动项）；通知来源（列表、勾选和关键词）；接收渠道（增删改、启停、密码输入、测试）；规则与记录（时间、电脑名称、导入导出、Agent 开关、历史详情、重发和清空）。

关闭窗口隐藏到托盘，托盘支持打开、暂停和退出，Explorer 重启后重建图标。原生控件支持滚动、Tab 导航和每监视器 DPI。初始 1060×760 DIP，最小 850×620 DIP。

UI 线程管理窗口和发起 WinRT 授权；通知读取在初始化 WinRT 的工作线程执行。后台协调线程独占 SQLite，独立网络线程发送请求。UI 不等待数据库或网络；每次状态变化保存成功后才发布快照。退出停止入口和新发送，最多等待 5 秒；未确认在途请求重启后记为不确定。

## Windows 通知

使用 UserNotificationListener 和 MSIX 的 userNotificationListener、internetClient、runFullTrust 能力；用户明确请求授权，拒绝、撤销、未授权和不可用分别显示。

首次成功读取仅建立当前通知基线，不补发旧通知；随后监听事件并每 5 秒补偿读取。来源用 AUMID，通知键由 AUMID、通知 Id、创建时间组成，格式按新版定义。读取后再次检查权限，撤销时丢弃本批。无文本跳过，单条异常不影响其他项。普通通知不推断任务成功。

锁屏仍需保持用户登录、电脑唤醒和联网。授权、ChatGPT/Cursor 原始内容及 Win+L 后的转发需要安装后的实机验收。

## 规则和持久队列

- 默认暂停且不选中任何来源；关键词按 Unicode 小写后子串匹配，空白词忽略，排除优先；包含列表无有效词时不限。
- 本地时间、开始包含/结束不包含，跨午夜星期按开始日，多规则取并集；全天独立选项，普通规则起止相等无效。星期使用 1=周一至 7=周日。
- 事件超过过去 24 小时或未来 5 分钟拒绝；事件时间与接收当前时间均须命中规则。时段外新消息不排队、不补发；每次实际发送和重试前再检查暂停、时段、来源和渠道。
- 观察去重与入队同一事务；同键同内容不重复发。仅在所有分段均未尝试发送时替换内容并保留原过期时间；已尝试后的更新形成新消息。渠道及分段各自保存状态。
- 接收后恰好 24 小时过期；最多 5 次重试，延迟 30 秒、2 分钟、10 分钟、30 分钟、2 小时，并取更长的 Retry-After。
- 超时、取消、传输中断和进程退出前未确认的发送记为不确定，不自动重试；能够证明未发送的连接失败可重试。手动重发失败/不确定项须提示可能重复，仍服从规则。
- 正文保留 7 天，状态保留 30 天；清空历史取消待发队列，在途完成回调不能重新创建已删除记录。导出不含密钥和历史，导入停用渠道和入口并重新生成渠道 Id。

## 渠道

保留飞书、企业微信和已有钉钉旧 Webhook；不新增钉钉机器人方案。按平台官方协议实现签名和消息体，不要求与旧版内部实现一致。仅允许相应官方 HTTPS host/path，禁止重定向，TLS 使用 Windows 系统能力。

文本包含来源、标题、正文、状态、电脑名、时间和可选项目。总 UTF-8 大小最多 65536 字节，截断时加说明；企业微信完整分段含序号 <=1800 字节，其他 <=15000 字节，不切断 Unicode 字符。分段格式为 `[i/n]\n`，单段不加序号。

渠道请求总 deadline 15 秒，响应最多 64 KiB。HTTP 429/5xx 可重试，其他明确拒绝永久失败；平台结果码需确认成功，错误 JSON、缺失结果码及无法确认的响应记为不确定。错误文案脱敏，不记录完整响应、Webhook 或密钥。真实手机收件另行实测。

## 全新 Agent 接口与 CLI

API 默认关闭，启用时只监听 `127.0.0.1:17842`（避开旧版默认端口），POST `/v1/events`，application/json，Bearer token 存于新版 DPAPI 凭据。本版本协议标识为 `version:1`，与旧版的相同版本数字不表示互通。

事件字段：version、event_id、source、title、body、state、occurred_at、project、session_id。必需 version=1、非空 event_id/source/title；state 缺省 unknown，允许 unknown/turn_ended/process_exited/succeeded/failed/waiting_input；occurred_at 缺省当前 UTC。按 Unicode 字符计数，source/event_id/title/body 上限 80/200/1000/65536；请求体额外限制 128 KiB。

持久化接受后返回 202（不代表已送达），参数错误 400、鉴权失败 401、超限 413、类型错误 415、存储/协调器不可用 503。来源先登记，用户勾选后后续事件才转发；重试同 event_id 不重复入队。常量时间 token 比较，不开放 LAN 或 CORS。

CLI 提供 `send --source NAME --title TITLE --body BODY [--state unknown]`、`send --stdin-json`、`--help` 及可选 `--data-dir`，只读新版凭据并提交本机 API，超时 3 秒且不跟随跳转；成功 0、失败 1。为四类 Agent 编写新版独立接入示例，不修改或保证旧示例工作。

## 验证和交付

测试以本设计的新合同为依据，不生成 C# fixture、不执行跨语言互通或旧版回归。覆盖规则边界、Unicode、去重事务、进程恢复、队列、DPAPI 往返、模拟渠道、HTTP deadline/长度、API/CLI 和原生窗口冒烟；只使用合成数据、临时目录和模拟网络。

执行格式检查、Clippy、全部 Rust 测试、Release 构建、MSIX 内容与签名验证。记录 EXE、CLI、stage、MSIX 的字节数及相对旧包的缩减。不自动安装、信任开发证书或发送真实机器人消息。

交付 Rust 源码、锁定依赖、独立构建/签名/验证脚本、新包、中文说明和体积报告。安装、授权/撤销、锁屏、启动项、缩放/键盘、真实手机收件和进程中断恢复明确区分已验收与待实机验收。原 C# 工程保持原状，不要求其作为 Rust 发布检查的一部分。
