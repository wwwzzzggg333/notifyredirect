# NotifyRedirect Rust 全新版本实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Subagent execution is optional only if selected by the user.

**Goal:** 交付独立的 Windows 11 x64 Rust 通知转发 APP，保留中文原生界面和功能，降低安装包体积；不承担旧版兼容或迁移。

**Architecture:** `native/` 五 crate workspace；core 定义新模型和协议，storage 持久化新数据库，platform 访问 Windows，app 组合协调器/API/原生 UI，cli 提交新版事件。UI、SQLite 协调器和网络线程分离，持久状态由协调器唯一管理。

**Tech Stack:** Rust 1.96.0 / edition 2024 / x86_64-pc-windows-msvc；windows 0.62.2、rusqlite 0.40.2（bundled）、chrono 0.4.45（serde/clock）、serde 1.0.229（derive）、serde_json 1.0.154、sha2 0.10.9、hmac 0.12.1、base64 0.22.1、uuid 1.18.1（v4）、subtle 2.6.1。精确版本和 Cargo.lock 固定依赖。

**Spec:** `docs/superpowers/specs/2026-10-17-rust-rewrite-design.md`，已按用户“当成全新版本”的修正更新。

## Global Constraints

- 不迁移旧数据库、配置、凭据或历史，不保证旧 JSON/API/CLI 互通；不生成 C# oracle，不增加跨语言兼容代码或测试。
- 保留当前约定的通知、过滤、时间、三渠道、队列、历史、Agent、中文界面和托盘功能。
- 新 MSIX Identity `NotifyRedirect.Native`，Application Id `App`，版本 `1.0.0.0`，开发 Publisher `CN=NotifyRedirect Development`；文件名 `NotifyRedirect.exe` 与 `tools/notifyredirect-cli.exe`。
- 互斥 `Local\NotifyRedirect.Native`，启动项 `NotifyRedirectNativeStartup`，数据目录 `NotifyRedirectNative`，API `127.0.0.1:17842`，全部独立于旧程序。
- 新 schema v1、新 snake_case JSON、字符串枚举、RFC3339 UTC 时间；星期 1..7，1 为周一；字段长度按 Unicode 字符数。
- 默认暂停和空来源；首次读取只建基线；事件通知加每 5 秒读取补偿。
- 24 小时过期，最多 5 次重试，不确定结果不自动重试，正文 7 天、状态 30 天。
- 渠道总超时 15 秒/64 KiB 响应，CLI 总超时 3 秒；不引入 .NET/Electron/WebView/Chromium/OpenSSL。
- Release stage <=20 MiB，MSIX <=15 MiB 是待测目标，不通过删功能达标；原包 132,549,351 字节仅作对照。
- 只使用临时测试数据和模拟网络；不自动安装、信任证书、发送真实机器人消息或改动旧程序数据。

## Review Focus

1. 新旧版同时安装时，必须使用独立包身份、路径、互斥和端口，不能访问旧数据；Task 3/6/9 检查。
2. 去重与入队事务失败、更新分段或清空历史后迟到回调不能造成漏发/复活；Task 3/4 检查。
3. 发送中暂停/退出不能冻结 UI 或把未确认消息当作未发送重试；Task 4/8 用受控 transport 检查。
4. 中文/emoji 加序号后长度、慢分块响应、半包 HTTP 不能突破限额；Task 2/5/6 检查。
5. 首次授权基线和读取中权限撤销不能补发旧消息；Task 7 检查。

## 文件结构与类型

workspace 成员路径为 `native/crates/{core,storage,platform,app,cli}`，包名 `notifyredirect-{core,storage,platform,app,cli}`。每个成员有 Cargo.toml 和 src/lib.rs，app/cli 另有 src/main.rs。

公共模型在 core/src/model.rs：Settings、Rules、ScheduleWindow、Message、SourceSnapshot、ChannelConfig、ChannelSecret、DeliveryRecord、Outcome；采用 Rust snake_case 字段和 snake_case 字符串枚举。UtcTime 指 DateTime<Utc>，Duration 指 std::time::Duration，各 crate 自有 Error/Result。测试显式传 now，UUID 使用无连字符 hex。以下接口属于新版本，内部实现不参考旧格式。

## Task 1: Workspace、模型和时间规则

**Files:** Create `native/{Cargo.toml,Cargo.lock,rust-toolchain.toml}`；五个成员清单、lib.rs、app/cli main.rs；Create `native/crates/core/src/{model,settings,schedule,filter,error}.rs`；Create `native/crates/core/tests/{settings,rules}.rs`；Modify `.gitignore` 增加 native/target/。

