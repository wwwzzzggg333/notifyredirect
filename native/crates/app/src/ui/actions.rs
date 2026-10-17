use super::*;
use windows::Win32::UI::Controls::Dialogs::*;
unsafe fn file_dialog(root: HWND, save: bool) -> Option<PathBuf> {
    let mut file = [0u16; 32768];
    let filter = wide("JSON 配置 (*.json)\0*.json\0所有文件\0*.*\0");
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: root,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: windows::core::PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrDefExt: w!("json"),
        Flags: OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            },
        ..Default::default()
    };
    if if save {
        GetSaveFileNameW(&mut dialog)
    } else {
        GetOpenFileNameW(&mut dialog)
    }
    .as_bool()
    {
        Some(PathBuf::from(String::from_utf16_lossy(
            &file[..file.iter().position(|c| *c == 0).unwrap_or(0)],
        )))
    } else {
        None
    }
}
impl Ui {
    pub(super) unsafe fn action(&mut self, id: i32, code: u32) {
        if (1..=4).contains(&id) {
            self.page = (id - 1) as usize;
            self.scroll = 0;
            self.layout();
            return;
        }
        if id == 301 {
            let _ = ShowWindow(self.root, SW_SHOW);
            let _ = SetForegroundWindow(self.root);
            return;
        }
        if id == 303 || id == 14 {
            let _ = DestroyWindow(self.root);
            return;
        }
        if id == 13 || id == 302 {
            self.select_combo(201, 2);
            self.send(Command::Pause);
            return;
        }
        if id == 11 {
            let tx = self.backend.as_ref().unwrap().tx.clone();
            if let Err(e) = request_access(move |a| {
                let _ = tx.send(Command::Access(a));
            }) {
                self.report(&e)
            }
            return;
        }
        if id == 12 || id == 242 {
            self.send(Command::Refresh);
            return;
        }
        if id == 18 {
            self.send(Command::Startup(self.checked(18)));
            return;
        }
        if id == 231 {
            self.send(Command::Api(self.checked(231)));
            return;
        }
        if id == 240 && code == LBN_SELCHANGE {
            self.history_detail();
            return;
        }
        if id == 130 && code == LBN_SELCHANGE {
            if let Some(c) = self
                .selection(130)
                .and_then(|i| self.snapshot.as_ref()?.settings.channels.get(i))
                .cloned()
            {
                self.channel_id = Some(c.id);
                self.set(132, &c.name);
                self.select_combo(
                    134,
                    match c.kind {
                        ChannelKind::Feishu => 0,
                        ChannelKind::DingTalk => 1,
                        ChannelKind::WeCom => 2,
                    },
                );
                self.set(136, "");
                self.set(138, "");
                self.check(139, c.enabled);
            }
            return;
        }
        if code != BN_CLICKED {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let mut settings = snapshot.settings.clone();
        let result = (|| -> Result<()> {
            match id {
                124 => {
                    settings.rules.app_ids = self
                        .source_ids
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| {
                            SendMessageW(self.controls[&110], LB_GETSEL, Some(WPARAM(*i)), None).0
                                > 0
                        })
                        .map(|(_, id)| id.clone())
                        .collect();
                    settings.rules.include_keywords = self
                        .text(121)
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect();
                    settings.rules.exclude_keywords = self
                        .text(123)
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect();
                    self.send(Command::Sources(
                        settings.rules.app_ids,
                        settings.rules.include_keywords,
                        settings.rules.exclude_keywords,
                    ));
                }
                140 => {
                    let c = ChannelConfig {
                        id: self
                            .channel_id
                            .clone()
                            .unwrap_or_else(notifyredirect_core::id),
                        name: self.text(132).trim().into(),
                        kind: match self.combo(134) {
                            1 => ChannelKind::DingTalk,
                            2 => ChannelKind::WeCom,
                            _ => ChannelKind::Feishu,
                        },
                        enabled: self.checked(139),
                    };
                    if c.name.is_empty() {
                        return Err("请填写渠道名称".into());
                    }
                    let url = self.text(136);
                    let key = self.text(138);
                    self.channel_id = Some(c.id.clone());
                    self.send(Command::Channel(
                        c,
                        if url.trim().is_empty() {
                            None
                        } else {
                            Some(url.trim().into())
                        },
                        if key.is_empty() { None } else { Some(key) },
                    ));
                    self.set(136, "");
                    self.set(138, "");
                }
                141 => {
                    self.channel_id = None;
                    self.set(132, "");
                    self.set(136, "");
                    self.set(138, "");
                    self.select_combo(134, 0);
                    self.check(139, true);
                    SendMessageW(
                        self.controls[&130],
                        LB_SETCURSEL,
                        Some(WPARAM(usize::MAX)),
                        None,
                    );
                }
                142 => {
                    let id = self.channel_id.clone().ok_or("请先保存并选择渠道")?;
                    self.send(Command::Test(id));
                }
                143 => {
                    if let Some(id) = self.channel_id.take() {
                        self.send(Command::DeleteChannel(id));
                        self.set(132, "");
                    }
                }
                222 => {
                    settings.rules.mode = match self.combo(201) {
                        0 => Mode::Always,
                        1 => Mode::Scheduled,
                        _ => Mode::Paused,
                    };
                    settings.rules.windows = parse_windows(&self.text(211))?;
                    settings.computer_name = self.text(221).trim().into();
                    settings.validate()?;
                    self.send(Command::Rules(
                        settings.rules.mode,
                        settings.rules.windows,
                        settings.computer_name,
                    ));
                }
                223 => {
                    if let Some(p) = file_dialog(self.root, true) {
                        self.send(Command::Export(p));
                    }
                }
                224 => {
                    if let Some(p) = file_dialog(self.root, false) {
                        self.send(Command::Import(p));
                    }
                }
                243 => {
                    if let Some(i) = self.selection(240)
                        && MessageBoxW(
                            Some(self.root),
                            w!("重发可能重复送达，请先检查手机。继续？"),
                            w!("重发消息"),
                            MB_YESNO | MB_ICONQUESTION,
                        ) == IDYES
                        && let Some(id) = self.history_ids.get(i)
                    {
                        self.send(Command::Retry(id.clone()));
                    }
                }
                244 if MessageBoxW(
                    Some(self.root),
                    w!("清空所有消息历史并取消待发队列？"),
                    w!("清空历史"),
                    MB_YESNO | MB_ICONQUESTION,
                ) == IDYES =>
                {
                    self.send(Command::Clear);
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.report(&e)
        }
    }
}
