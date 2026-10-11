# Windows 通知转发工具 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付独立安装的 Windows APP，在指定时间段把所选应用的通知发送到用户配置的手机接收渠道，支持锁屏运行及可选 Agent 接入。

**Architecture:** 使用带 MSIX 包身份的 WPF 桌面常驻进程读取 Windows 通知。来源、规则、持久化队列和渠道通过独立接口连接；SQLite 保存配置和队列，Windows 用户范围 DPAPI 保护凭据。首个交付物验证通知监听授权及锁屏能力，首条外部链路使用飞书，随后完善可靠发送、界面和其他渠道。

**Tech Stack:** C#、.NET 10 LTS、WPF、Windows Runtime 通知 API、MSIX、Microsoft.Data.Sqlite、System.Security.Cryptography.ProtectedData、System.Text.Json、HttpClient、xUnit；本地 HTTP 入口使用 ASP.NET Core Kestrel。Windows 11 x64 为首个验证平台，目标 TFM `net10.0-windows10.0.22000.0`，Core 使用 `net10.0`。不在未经测试时声明 Windows 10 或 ARM64 兼容。

**Spec:** `docs/superpowers/specs/2026-10-11-notification-forwarder-design.md`

## Global Constraints

- 本地 Windows APP，电脑直接向平台发送消息，不建设运营云服务。
- 手机仅接收消息，不提供回复、远程控制或权限审批。
- 默认不转发全部应用，用户明确勾选来源。
- 普通通知不自行推断任务成功，不调用模型生成摘要。
- 时间段使用电脑本地时区；开始时间包含、结束时间不包含；跨午夜规则的星期指开始日。
- 时间段外消息不加入发送队列，不积攒到下班后补发；每次自动发送或重试前再次检查时段和暂停状态。
- 队列消息超过 24 小时后标记过期；最多 5 次重试，不把首次尝试计入重试次数。
- 通知正文保留 7 天，发送状态保留 30 天；配置导出排除凭据和消息历史。
- 锁屏、用户仍登录、电脑唤醒且联网时继续工作为必需验收项；不自动修改系统电源设置。
- 不读取或删除其他应用的通知以外的数据，不调用通知清除 API。
- 平台只有在官方接口、账户条件及实测发送确认后才标记可用；不把模拟测试当作兼容性实测。

## Review Focus

1. 通知标识被复用：不同应用或不同创建时间不得被误当重复通知；测试归 Task 2。
2. 时间变更和夏令时：本地墙钟窗口与 UTC 有效期分别判断；测试归 Task 3。
3. 网络超时发生在服务端接收之后：标记结果不确定，不能宣称恰好一次；测试归 Task 5。
4. 安装升级后密钥不可解密或数据库版本过新：显示恢复要求，不清空数据假装成功；测试归 Task 4、7。
5. 中文、表情和平台按字节限制的长文本：分段不破坏字符，段落失败可单独重试；测试归 Task 6。

## 文件布局与构建约定

所有路径相对于仓库根目录。初始目录目前只有设计文档，没有 Git 仓库，也没有产品代码。

```text
NotifyRedirect.sln
global.json                         SDK 主版本锁定为 10，允许同主版本功能带向前滚动
Directory.Build.props               Nullable、ImplicitUsings、警告设置
src/NotifyRedirect.Core/
  Models/                           消息、来源、规则、渠道和发送状态类型
  Scheduling/                       本地时间段判断、过滤
  Delivery/                         渠道契约、队列消费和重试
src/NotifyRedirect.Infrastructure/
  Storage/                          SQLite、迁移、去重与保留策略
  Security/                         DPAPI 凭据保护、日志脱敏
  Channels/                         平台实现和长文本分段
src/NotifyRedirect.Windows/
  Notifications/                    WinRT API 包装、监听和同步
  Startup/                          MSIX 登录启动
src/NotifyRedirect.App/
  App.xaml, App.xaml.cs             生命周期、组合根、单实例
  MainWindow.xaml, MainWindow.xaml.cs
  Views/, ViewModels/               四个页面和首次使用引导
  Tray/                             托盘与退出
src/NotifyRedirect.LocalApi/         Kestrel 主机、统一消息验证
src/NotifyRedirect.Cli/              send 子命令
integrations/                       Agent 配置示例，不改用户既有配置
packaging/AppxManifest.xml           包身份、通知读取能力、全信任桌面进程声明
packaging/Assets/                    本地生成的简单安装图标
scripts/                            构建、打包、安装指导脚本
tests/NotifyRedirect.Tests/          Core 和可模拟基础设施测试
tests/NotifyRedirect.Windows.Tests/  Windows 集成测试
docs/validation/                    实测矩阵和结果
README.md                           安装、配置与锁屏说明
```