**Interfaces:** `Settings::default()` 默认 paused、空来源/渠道、api_enabled=false；`settings::{decode(&str)->Result<Settings>,encode(&Settings)->Result<String>,validate(&Settings)->Result<()>}`；`schedule::{validate(&Rules)->Result<()>,is_allowed<Tz:TimeZone>(UtcTime,&Tz,&Rules)->bool}`；`filter::accept(&Message,&Rules)->bool`。

- [ ] 1. 写测试：`assert_eq!(Settings::default().rules.mode, Mode::Paused)`；新配置 roundtrip 相同，未知 enum/null 必需结构/重复渠道 Id 拒绝；北京时间周五 18:30 至周六 09:00，18:29:59=false、18:30=true、08:59:59=true、09:00=false，周日 08:00=false；全天/暂停/多规则及空星期/相等起止无效。过滤采用 Unicode lowercase 子串，排除优先，空白词忽略。
- [ ] 2. 建立最小 workspace 让测试运行；Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-core --test settings --test rules`，记录未实现接口导致的失败。
- [ ] 3. 实现接口；Release `opt-level="z"`, `lto=true`, `codegen-units=1`, `strip=true`。使用当前本地时区换算，不缓存 offset；星期按本地日期计算。依赖精确固定并生成锁文件。
- [ ] 4. 重跑同一命令，预期通过；提交 `feat: add fresh Rust configuration and scheduling`。

## Task 2: 通知、事件、Unicode 文本和重试

**Files:** Create `native/crates/core/src/{notification,event,text,retry}.rs`；Create `native/crates/core/tests/{events,text,notifications}.rs`；Modify core/src/lib.rs。

**Interfaces:** `notification::{map(SourceSnapshot)->Option<Message>,fingerprint(&Message)->String}`；`event::parse(&[u8],UtcTime)->Result<Message>`；`text::{split(&str,usize)->Result<Vec<String>>,prepare_parts(&Message,ChannelKind,&str)->Result<Vec<String>>}`；`retry::next_delay(u32,Option<Duration>)->Option<Duration>`。

- [ ] 1. 写测试：首文本为标题，其余为正文，无文本跳过，普通通知 state=unknown；同 source key 和内容指纹相同；新版 event snake_case 字段、version=1、字符串 state 校验；source/event_id/title/body 字符上限 80/200/1000/65536，1000 个 emoji 标题接受、1001 个拒绝；过去恰好 24h、未来恰好 5min 接受，超界拒绝。
- [ ] 2. 写测试：`let p=split("你好🙂世界🙂",8).unwrap(); assert_eq!(p.concat(),"你好🙂世界🙂"); assert!(p.iter().all(|s|s.len()<=8));`；完整分段含 `[i/n]\n` <=1800（WeCom）或15000（其他），65536 总长度包含截断说明。retry 默认依次 30/120/600/1800/7200 秒，`assert_eq!(next_delay(5,None),None)`；Retry-After 取较大值。
- [ ] 3. Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-core` 确认失败，实施上述接口后重跑通过；指纹 SHA-256 输入为 title+NUL+body，新 source key 自定义稳定格式，不要求旧 ticks 编码。
- [ ] 4. Commit `feat: add native event validation and message formatting`。

## Task 3: 新 SQLite 与 DPAPI

**Files:** Create `native/crates/storage/src/{store,settings,error}.rs`；Create `native/crates/platform/src/{paths,secrets,error}.rs`；Create `native/crates/storage/tests/{transactions,recovery}.rs` 和 `native/crates/platform/tests/{paths,secrets}.rs`。

**Interfaces:** `Store::{open(&Path)->Result<Self>,open_read_only(&Path)->Result<Self>,get_value(&self,&str)->Result<Option<String>>,set_value(&mut self,&str,&str)->Result<()>,observe_and_enqueue(&mut self,&Message,bool,&[ChannelConfig],bool,UtcTime)->Result<Observation>,set_baseline(&mut self)->Result<()>,has_baseline(&self)->Result<bool>,expand_parts(&mut self,&DeliveryRecord,&[String])->Result<()>,due(&self,UtcTime)->Result<Vec<DeliveryRecord>>,history(&self)->Result<Vec<DeliveryRecord>>,get_message(&self,&str)->Result<Option<Message>>,mark(&mut self,&DeliveryRecord)->Result<()>,purge(&mut self,UtcTime)->Result<()>,clear_history(&mut self)->Result<()>}`。两个 bool 为 baseline、should_enqueue。`storage::settings::{load(&Store)->Result<Settings>,save(&mut Store,&Settings)->Result<()>,export(&Settings)->Result<String>,import(&str)->Result<Settings>}`；`secrets::{protect(&str)->Result<String>,unprotect(&str)->Result<String>}` 返回/读取 Base64 DPAPI；`paths::{app_db()->Result<PathBuf>,cli_db(Option<&Path>)->Result<PathBuf>}`。

