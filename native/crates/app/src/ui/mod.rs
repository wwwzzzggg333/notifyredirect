#![allow(unsafe_op_in_unsafe_fn)]
mod actions;
mod view;
use crate::{Backend, Command, Snapshot};
use notifyredirect_core::*;
use notifyredirect_platform::*;
use std::{collections::HashMap, ffi::c_void, path::PathBuf};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW, WinRT::*},
        UI::{Controls::*, HiDpi::*, Shell::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Item {
    page: usize,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
struct Ui {
    root: HWND,
    viewport: HWND,
    header: HWND,
    footer: HWND,
    nav: Vec<HWND>,
    controls: HashMap<i32, HWND>,
    items: Vec<Item>,
    font: HFONT,
    page: usize,
    scroll: i32,
    backend: Option<Backend>,
    snapshot: Option<Snapshot>,
    source_ids: Vec<String>,
    channel_id: Option<String>,
    history_ids: Vec<String>,
    initialized: bool,
    import_generation: u64,
    smoke: Option<PathBuf>,
    ticks: u32,
    tray_ok: bool,
    recreated: u32,
    dpi: u32,
}
impl Ui {
    fn send(&self, c: Command) {
        if let Some(b) = &self.backend {
            let _ = b.tx.send(c);
        }
    }
    unsafe fn text(&self, id: i32) -> String {
        let h = self.controls[&id];
        let len = GetWindowTextLengthW(h).max(0) as usize;
        let mut text = vec![0u16; len + 1];
        let n = GetWindowTextW(h, &mut text);
        String::from_utf16_lossy(&text[..n.max(0) as usize])
    }
    unsafe fn set(&self, id: i32, s: &str) {
        let text = wide(s);
        let _ = SetWindowTextW(self.controls[&id], PCWSTR(text.as_ptr()));
    }
    unsafe fn checked(&self, id: i32) -> bool {
        SendMessageW(self.controls[&id], BM_GETCHECK, None, None).0 == 1
    }
    unsafe fn check(&self, id: i32, b: bool) {
        SendMessageW(
            self.controls[&id],
            BM_SETCHECK,
            Some(WPARAM(b as usize)),
            None,
        );
    }
    unsafe fn report(&self, s: &str) {
        let text = wide(s);
        let _ = SetWindowTextW(self.footer, PCWSTR(text.as_ptr()));
    }
    unsafe fn combo(&self, id: i32) -> usize {
        SendMessageW(self.controls[&id], CB_GETCURSEL, None, None)
            .0
            .max(0) as usize
    }
    unsafe fn select_combo(&self, id: i32, index: usize) {
        SendMessageW(self.controls[&id], CB_SETCURSEL, Some(WPARAM(index)), None);
    }
    unsafe fn list_add(&self, id: i32, s: &str) {
        let text = wide(s);
        SendMessageW(
            self.controls[&id],
            LB_ADDSTRING,
            None,
            Some(LPARAM(text.as_ptr() as isize)),
        );
    }
    unsafe fn selection(&self, id: i32) -> Option<usize> {
        let n = SendMessageW(self.controls[&id], LB_GETCURSEL, None, None).0;
        if n < 0 { None } else { Some(n as usize) }
    }
    unsafe fn tray(&mut self, add: bool) {
        let mut d = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.root,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_APP + 1,
            hIcon: LoadIconW(None, IDI_INFORMATION).unwrap_or_default(),
            ..Default::default()
        };
        let tip = wide("通知转发器 · Rust 原生版");
        d.szTip[..tip.len()].copy_from_slice(&tip);
        self.tray_ok = Shell_NotifyIconW(if add { NIM_ADD } else { NIM_DELETE }, &d).as_bool();
    }
    unsafe fn layout(&mut self) {
        if self.viewport.0.is_null() {
            return;
        }
        let mut rect = RECT::default();
        let _ = GetClientRect(self.root, &mut rect);
        let scale = self.dpi as f64 / 96.0;
        let d = |v: i32| (v as f64 * scale).round() as i32;
        let width = rect.right;
        let height = rect.bottom;
        let _ = MoveWindow(self.header, d(20), d(14), width - d(40), d(44), true);
        for (i, h) in self.nav.iter().enumerate() {
            let _ = MoveWindow(*h, d(20 + i as i32 * 150), d(65), d(140), d(34), true);
        }
        let vh = (height - d(165)).max(d(100));
        let _ = MoveWindow(self.viewport, d(20), d(112), width - d(40), vh, true);
        let _ = MoveWindow(
            self.footer,
            d(20),
            height - d(45),
            width - d(40),
            d(40),
            true,
        );
        let full = self
            .items
            .iter()
            .filter(|i| i.page == self.page)
            .map(|i| i.y + i.h + 20)
            .max()
            .unwrap_or(0);
        let page = (vh as f64 / scale) as i32;
        self.scroll = self.scroll.clamp(0, (full - page).max(0));
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: full - 1,
            nPage: page.max(1) as u32,
            nPos: self.scroll,
            ..Default::default()
        };
        SetScrollInfo(self.viewport, SB_VERT, &si, true);
        for item in &self.items {
            let h = self.controls[&item.id];
            let _ = ShowWindow(
                h,
                if item.page == self.page {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
            if item.page == self.page {
                let w = if item.w < 0 {
                    width - d(100) - d(item.x)
                } else {
                    d(item.w)
                };
                let _ = MoveWindow(h, d(item.x), d(item.y - self.scroll), w, d(item.h), true);
            }
        }
    }
    unsafe fn render(&mut self, s: Snapshot) {
        let reset = !self.initialized || self.import_generation != s.import_generation;
        if reset {
            self.set(221, &s.settings.computer_name);
            self.set(121, &s.settings.rules.include_keywords.join("\r\n"));
            self.set(123, &s.settings.rules.exclude_keywords.join("\r\n"));
            self.set(
                211,
                &s.settings
                    .rules
                    .windows
                    .iter()
                    .map(|w| {
                        format!(
                            "{} | {}",
                            w.days
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join(","),
                            if w.all_day {
                                "all".into()
                            } else {
                                format!("{} | {}", w.start, w.end)
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\r\n"),
            );
            self.select_combo(
                201,
                match s.settings.rules.mode {
                    Mode::Always => 0,
                    Mode::Scheduled => 1,
                    Mode::Paused => 2,
                },
            );
        }
        let access = match s.access {
            Access::Allowed => "已授权 · 监听中",
            Access::Denied => "授权被拒绝，请在 Windows 设置中允许",
            Access::Unspecified => "尚未授权",
            Access::Unavailable => "接口不可用，请安装新版 MSIX",
        };
        let allowed = is_allowed(chrono::Utc::now(), &chrono::Local, &s.settings.rules);
        self.set(
            10,
            &format!(
                "通知权限：{access}\r\n当前{}自动转发 · 已启用 {} 个渠道",
                if allowed { "允许" } else { "暂停" },
                s.settings.channels.iter().filter(|c| c.enabled).count()
            ),
        );
        self.set(16, &s.preview);
        self.check(18, s.startup);
        self.check(231, s.settings.api_enabled);
        self.report(&s.feedback);
        let ids: Vec<_> = s.sources.keys().cloned().collect();
        if ids != self.source_ids || reset {
            let selected: Vec<_> = self
                .source_ids
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    SendMessageW(self.controls[&110], LB_GETSEL, Some(WPARAM(*i)), None).0 > 0
                })
                .map(|(_, id)| id.clone())
                .collect();
            SendMessageW(self.controls[&110], LB_RESETCONTENT, None, None);
            for (i, id) in ids.iter().enumerate() {
                self.list_add(110, s.sources.get(id).unwrap());
                let chosen = if reset || !self.source_ids.contains(id) {
                    s.settings.rules.app_ids.contains(id)
                } else {
                    selected.contains(id)
                };
                SendMessageW(
                    self.controls[&110],
                    LB_SETSEL,
                    Some(WPARAM(chosen as usize)),
                    Some(LPARAM(i as isize)),
                );
            }
            self.source_ids = ids;
        }
        let selected = self.channel_id.clone();
        SendMessageW(self.controls[&130], LB_RESETCONTENT, None, None);
        for (i, c) in s.settings.channels.iter().enumerate() {
            self.list_add(
                130,
                &format!(
                    "{} · {} · {} · 待手机实测",
                    c.name,
                    kind_name(c.kind),
                    if c.enabled { "启用" } else { "停用" }
                ),
            );
            if Some(&c.id) == selected.as_ref() {
                SendMessageW(self.controls[&130], LB_SETCURSEL, Some(WPARAM(i)), None);
            }
        }
        let previous = self
            .selection(240)
            .and_then(|i| self.history_ids.get(i).cloned());
        SendMessageW(self.controls[&240], LB_RESETCONTENT, None, None);
        self.history_ids.clear();
        for (d, m) in &s.history {
            self.list_add(
                240,
                &format!(
                    "{} · {} · {}",
                    status_name(d.status),
                    m.as_ref().map(|m| m.app_name.as_str()).unwrap_or("消息"),
                    m.as_ref().map(|m| m.title.as_str()).unwrap_or("")
                ),
            );
            self.history_ids.push(d.id.clone());
            if previous.as_deref() == Some(&d.id) {
                SendMessageW(
                    self.controls[&240],
                    LB_SETCURSEL,
                    Some(WPARAM(self.history_ids.len() - 1)),
                    None,
                );
            }
        }
        self.import_generation = s.import_generation;
        self.snapshot = Some(s);
        self.initialized = true;
        self.history_detail();
    }
    unsafe fn history_detail(&self) {
        let detail = self
            .selection(240)
            .and_then(|i| self.snapshot.as_ref()?.history.get(i))
            .map(|(d, m)| {
                format!(
                    "状态：{}\r\n渠道：{}\r\n{}\r\n{}\r\n{}",
                    status_name(d.status),
                    self.snapshot
                        .as_ref()
                        .and_then(|s| s.settings.channels.iter().find(|c| c.id == d.channel_id))
                        .map(|c| c.name.as_str())
                        .unwrap_or("已删除渠道"),
                    m.as_ref()
                        .map(|m| format!(
                            "{} · {}",
                            m.app_name,
                            m.occurred_at.with_timezone(&chrono::Local)
                        ))
                        .unwrap_or_default(),
                    m.as_ref()
                        .map(|m| format!("{}\r\n{}", m.title, m.body))
                        .unwrap_or_default(),
                    d.error.as_deref().unwrap_or("")
                )
            })
            .unwrap_or_default();
        self.set(241, &detail);
    }
}
fn kind_name(k: ChannelKind) -> &'static str {
    match k {
        ChannelKind::Feishu => "飞书",
        ChannelKind::DingTalk => "钉钉旧机器人",
        ChannelKind::WeCom => "企业微信",
    }
}
fn status_name(s: Status) -> &'static str {
    match s {
        Status::Pending => "待发送",
        Status::InFlight => "发送中",
        Status::Succeeded => "平台已接受",
        Status::RetryableFailed => "等待重试",
        Status::PermanentFailed => "失败",
        Status::Uncertain => "结果不确定",
        Status::Expired => "已过期",
        Status::Expanded => "已分段",
    }
}
unsafe extern "system" fn viewport_proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_COMMAND
        && let Ok(parent) = GetParent(h)
    {
        return SendMessageW(parent, msg, Some(w), Some(l));
    }
    let ptr = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut std::cell::RefCell<Ui>;
    if !ptr.is_null() {
        let Ok(mut ui) = (*ptr).try_borrow_mut() else {
            return DefWindowProcW(h, msg, w, l);
        };
        match msg {
            WM_COMMAND => return SendMessageW(ui.root, msg, Some(w), Some(l)),
            WM_VSCROLL => {
                let code = (w.0 & 0xffff) as u32;
                let mut info = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL,
                    ..Default::default()
                };
                let _ = GetScrollInfo(h, SB_VERT, &mut info);
                ui.scroll = match code {
                    0 => ui.scroll - 30,
                    1 => ui.scroll + 30,
                    2 => ui.scroll - 200,
                    3 => ui.scroll + 200,
                    4 | 5 => info.nTrackPos,
                    _ => ui.scroll,
                };
                ui.layout();
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                ui.scroll -= ((w.0 >> 16) as i16 as i32) / 120 * 90;
                ui.layout();
                return LRESULT(0);
            }
            _ => {}
        }
    }
    DefWindowProcW(h, msg, w, l)
}
unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_CLOSE {
        let _ = ShowWindow(h, SW_HIDE);
        return LRESULT(0);
    }
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return LRESULT(0);
    }
    if msg == WM_NCCREATE {
        let c = &*(l.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(h, GWLP_USERDATA, c.lpCreateParams as isize);
    }
    let ptr = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut std::cell::RefCell<Ui>;
    if !ptr.is_null() {
        let Ok(mut ui) = (*ptr).try_borrow_mut() else {
            return DefWindowProcW(h, msg, w, l);
        };
        if msg == ui.recreated && msg != 0 {
            ui.tray(true);
            return LRESULT(0);
        }
        match msg {
            WM_SIZE => {
                ui.layout();
                return LRESULT(0);
            }
            WM_COMMAND => {
                ui.action((w.0 & 0xffff) as i32, (w.0 >> 16) as u32);
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = ShowWindow(h, SW_HIDE);
                return LRESULT(0);
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            WM_GETMINMAXINFO => {
                let p = &mut *(l.0 as *mut MINMAXINFO);
                p.ptMinTrackSize.x = (850 * ui.dpi / 96) as i32;
                p.ptMinTrackSize.y = (620 * ui.dpi / 96) as i32;
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                ui.dpi = (w.0 & 0xffff) as u32;
                let r = &*(l.0 as *const RECT);
                let _ = SetWindowPos(
                    h,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                ui.layout();
                return LRESULT(0);
            }
            WM_TIMER => {
                let mut newest = None;
                if let Some(b) = &ui.backend {
                    while let Ok(s) = b.rx.try_recv() {
                        newest = Some(s)
                    }
                }
                if let Some(s) = newest {
                    ui.render(s)
                }
                ui.ticks += 1;
                if ui.smoke.is_some() && ui.ticks == 10 {
                    let import_keeps_edits = if let Some(mut snap) = ui.snapshot.clone() {
                        snap.import_generation += 1;
                        snap.feedback = "已导入；测试".into();
                        ui.render(snap.clone());
                        ui.set(121, "未保存的输入");
                        ui.render(snap);
                        ui.text(121) == "未保存的输入"
                    } else {
                        false
                    };
                    let _ = SendMessageW(h, WM_CLOSE, None, None);
                    let hidden = !IsWindowVisible(h).as_bool();
                    let _ = ShowWindow(h, SW_SHOW);
                    for page in 0..4 {
                        ui.page = page;
                        ui.layout();
                    }
                    ui.send(Command::Pause);
                    let report = serde_json::json!({"pages":4,"controls":ui.controls.len(),"close_hides":hidden,"tray_created":ui.tray_ok,"import_keeps_edits":import_keeps_edits});
                    if let Some(path) = &ui.smoke {
                        let _ = std::fs::write(path.join("ui-smoke.json"), report.to_string());
                    }
                    let _ = DestroyWindow(h);
                }
                return LRESULT(0);
            }
            _ if msg == WM_APP + 1 => {
                match l.0 as u32 {
                    WM_LBUTTONDBLCLK => {
                        let _ = ShowWindow(h, SW_SHOW);
                        let _ = SetForegroundWindow(h);
                    }
                    WM_RBUTTONUP => {
                        if let Ok(menu) = CreatePopupMenu() {
                            let _ = AppendMenuW(menu, MF_STRING, 301, w!("打开"));
                            let _ = AppendMenuW(menu, MF_STRING, 302, w!("暂停转发"));
                            let _ = AppendMenuW(menu, MF_STRING, 303, w!("退出"));
                            let mut p = POINT::default();
                            let _ = GetCursorPos(&mut p);
                            let _ = SetForegroundWindow(h);
                            let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, p.x, p.y, None, h, None);
                            let _ = DestroyMenu(menu);
                        }
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    DefWindowProcW(h, msg, w, l)
}
pub fn run(backend: Backend, smoke: Option<PathBuf>) -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = RoInitialize(RO_INIT_SINGLETHREADED);
        let instance = GetModuleHandleW(None).map_err(|_| "无法获取模块")?;
        let font = CreateFontW(
            -16,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            w!("Microsoft YaHei UI"),
        );
        let ui = Box::new(std::cell::RefCell::new(Ui {
            root: HWND::default(),
            viewport: HWND::default(),
            header: HWND::default(),
            footer: HWND::default(),
            nav: vec![],
            controls: HashMap::new(),
            items: vec![],
            font,
            page: 0,
            scroll: 0,
            backend: Some(backend),
            snapshot: None,
            source_ids: vec![],
            channel_id: None,
            history_ids: vec![],
            initialized: false,
            import_generation: 0,
            smoke,
            ticks: 0,
            tray_ok: false,
            recreated: RegisterWindowMessageW(w!("TaskbarCreated")),
            dpi: 96,
        }));
        let class = WNDCLASSW {
            hInstance: instance.into(),
            lpfnWndProc: Some(proc),
            lpszClassName: w!("NotifyRedirectNativeMain"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut c_void),
            ..Default::default()
        };
        RegisterClassW(&class);
        let child = WNDCLASSW {
            lpfnWndProc: Some(viewport_proc),
            lpszClassName: w!("NotifyRedirectNativeView"),
            ..class
        };
        RegisterClassW(&child);
        let ptr = &*ui as *const std::cell::RefCell<Ui>;
        let root = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("NotifyRedirectNativeMain"),
            w!("通知转发器 · Rust 原生版"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1060,
            760,
            None,
            None,
            Some(instance.into()),
            Some(ptr.cast()),
        )
        .map_err(|_| "无法创建设置窗口")?;
        let mut state = ui.borrow_mut();
        state.root = root;
        state.dpi = GetDpiForWindow(root);
        state.build()?;
        state.layout();
        state.tray(true);
        drop(state);
        SetTimer(Some(root), 1, 200, None);
        let _ = ShowWindow(root, SW_SHOW);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if !IsDialogMessageW(root, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let mut state = ui.borrow_mut();
        state.tray(false);
        if let Some(b) = state.backend.take() {
            b.shutdown();
        }
        let _ = DeleteObject(font.into());
        Ok(())
    }
}

pub struct InstanceGuard(HANDLE);
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
pub fn acquire_instance(isolated: bool) -> Result<Option<InstanceGuard>> {
    unsafe {
        let name = if isolated {
            format!("Local\\NotifyRedirect.Native.Smoke.{}", std::process::id())
        } else {
            "Local\\NotifyRedirect.Native".into()
        };
        let handle = CreateMutexW(None, false, PCWSTR(wide(&name).as_ptr()))
            .map_err(|_| "无法创建程序互斥")?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            Ok(None)
        } else {
            Ok(Some(InstanceGuard(handle)))
        }
    }
}