每个任务先运行所列测试确认失败，再实现，再跑通过；仅对行为和可靠性写测试，界面布局和文案使用人工检查。新依赖从官方 NuGet 解析稳定版本并固定，提交锁文件，不使用浮动版本。

每个任务验证通过后提交该任务涉及的具体文件。如果当前仍非 Git 仓库，在执行阶段初始化仓库后提交，不添加构建产物、数据库、凭据、证书或本地 SDK。首轮无需创建隔离 worktree，因为目录尚无现有产品代码或用户未提交修改。

## Task 1：可安装的通知监听诊断版

**Files:** `NotifyRedirect.sln`, `global.json`, `Directory.Build.props`, `.gitignore`, `src/NotifyRedirect.Core/Models/ForwardMessage.cs`, `src/NotifyRedirect.Core/Models/SourceSnapshot.cs`, `src/NotifyRedirect.Windows/Notifications/IWindowsNotificationApi.cs`, `WindowsNotificationApi.cs`, `src/NotifyRedirect.App/App.xaml`, `App.xaml.cs`, `MainWindow.xaml`, `MainWindow.xaml.cs`, `packaging/AppxManifest.xml`, `packaging/Assets/*`, `scripts/Build-Msix.ps1`, `scripts/Install-DevPackage.ps1`, `tests/NotifyRedirect.Windows.Tests/PermissionTests.cs`, `docs/validation/windows-listener.md`，及对应 csproj。

**Interfaces:** `ForwardMessage(string Id, string SourceKey, string AppId, string AppName, string Title, string Body, DateTimeOffset OccurredAt, string? Project, string? SessionId, AgentState State)`；`AgentState` 为 Unknown、TurnEnded、ProcessExited、Succeeded、Failed、WaitingInput。`SourceSnapshot(string SourceKey, string AppId, string AppName, DateTimeOffset CreatedAt, IReadOnlyList<string> TextElements)`。`IWindowsNotificationApi.RequestAccessAsync(CancellationToken) -> Task<ListenerAccess>`、`GetAccess() -> ListenerAccess`、`ReadAsync(CancellationToken) -> Task<IReadOnlyList<SourceSnapshot>>`；`ListenerAccess` 为 Allowed、Denied、Unspecified、Unavailable。