- [ ] 1. 写新库 roundtrip、重开后持久化、未知 schema 无修改拒绝、只读打开不创建库/不恢复 InFlight、DPAPI 当前用户往返和损坏密文报错测试。路径必须包含 NotifyRedirectNative，从不给旧包或 NotifyRedirect 目录探测请求；显式 --data-dir 使用指定目录。
- [ ] 2. 写事务测试：故意失败 INSERT 后重试仍 New；baseline/duplicate/changed；未发送分段替换不延长 expires，已尝试后生成新消息；重开 InFlight=>Uncertain；恰好7d清空正文、30d删除状态；清空取消队列。导出不含秘密/历史，导入停用并生成新渠道 Id。
- [ ] 3. Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-storage -p notifyredirect-platform` 确认失败；实现独立 schema v1 和参数化 SQL/WAL、显式事务、新格式序列化及 DPAPI RAII；不添加 backup、takeover 或任何旧格式处理。重跑全部通过。
- [ ] 4. Commit `feat: add standalone Rust data and credential storage`。

## Task 4: 协调器、队列和恢复

**Files:** Create `native/crates/app/src/{pipeline,worker,coordinator,commands,error}.rs`；Create `native/crates/app/tests/{pipeline,worker,coordinator}.rs`。

**Interfaces:** `pipeline::should_enqueue(&Message,&Settings,UtcTime)->bool`；`worker::{next_action(&Store,&Settings,UtcTime)->Result<WorkerAction>,apply_outcome(&mut Store,&DeliveryRecord,Outcome,UtcTime)->Result<()>}`；WorkerAction=Idle/Expand/Send/Mark（各带相应 Message、ChannelConfig、DeliveryRecord 或分段正文）。`coordinator::start(PathBuf)->Result<AppHandle>`；`AppHandle::{send(Command)->Result<()>,try_event()->Option<AppEvent>,shutdown(Duration)->Result<()>}`。Command 包含 SaveSettings、SaveChannelSecret、DeleteChannel、SetApiEnabled、Ingest{message,baseline,receipt:Option<SyncSender<Result<()>>>}、CompleteBaseline、Refresh、TestChannel、Retry、ClearHistory、Pause、Exit；AppEvent=Snapshot/Feedback/Stopped；Snapshot 含 settings/sources/history/latest_message，无明文密钥。网络任务由内部通道派发。

- [ ] 1. 写发生/当前时间任一时段外不入队、暂停不发、禁用来源/渠道不发、24h精确过期、两渠道仅失败渠道重试、5次上限和Uncertain不重试测试；手动重发仅失败/不确定项且遵守规则；清空后迟到发送回调不重新建记录。
- [ ] 2. 注入可阻塞假 transport，发送期间保存暂停和退出，send(Command) 不阻塞 UI；停机最多5秒，新请求不派发，在途未确认重开为Uncertain。用显式时钟/屏障，无真实长时间 sleep。
- [ ] 3. Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-app --test pipeline --test worker --test coordinator`，确认失败；实现后重跑通过。协调线程独占 SQLite，每批 due <=20，每5秒调度，每24小时 purge（含启动），所有配置先保存成功再发布。receipt 仅在入站事务提交后应答。
- [ ] 4. Commit `feat: implement durable native forwarding coordinator`。

## Task 5: WinHTTP 和平台渠道

**Files:** Create `native/crates/platform/src/{http,webhook}.rs`；Create `native/crates/platform/tests/{http,webhook}.rs`；Modify app coordinator 接入。

**Interfaces:** HttpRequest{url,headers,body,timeout,max_response_bytes}；HttpResponse{status,headers,body}；HttpError=NotSent/Uncertain/ResponseTooLarge；`trait Transport:Send+Sync { fn send(&self,&HttpRequest)->Result<HttpResponse,HttpError>; }`，WinHttpTransport 实现；`webhook::{build_request(ChannelKind,&ChannelSecret,&str,UtcTime)->Result<HttpRequest>,classify(Result<HttpResponse,HttpError>,UtcTime)->Outcome}`。

