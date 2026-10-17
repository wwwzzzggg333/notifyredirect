#![windows_subsystem = "windows"]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let smoke = args.iter().any(|s| s == "--smoke-ui");
    let dir = args
        .iter()
        .position(|s| s == "--data-dir")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);
    let result = (|| -> notifyredirect_core::Result<()> {
        if smoke && dir.is_none() {
            return Err("测试模式需显式临时目录".into());
        }
        let Some(_instance) = notifyredirect_app::ui::acquire_instance(smoke)? else {
            return Err("程序已在运行，请从系统托盘打开".into());
        };
        let path = match &dir {
            Some(p) => p.join("notifyredirect.db"),
            None => notifyredirect_platform::app_db()?,
        };
        let backend = notifyredirect_app::Backend::start(path, smoke)?;
        notifyredirect_app::ui::run(backend, if smoke { dir } else { None })
    })();
    if let Err(e) = result {
        if !smoke {
            unsafe {
                let text: Vec<u16> = e.encode_utf16().chain(Some(0)).collect();
                let _ = windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    None,
                    windows::core::PCWSTR(text.as_ptr()),
                    windows::core::w!("通知转发器启动失败"),
                    windows::Win32::UI::WindowsAndMessaging::MB_OK,
                );
            }
        }
        std::process::exit(1)
    }
}