- [ ] 建立权限契约测试 `DeniedDoesNotRead`、`UnspecifiedCanRequestAgain`、`RevokedPermissionIsNotEmptySuccess`，断言 Denied 不调用读取、撤销显示权限状态而不是零条消息正常状态。
- [ ] 在项目目录安装本地 .NET 10 SDK 到 `.tools/dotnet`（从微软官方来源，不替换全局 SDK），使用 `.tools/dotnet/dotnet.exe --info` 确认 10.x；创建最小项目及测试，运行 `dotnet test NotifyRedirect.sln --filter FullyQualifiedName~PermissionTests`，确认测试因实现缺失失败。后续命令中的 dotnet 均指该本地 SDK；脚本显式选择它。
- [ ] 实现 API 包装和诊断窗口；在 UI 线程请求授权、每次读取前检查权限，清单声明 `uap3:userNotificationListener` 与桌面进程能力。包使用 MakeAppx 及 SignTool 构建；检查现有 Windows SDK 工具，无需引入 WinUI 或 Windows App SDK 背景 COM 服务。监听随桌面进程常驻，显式退出后不继续监听。
- [ ] 测试通过并构建 MSIX；开发证书在用户明确同意后才信任到证书存储，安装包注册也需用户批准。签名密钥排除版本控制，不尝试静默降低系统安全设置。
- [ ] 手动安装并验证授权、拒绝、撤销、读取 ChatGPT/Cursor 原始文本、Win+L 后创建通知及唤醒状态；记录实际操作系统、源应用版本和读取结果。此步不要求外部凭据。若 WPF 包身份无法获得监听能力，先查官方支持并修正 Windows 适配层，不继续制作依赖不可用接口的完整界面。
- [ ] 提交诊断版及实测记录：`feat: add packaged Windows notification listener`。若安装/授权等操作尚待用户，记录待验收项，不声称通过锁屏测试。

## Task 2：标准消息、应用选择与可靠通知同步

**Files:** `src/NotifyRedirect.Core/Models/NotificationIdentity.cs`, `Models/SourceSelection.cs`, `src/NotifyRedirect.Core/Delivery/INotificationLedger.cs`, `src/NotifyRedirect.Windows/Notifications/NotificationMapper.cs`, `NotificationSynchronizer.cs`, `NotificationSource.cs`, `tests/NotifyRedirect.Tests/NotificationMappingTests.cs`, `NotificationSyncTests.cs`。

**Interfaces:** `NotificationMapper.Map(SourceSnapshot) -> ForwardMessage?`；`NotificationIdentity` 用应用 AUMID、通知 ID、创建时间形成 SourceKey；`INotificationLedger.ObserveAsync(ForwardMessage, bool baselineOnly, CancellationToken) -> Task<Observation>`，Observation 为 Baseline、Duplicate、New、Changed；ledger 具体实现在 Task 4。`NotificationSource.RunAsync(Func<ForwardMessage,CancellationToken,Task> onMessage,CancellationToken) -> Task`。测试先使用内存 ledger。

- [ ] 写 `MapsTitleAndAllBodyElements`、`AllowsTitleOnly`、`SkipsEmptyText`、`BadEntryDoesNotStopBatch`、`FirstConsentBaselinesExistingNotifications`、`DuplicateDoesNotEmit`、`ChangedContentEmitsUpdate`、`ReusedIdAcrossAppsOrCreationTimesIsNew`，精确断言输入文本顺序和输出消息数。
- [ ] 运行 `dotnet test NotifyRedirect.sln --filter "FullyQualifiedName~NotificationMappingTests|FullyQualifiedName~NotificationSyncTests"` 确认失败。
- [ ] 实现映射、逐条异常隔离和串行同步；订阅通知变化事件，并每 5 秒作一次补偿读取；防止重叠扫描。首次授权建立基线，后续重启使用持久化标识。应用勾选使用稳定 AUMID，不以显示名作为身份。无法枚举未知应用时说明只有观察到通知后才会出现在列表。
- [ ] 跑测试通过；手动验证同一通知更新、重启、两个应用相同标题，记录无意义全文关键词推断不会把 Unknown 改为 Succeeded。
- [ ] 提交：`feat: normalize and synchronize notification sources`。

## Task 3：时间段、应用与关键词过滤

**Files:** `src/NotifyRedirect.Core/Models/ForwardingRules.cs`, `Scheduling/ScheduleEvaluator.cs`, `Scheduling/MessageFilter.cs`, `tests/NotifyRedirect.Tests/ScheduleTests.cs`, `FilterTests.cs`。