- [ ] 1. 写三平台 payload/HMAC 固定向量及官方 HTTPS host/path/默认端口/无 userinfo或fragment 断言；成功业务码0、429/503可重试、401永久失败、未知/错误响应不确定。Retry-After delta/date 解析，平台限流码99991400/130101/45009等待至少1min。向量按官方协议推导，不调用旧实现。
- [ ] 2. 本地模拟 HTTP 验证302不跳转、累计响应>65536停止、慢分块触发15秒总deadline；timeout/cancel/中途断连不确定，确认未发送的连接失败可重试。测试使用短deadline，生产Webhook校验不允许loopback。
- [ ] 3. Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-platform --test http --test webhook` 确认失败；实现系统 TLS、WinHTTP RAII/取消与整体deadline（不是仅各阶段timeout）、有界读取；接入 worker 网络线程。重跑通过，不发送真实机器人。
- [ ] 4. Commit `feat: implement Windows HTTP channel delivery`。

## Task 6: 新 Agent API、CLI 和接入示例

**Files:** Create `native/crates/app/src/api.rs`、`native/crates/cli/src/{args,client}.rs`；Modify cli/src/main.rs；Create `native/crates/app/tests/api.rs`、`native/crates/cli/tests/cli.rs` 和 `integrations/native/{README.md,claude-code.md,opencode.md,codex-cli.md,cursor.md}`。

**Interfaces:** `api::start(SocketAddrV4,String,Sender<Command>)->Result<ApiHandle>`，ApiHandle::{local_addr,stop}；仅loopback，测试端口0。`args::parse(&[String],&str,UtcTime)->Result<Option<Vec<u8>>>`（None=help）；`client::submit(&Path,&[u8],&dyn Transport)->Result<()>`。app bin=NotifyRedirect，cli bin=notifyredirect-cli。生产固定端口17842，JSON snake_case和状态字符串按spec。

- [ ] 1. 写真实loopback API测试：401/415/400/413/202和存储失败503；合法事件回调一次，重复 event_id 不重复排队，未知来源仅登记；131072字节body以内仍需JSON有效，131073拒绝；16KiB头、8连接、每请求5秒限制；冲突Content-Length/TE、半包、chunked累计超限和stop释放连接。
- [ ] 2. 写CLI进程测试：帮助0；新版flags/stdin协议成功0；坏参数/401/入口关闭1；--data-dir只读新库、3秒deadline、不跟随跳转，不暴露token。示例只使用新版事件字段和状态，不承诺旧hook可直接运行。
- [ ] 3. Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-app --test api -p notifyredirect-cli --test cli` 确认失败；实现有界HTTP/1.1（单请求Connection:close）、constant-time鉴权、持久化receipt后202、可取消handler；接入开关失败不保存enabled=true；实现CLI及四类新版文档。重跑通过。
- [ ] 4. Commit `feat: add fresh native Agent API and CLI`。

## Task 7: Windows 通知和启动项

**Files:** Create `native/crates/platform/src/{notifications,startup}.rs`、`native/crates/app/src/listener.rs`；Create `native/crates/platform/tests/notifications.rs`、`native/crates/app/tests/listener.rs`。

**Interfaces:** ListenerAccess=Allowed/Denied/Unspecified/Unavailable；`trait NotificationApi { fn access(&self)->ListenerAccess; fn read(&self)->Result<Vec<SourceSnapshot>>; }`；WindowsNotifications::{new,request_access,subscribe,unsubscribe}；`listener::refresh(&dyn NotificationApi,&AppHandle)->Result<ListenerAccess>`；startup::{enabled()->Result<bool>,request_enable()->Result<bool>,disable()->Result<()>}。

