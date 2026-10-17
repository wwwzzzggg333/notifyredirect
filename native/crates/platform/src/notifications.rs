use notifyredirect_core::*;
use windows::{
    Foundation::TypedEventHandler,
    UI::Notifications::{
        KnownNotificationBindings,
        Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
        NotificationKinds,
    },
    Win32::System::WinRT::*,
    core::HSTRING,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Allowed,
    Denied,
    Unspecified,
    Unavailable,
}
fn convert(s: UserNotificationListenerAccessStatus) -> Access {
    match s {
        UserNotificationListenerAccessStatus::Allowed => Access::Allowed,
        UserNotificationListenerAccessStatus::Denied => Access::Denied,
        _ => Access::Unspecified,
    }
}
pub fn initialize_runtime() {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
}
pub trait NotificationApi {
    fn access(&self) -> Access;
    fn read(&self) -> Result<Vec<SourceSnapshot>>;
}
pub fn read_gated(api: &dyn NotificationApi) -> Result<(Access, Vec<SourceSnapshot>)> {
    let access = api.access();
    if access != Access::Allowed {
        return Ok((access, vec![]));
    }
    let items = api.read()?;
    let access = api.access();
    Ok((
        access,
        if access == Access::Allowed {
            items
        } else {
            vec![]
        },
    ))
}
pub struct WindowsNotifications {
    listener: UserNotificationListener,
    token: Option<i64>,
}
impl WindowsNotifications {
    pub fn new() -> Result<Self> {
        UserNotificationListener::Current()
            .map(|listener| Self {
                listener,
                token: None,
            })
            .map_err(|_| "通知接口不可用，请安装新版 MSIX 后运行".into())
    }
    pub fn subscribe(&mut self, wake: std::sync::mpsc::Sender<()>) {
        if self.token.is_none() && self.access() == Access::Allowed {
            self.token = self
                .listener
                .NotificationChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = wake.send(());
                    Ok(())
                }))
                .ok();
        }
    }
}
impl Drop for WindowsNotifications {
    fn drop(&mut self) {
        if let Some(t) = self.token {
            let _ = self.listener.RemoveNotificationChanged(t);
        }
    }
}
impl NotificationApi for WindowsNotifications {
    fn access(&self) -> Access {
        self.listener
            .GetAccessStatus()
            .map(convert)
            .unwrap_or(Access::Unavailable)
    }
    fn read(&self) -> Result<Vec<SourceSnapshot>> {
        let items = self
            .listener
            .GetNotificationsAsync(NotificationKinds::Toast)
            .and_then(|op| op.join())
            .map_err(|_| "读取通知失败".to_string())?;
        let mut result = vec![];
        for n in items {
            let parsed = (|| -> windows::core::Result<SourceSnapshot> {
                let app = n.AppInfo()?;
                let app_id = app.AppUserModelId()?.to_string();
                let time = n.CreationTime()?.UniversalTime;
                let unix = time - 116444736000000000;
                let created_at = chrono::DateTime::from_timestamp(
                    unix.div_euclid(10000000),
                    (unix.rem_euclid(10000000) * 100) as u32,
                )
                .ok_or(windows::core::Error::from_hresult(
                    windows::core::HRESULT(0x80070057u32 as i32),
                ))?;
                let binding = n
                    .Notification()?
                    .Visual()?
                    .GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
                let text = binding
                    .GetTextElements()?
                    .into_iter()
                    .filter_map(|t| t.Text().ok().map(|s| s.to_string()))
                    .collect();
                Ok(SourceSnapshot {
                    source_key: format!("win:{app_id}:{}:{time}", n.Id()?),
                    app_id,
                    app_name: app.DisplayInfo()?.DisplayName()?.to_string(),
                    text,
                    created_at,
                })
            })();
            if let Ok(s) = parsed {
                result.push(s)
            }
        }
        Ok(result)
    }
}
pub fn request_access(callback: impl Fn(Access) + Send + 'static) -> Result<()> {
    let listener =
        UserNotificationListener::Current().map_err(|_| "通知接口不可用，请安装新版 MSIX")?;
    let op = listener
        .RequestAccessAsync()
        .map_err(|_| "无法发起通知授权")?;
    op.when(move |result| callback(result.map(convert).unwrap_or(Access::Unavailable)))
        .map_err(|_| "无法等待通知授权".into())
}
pub fn set_startup(enabled: bool) -> Result<bool> {
    initialize_runtime();
    let task = windows::ApplicationModel::StartupTask::GetAsync(&HSTRING::from(
        "NotifyRedirectNativeStartup",
    ))
    .and_then(|op| op.join())
    .map_err(|_| "登录启动需要安装新版 MSIX")?;
    if enabled {
        let status = task
            .RequestEnableAsync()
            .and_then(|op| op.join())
            .map_err(|_| "启动项授权失败")?;
        Ok(status == windows::ApplicationModel::StartupTaskState::Enabled)
    } else {
        task.Disable().map_err(|_| "无法停用登录启动")?;
        Ok(false)
    }
}
pub fn startup_enabled() -> Result<bool> {
    initialize_runtime();
    windows::ApplicationModel::StartupTask::GetAsync(&HSTRING::from("NotifyRedirectNativeStartup"))
        .and_then(|op| op.join())
        .and_then(|t| t.State())
        .map(|s| s == windows::ApplicationModel::StartupTaskState::Enabled)
        .map_err(|_| "启动项不可用".into())
}