**Interfaces:** `ScheduleMode` 为 Always、Scheduled、Paused；`ScheduleWindow(IReadOnlySet<DayOfWeek> StartDays, TimeOnly Start, TimeOnly End, bool AllDay)`；`ForwardingRules(ScheduleMode Mode, IReadOnlyList<ScheduleWindow> Windows, IReadOnlySet<string> AppIds, IReadOnlyList<string> IncludeKeywords, IReadOnlyList<string> ExcludeKeywords)`。`ScheduleEvaluator.IsAllowed(DateTimeOffset instant,TimeZoneInfo zone,ForwardingRules rules) -> bool`；`MessageFilter.Accept(ForwardMessage message,ForwardingRules rules) -> bool`。

- [ ] 写如下边界测试及关键词空值、排除优先测试。关键词按标题与正文的大小写不敏感子串匹配，包含列表为空允许、非空任意命中允许；排除任意命中即拒绝。

```csharp
// 测试夹具配置 Friday 18:30—09:00，时区 China Standard Time。
Assert.False(Allowed("2026-10-16T18:29:59+08:00"));
Assert.True(Allowed("2026-10-16T18:30:00+08:00"));
Assert.True(Allowed("2026-10-17T08:59:59+08:00"));
Assert.False(Allowed("2026-10-17T09:00:00+08:00"));
```

- [ ] `dotnet test NotifyRedirect.sln --filter "FullyQualifiedName~ScheduleTests|FullyQualifiedName~FilterTests"` 确认失败。
- [ ] 实现时区转换和跨午夜起始星期判断；相同起止时间普通规则验证失败；重叠规则取并集。增加 `ClockChangeReevaluatesWindow` 和 `DstRepeatedHourUsesLocalWindow`，以 DateTimeOffset 固定瞬时，不能用本地 DateTime 做有效期计时。
- [ ] 测试全部通过，并验证模板未保存不会启用，Paused 始终拒绝自动转发。
- [ ] 提交：`feat: add forwarding schedules and filters`。

## Task 4：SQLite 队列、配置与凭据

**Files:** `src/NotifyRedirect.Core/Delivery/IForwardingStore.cs`, `Models/ChannelConfig.cs`, `Models/DeliveryRecord.cs`, `src/NotifyRedirect.Infrastructure/Storage/SqliteStore.cs`, `SchemaMigrator.cs`, `RetentionCleaner.cs`, `Security/SecretProtector.cs`, `Security/Redactor.cs`, `tests/NotifyRedirect.Tests/StorageTests.cs`, `tests/NotifyRedirect.Windows.Tests/SecretProtectionTests.cs`。

**Interfaces:** `ChannelConfig(string Id,ChannelKind Kind,string Name,bool Enabled,string SecretRef)`，ChannelKind 为 Feishu、DingTalk、WeCom。`DeliveryStatus` 为 Pending、InFlight、Succeeded、RetryableFailed、PermanentFailed、Uncertain、Expired。`DeliveryRecord(string Id,string MessageId,string ChannelId,int PartIndex,string? PartBody,DeliveryStatus Status,int RetryCount,DateTimeOffset? NextAttemptAt,DateTimeOffset ExpiresAt,string? ErrorCode)`。`PartIndex=-1` 表示尚未分段的父项；正文分段后父项不再参与发送。`IForwardingStore` 同时实现 Task 2 ledger，提供 `EnqueueAsync(ForwardMessage,IReadOnlyList<ChannelConfig>,CancellationToken) -> Task`、`ExpandPartsAsync(string parentId,IReadOnlyList<string> bodies,CancellationToken) -> Task`、`TakeDueAsync(DateTimeOffset,CancellationToken) -> Task<DeliveryRecord?>`、`GetMessageAsync(string,CancellationToken) -> Task<ForwardMessage?>`、`SaveDeliveryAsync(DeliveryRecord,CancellationToken) -> Task`、`PurgeAsync(DateTimeOffset,CancellationToken) -> Task`。ExpandPartsAsync 在一个事务中存入分段正文并关闭父项。`SecretProtector.Protect(string) -> byte[]`、`Unprotect(byte[]) -> string`。