- [ ] 1. 假接口测试：Denied不读取，读取中撤销丢弃本批，首次成功批（含空批）只建基线、重启不补发、后续新增/更新按规则，单条异常不影响其他项，授权失败不落baseline。
- [ ] 2. Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-platform --test notifications -p notifyredirect-app --test listener` 确认失败；实现WinRT通知读取、事件订阅退订和5秒补偿合并；UI STA只发起异步授权，不阻塞等待，读取工作线程初始化WinRT；启动项只响应主动勾选。
- [ ] 3. 重跑测试及 `cargo build --manifest-path native/Cargo.toml --workspace --locked` 全部通过；未安装时显示不可用，不把fake测试写成已通过实机通知/锁屏。Commit `feat: add notification listener and native startup`。

## Task 8: 四页原生 UI 与托盘

**Files:** Create `native/crates/app/src/ui/{mod,window,home,sources,channels,rules,history,tray,dialogs}.rs`、`native/crates/app/src/single_instance.rs`；Modify app main.rs；Create `native/crates/app/tests/{ui_commands,smoke}.rs`。

**Interfaces:** `ui::run(AppHandle)->Result<()>`；`single_instance::acquire()->Result<Option<InstanceGuard>>`；UiState{snapshot,selected_channel,selected_delivery} 无明文密钥；`ui::rules::parse_windows(&str)->Result<Vec<ScheduleWindow>>`；表单转换为Task4 Command，不直接访问库。

- [ ] 1. 测试所有spec页面操作对应正确Command：保存来源/关键词，渠道增删改/启停/保留已有密钥/测试，时间文本解析、电脑名、导入导出、历史详情/失败重发/清空、入口和启动项；失败保存不覆盖已生效配置。
- [ ] 2. 写隔离smoke入口 `NotifyRedirect.exe --smoke-ui --data-dir <temp>`，禁用真实通知/启动项/外发并用测试互斥；创建四页和tray，WM_CLOSE隐藏、打开恢复、暂停生效、退出释放。Run `cargo test --manifest-path native/Cargo.toml -p notifyredirect-app --test ui_commands --test smoke` 确认失败。
- [ ] 3. 实现Win32原生控件、密码框、文件对话框、滚动、Tab导航、中文字体和每监视器DPI；初始1060×760、最小850×620 DIP。实现TaskbarCreated恢复tray，双击/右键打开暂停退出，生产独立互斥和数据路径；网络/SQLite不在窗口消息处理函数执行。
- [ ] 4. 重跑测试和实际窗口smoke；无交互桌面则明确跳过并保留人工步骤。记录100%/150%/200%缩放、键盘访问。Commit `feat: build native settings window and tray`。

## Task 9: 新包、文档和体积验证

**Files:** Create `scripts/{Verify-Rust,Build-RustMsix,Sign-RustDevPackage}.ps1`、`packaging/rust/AppxManifest.xml`、`docs/validation/rust-release-checklist.md`；Modify README.md 添加全新Rust使用说明。原C#脚本和旧集成文档保持原状。

**Interfaces:** Verify-Rust.ps1 [-SkipUiSmoke]；Build-RustMsix.ps1 [-OutputDirectory artifacts/rust]；Sign-RustDevPackage.ps1 [-PackagePath artifacts/rust/NotifyRedirect-Native-1.0.0-x64.msix]；退出码传递，输出检查结果和尺寸JSON。

- [ ] 1. 写脚本验证：新版identity/version/startupTask/能力/文件路径正确，stage不含.NET/浏览器；独立输出目录不触碰旧包，删除前校验绝对路径位于artifacts/rust内；尺寸超目标明确失败，不伪报达标。
- [ ] 2. 实现cargo Release、复制Assets、MakeAppx和可选开发签名独立脚本；缺SDK/私钥明确报错，签名不自动信任/安装；不运行C#工程或添加旧版回归依赖。
- [ ] 3. 独立运行并逐一检查exit code：`cargo fmt --manifest-path native/Cargo.toml --all -- --check`；`cargo clippy --manifest-path native/Cargo.toml --workspace --all-targets --locked -- -D warnings`；`cargo test --manifest-path native/Cargo.toml --workspace --locked`；`cargo build --manifest-path native/Cargo.toml --workspace --release --locked --target x86_64-pc-windows-msvc`。均应为0。
- [ ] 4. Run `./scripts/Verify-Rust.ps1`、`./scripts/Build-RustMsix.ps1`、`./scripts/Sign-RustDevPackage.ps1`；验证签名，记录EXE/CLI/stage/MSIX字节数，stage<=20×1048576、包<=15×1048576。未达先分析依赖、重建并诚实报告。
- [ ] 5. 更新首次安装重新授权/配置、新版API/CLI、同机旧程序退出提示。检查清单区分自动通过、UI已测、待安装/锁屏/手机收件；不设置旧库迁移/备份/回退验收。完成分支审查并修复后重跑受影响检查，Commit `build: package standalone compact Rust release`。

## 自审与交接

依赖core独立、storage→core、platform→core、app/cli→core+storage+platform，无循环。九项任务覆盖新版本全部功能；Review Focus都有相应测试。移除旧版互通、DPAPI旧密文、C# fixture、schema接管、在线备份、旧CLI/适配器兼容和旧工程回归要求。生产故障恢复/DPAPI/去重事务仍保留，因为它们是新程序自身的可靠性要求。

本计划据用户修正更新，尚未实现；建议由当前会话直接执行并最终审查，不主动派发子代理。执行前检查当前checkout，优先复用合适工作目录。所有实现checkbox保持未完成。