- [ ] 写数据库事务、重启、通知更新替换未发项、独立渠道状态、原子领取测试；正文第 7 天清除、状态第 30 天清除、未过期队列内容保留。写 `NewerSchemaRefusesWithoutDeletingDatabase`、`DamagedSecretRequiresReconfiguration` 和导出不含 Webhook、token、正文的断言。
- [ ] `dotnet test NotifyRedirect.sln --filter "FullyQualifiedName~StorageTests|FullyQualifiedName~SecretProtectionTests"` 确认失败。
- [ ] 实现 SQLite WAL 与事务，唯一键覆盖来源身份和内容指纹；数据库位于用户本地数据目录。进程崩溃留下的 InFlight 恢复为 Uncertain，不能假定未发送。迁移只向前，先备份；密钥 DPAPI CurrentUser，损坏时保留配置并显示重新配置要求。RetentionCleaner 启动及每日执行。
- [ ] 跑测试通过；手动重启进程核对队列存在，日志及导出不含凭据。数据库正文可由同一用户读取，README 不宣称整个数据库已加密。
- [ ] 提交：`feat: persist delivery queue and protect credentials`。

## Task 5：发送管线、时段控制与重试

**Files:** `src/NotifyRedirect.Core/Delivery/INotificationChannel.cs`, `SendOutcome.cs`, `ForwardingPipeline.cs`, `DeliveryWorker.cs`, `RetryPolicy.cs`, `tests/NotifyRedirect.Tests/PipelineTests.cs`, `DeliveryWorkerTests.cs`。

**Interfaces:** `INotificationChannel.Kind -> ChannelKind`、`PrepareParts(ChannelConfig,ForwardMessage) -> IReadOnlyList<string>`、`SendAsync(ChannelConfig,ForwardMessage,DeliveryRecord,CancellationToken) -> Task<SendOutcome>`；SendAsync 只发送已存储的 PartBody，不能重新分段。`SendOutcome(DeliveryStatus Status,string? SafeErrorCode,TimeSpan? RetryAfter)`。`ForwardingPipeline.HandleAsync(ForwardMessage,CancellationToken) -> Task`；`DeliveryWorker.RunOnceAsync(CancellationToken) -> Task<bool>`；`RetryPolicy.NextDelay(int retryCount,TimeSpan? retryAfter) -> TimeSpan?`。工作器领取父项后调用 PrepareParts 与 ExpandPartsAsync，后续按分段记录发送。注入 TimeProvider、当前规则提供函数、TimeZoneInfo 获取函数、store 和渠道注册表，避免隐藏全局时钟。

- [ ] 写 `OutsideOccurrenceWindowNeverQueues`、`OutsideCurrentWindowNeverSends`、`PausedNeverSends`、`Exactly24HoursExpires`、`OnlyFailedChannelRetries`、`FiveRetriesThenStops`、`SuccessBodyErrorIsFailure`、`TimeoutAfterAcceptanceIsUncertain`、`RestartInFlightIsUncertain`、`RuleChangeCancelsQueuedDisallowedSource`。
- [ ] `dotnet test NotifyRedirect.sln --filter "FullyQualifiedName~PipelineTests|FullyQualifiedName~DeliveryWorkerTests"` 确认失败。
- [ ] 实现事件时间和发送时刻双检查；时段外不排队。队列有效期从首次本机接收时刻计 24 小时，拒绝未来超过 5 分钟的自报时间。重试基础延迟为 30 秒、2 分钟、10 分钟、30 分钟、2 小时，有效 RetryAfter 取较大值，若下次已过期直接 Expired；永久错误不重试。每个请求 15 秒超时，失败消息只展示脱敏原因。
- [ ] Uncertain 保持独立可见状态，不自动重试以降低重复风险；用户手动重发允许重复并遵守时间规则。显式渠道测试绕过自动时段但仍需要用户点击和平台安全限制。全部测试通过。
- [ ] 提交：`feat: enforce schedules and reliable delivery retries`。

## Task 6：飞书及其他平台渠道

**Files:** `src/NotifyRedirect.Infrastructure/Channels/FeishuChannel.cs`, `DingTalkChannel.cs`, `WeComChannel.cs`, `ChannelAvailability.cs`, `MessageSplitter.cs`, `WebhookValidator.cs`, `tests/NotifyRedirect.Tests/ChannelTests.cs`, `MessageSplitterTests.cs`, `docs/validation/platforms.md`。

**Interfaces:** 各渠道实现 Task 5 INotificationChannel。`MessageSplitter.Split(string text,int maxUtf8Bytes) -> IReadOnlyList<string>`；`ChannelAvailability` 保存当前可用、需管理员设置、需验证、不可用及依据。平台密钥结构只在 Infrastructure 定义，Core 不暴露明文凭据。

- [ ] 实施前读取三个平台当前官方接口、业务错误码、安全要求及消息长度限制，记录到 platforms.md，并把具体限制写入对应实现和测试。企业微信官方页面此前抓取失败，需通过可访问的官方页面或浏览器重新核对；钉钉核对新用户现行方案，旧 Webhook 仅作为已创建机器人兼容模式，不默认推荐给新用户。
- [ ] 写 HTTP 模拟测试：正确序列化和签名、HTTP 200 业务失败、401/403 配置错误、429 限流、5xx 暂时失败、超时 Uncertain、响应解析失败、禁用渠道拒绝发送；写中文、表情、一个字符超过限制、分段序号和每段状态独立测试。
- [ ] `dotnet test NotifyRedirect.sln --filter "FullyQualifiedName~ChannelTests|FullyQualifiedName~MessageSplitterTests"` 确认失败。
- [ ] 实现已核实的渠道，校验 HTTPS 官方 Webhook 主机与路径，不允许任意地址或跨主机重定向。正文总上限 64 KiB UTF-8，超出明确标注截断；分段限制依据官方接口，预留序号与元信息字节。分段队列在首次发送前持久化，成功段不重发。平台回包正文不直接写日志。
- [ ] 运行测试通过；使用用户提供的测试机器人配置，分别点击测试并在手机端确认。用户未提供某平台条件时保留“未实测”标识，不编造凭据、不自动创建外部机器人；若新方案需企业审批，将其作为明确的渠道条件，不把接口按钮当作已完成支持。
- [ ] 首次外部完整链路：选定 ChatGPT/Cursor → 时间规则 → 飞书 → 手机确认；保存脱敏结果。提交：`feat: add verified notification delivery channels`。

## Task 7：完整界面、托盘与首次使用流程

**Files:** `src/NotifyRedirect.App/Views/{Home,Sources,Channels,RulesHistory,Onboarding}View.xaml`, 对应 `ViewModels/*ViewModel.cs`, `Tray/TrayController.cs`, `App.xaml.cs`, `MainWindow.xaml`, `src/NotifyRedirect.Windows/Startup/StartupRegistration.cs`, `src/NotifyRedirect.Infrastructure/Storage/SettingsRepository.cs`, `tests/NotifyRedirect.Tests/SettingsValidationTests.cs`, `docs/validation/ui-checklist.md`。

**Interfaces:** `SettingsRepository.LoadAsync(CancellationToken) -> Task<AppSettings>`、`SaveAsync(AppSettings,CancellationToken) -> Task`；AppSettings 包含 Task 3 rules、Task 4 channels、启动偏好和可配置电脑名称。`StartupRegistration.SetEnabledAsync(bool,CancellationToken) -> Task<StartupState>`，区分 Enabled、Disabled、DisabledByUser、Unavailable，不把保存偏好视为系统启动已开启。

- [ ] 写设置验证测试：开始结束相等拒绝、空勾选不转发、无启用渠道显示未配置、密钥损坏要求重配、配置导入只更新明确字段且不启用缺少凭据的渠道；明确错误文案所对应状态。
- [ ] `dotnet test NotifyRedirect.sln --filter FullyQualifiedName~SettingsValidationTests` 确认失败。
- [ ] 实现四页和首次使用引导，采用原生 WPF 控件及简洁中文；配置先预览后保存，显示当前状态、下次窗口和跨午夜归属。发送历史按消息展开到渠道和分段。失败记录提供时段内手动重发；测试按钮单独标识可在时段外发送。
- [ ] 实现单实例、关闭主窗口转托盘、显式退出取消循环并关闭数据库；不模拟鼠标活动维持唤醒。MSIX StartupTask 用用户主动启用，不偷偷写注册表。允许屏幕关闭但保持电脑唤醒的要求放在引导和状态说明。
- [ ] 测试通过；人工检查 100%/150% 缩放、键盘操作、长应用名、错误提示、最小窗口、密钥不回显、数据清理、设置恢复、托盘退出与升级后的数据保留，保存检查结果。
- [ ] 提交：`feat: add desktop configuration and tray workflow`。

## Task 8：通用 Agent 入口与首批适配示例

**Files:** `src/NotifyRedirect.LocalApi/LocalApiHost.cs`, `EventValidator.cs`, `src/NotifyRedirect.Cli/Program.cs`, `integrations/{claude-code,opencode,codex-cli,cursor}/README.md`, 各自脚本/插件示例, `tests/NotifyRedirect.Tests/LocalApiTests.cs`, `AgentMappingTests.cs`, `docs/validation/agents.md`。

**Interfaces:** JSON v1 包含 `version=1,eventId,source,title,body,occurredAt,project?,sessionId?,state`；`state` 值对应 Task 1 AgentState，未知值拒绝而不是映射成功。AppId 使用 `agent:<source>`，SourceKey 使用 `agent:<source>:<eventId>`，在来源页独立勾选。`LocalApiHost.StartAsync(int port,CancellationToken) -> Task` 默认仅 `127.0.0.1:17841`，`POST /v1/events` 需要 Bearer token，最大请求体 128 KiB；`EventValidator.Validate(string json) -> EventValidationResult`，结果包含 ForwardMessage 或字段错误。CLI `notifyredirect send --source NAME --title TITLE --body BODY --state STATE`，或 `--stdin-json`。

- [ ] 写无 token、错误 token、非法版本、缺少字段、超大请求、将来时间、重复 eventId、无 sessionId 不跨来源合并、端口占用、未知状态以及正文敏感数据不入日志的测试。有效输入返回 202；过滤消息返回 202 并记录未转发，不能宣称已送达手机。
- [ ] `dotnet test NotifyRedirect.sln --filter "FullyQualifiedName~LocalApiTests|FullyQualifiedName~AgentMappingTests"` 确认失败。
- [ ] 实现入口，默认关闭，用户开启后生成随机令牌并用 DPAPI 保存；不开放 CORS，不监听所有网卡，不提供运行命令接口。端口占用展示修复方法，CLI 非零退出不终止源 Agent 工作。CLI token 来自用户保护存储，不放命令行或模板。
- [ ] 为 Claude Code、OpenCode、Codex CLI、Cursor 编写官方事件适配示例，只增加配置片段或让用户复制，不覆盖现有配置；读取结果限于用户启用的接口返回字段，不未经授权扫描完整对话目录。Codex CLI notify 只按已核实事件范围实现，不假定 ChatGPT 桌面支持该配置。Hook 失败不阻塞开发流程。
- [ ] 跑测试通过；每个适配器记录版本与实际验证事件。Gemini CLI、Copilot CLI、Aider 的后续接入说明先使用通用入口，不标为已完成专用适配器。
- [ ] 提交：`feat: add local agent event ingestion and integrations`。

## Task 9：安装包、升级及端到端验收

**Files:** `scripts/Build-Msix.ps1`, `scripts/Install-DevPackage.ps1`, `scripts/Verify.ps1`, `README.md`, `docs/validation/{release-checklist,windows-listener,platforms,agents,ui-checklist}.md`。

**Interfaces:** `Build-Msix.ps1 -Configuration Release -OutputDirectory <path> -SigningCertificateThumbprint <thumbprint>` 使用固定源目录、拒绝输出目录越界，使用用户选择的证书；证书私钥不写入仓库。`Verify.ps1` 运行 restore、build、test，并分别返回失败退出码。

- [ ] 增加安装后诊断检查：包身份存在、通知权限可见、UI 和托盘启动、版本升级保留配置与数据；正式包使用自包含 win-x64 发布，接收者无需另外安装 .NET SDK。明确开发自签包安装需要信任证书，不能称为公众无障碍安装。
- [ ] 运行 `dotnet restore NotifyRedirect.sln --locked-mode`、`dotnet build NotifyRedirect.sln -c Release --no-restore`、`dotnet test NotifyRedirect.sln -c Release --no-build`，必须全部通过；脚本只在最终验证发现问题或产品变更后重复，不做无依据重复测试。
- [ ] 构建 MSIX 并验证签名及清单；在目标 Windows 11 用户环境测试新装、升级和卸载。安装、信任证书和发送真实测试消息均让用户知道实际变更，不做不可见的外部操作。
- [ ] 实机验收：允许时段内锁屏后 ChatGPT 和 Cursor 新消息到手机；隐藏锁屏通知正文设置；时间段外不发送；断网恢复；程序重启；权限撤销；跨午夜；分段正文；各平台未验证条件清楚标记。
- [ ] README 包含安装、来源授权、渠道配置、时间规则、电源要求、CLI 与 ChatGPT 区别、日志位置、保留期、超时重复风险及现有兼容性矩阵。没有真实测试条件的项目明确列为待验收，不写“全部完成”。
- [ ] 提交：`docs: add installation and verified release checklist`；最终汇报可用构建、验证证据、未验收限制，不自动发布或上传。

## 交付阶段与依赖

- **阶段 1（Task 1–3）**：可以安装、读取与预览通知，验证锁屏和时间规则；尚无外部转发。
- **阶段 2（Task 4–7）**：可日常使用的 Windows 通知转发版，飞书完整链路，其他平台按实际资格和验证启用。
- **阶段 3（Task 8–9）**：通用 Agent 接入及可分享安装包，完成最终验收。

任务按顺序执行，接口强依赖，不建议在第一条锁屏通知链路通过前分散实施。用户未提供机器人凭据不会妨碍本地实现和模拟测试，但手机真实收件验收必须等用户配置。

## 计划自审与执行状态

- 已逐项映射设计范围：通用通知 Task 1–2；时间与筛选 Task 3；持久化及凭据 Task 4；可靠发送 Task 5；渠道 Task 6；界面/托盘/启动 Task 7；Agent 扩展 Task 8；安装与验收 Task 9。
- 五项 Review Focus 已分配测试，无缺失接口；IForwardingStore 与分段数据结构在 Task 4 定义，Task 6 增加实际分段行而不改变工作器入口。
- 本文件为待用户审阅的计划，产品代码尚未创建、依赖尚未安装、锁屏与外部渠道尚未实测。执行方式由用户选择后进入相应执行技能。

## 官方依据

- 通知能力、授权、读取与事件：https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener
- .NET 支持期限：https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core
- MSIX 应用类型：https://devblogs.microsoft.com/insidemsix/types-of-packaged-applications/
- 飞书机器人：https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot
- 钉钉现行方案核对入口：https://open.dingtalk.com/document/orgapp/custom-robot-access
- 钉钉旧机器人提醒：https://open-dingtalk.github.io/developerpedia/docs/learn/bot/webhook/overview/
- 企业微信核对入口：https://developer.work.weixin.qq.com/document/path/91770
