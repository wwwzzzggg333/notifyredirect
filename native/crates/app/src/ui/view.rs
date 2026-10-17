use super::*;
impl Ui {
    #[allow(clippy::too_many_arguments)] // Mirrors Win32 control geometry and style arguments.
    unsafe fn control(
        &mut self,
        page: usize,
        id: i32,
        class: &str,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        extra: u32,
    ) -> Result<HWND> {
        let class = wide(class);
        let text = wide(text);
        let handle = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(extra),
            x,
            y,
            w.max(100),
            h,
            Some(self.viewport),
            Some(HMENU(id as usize as *mut c_void)),
            None,
            None,
        )
        .map_err(|_| "无法创建设置控件")?;
        SendMessageW(
            handle,
            WM_SETFONT,
            Some(WPARAM(self.font.0 as usize)),
            Some(LPARAM(1)),
        );
        self.controls.insert(id, handle);
        self.items.push(Item {
            page,
            id,
            x,
            y,
            w,
            h,
        });
        Ok(handle)
    }
    unsafe fn label(&mut self, p: usize, id: i32, text: &str, y: i32) -> Result<()> {
        self.control(p, id, "STATIC", text, 10, y, -1, 26, 0)?;
        Ok(())
    }
    unsafe fn edit(
        &mut self,
        p: usize,
        id: i32,
        y: i32,
        h: i32,
        multi: bool,
        password: bool,
    ) -> Result<()> {
        self.control(
            p,
            id,
            "EDIT",
            "",
            10,
            y,
            -1,
            h,
            WS_BORDER.0
                | WS_TABSTOP.0
                | if multi {
                    ES_MULTILINE as u32
                        | ES_AUTOVSCROLL as u32
                        | ES_WANTRETURN as u32
                        | WS_VSCROLL.0
                } else {
                    ES_AUTOHSCROLL as u32
                }
                | if password { ES_PASSWORD as u32 } else { 0 },
        )?;
        SendMessageW(
            self.controls[&id],
            EM_SETLIMITTEXT,
            Some(WPARAM(131072)),
            None,
        );
        Ok(())
    }
    unsafe fn button(
        &mut self,
        p: usize,
        id: i32,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
    ) -> Result<()> {
        self.control(p, id, "BUTTON", text, x, y, w, 34, WS_TABSTOP.0)?;
        Ok(())
    }
    unsafe fn checkbox(&mut self, p: usize, id: i32, text: &str, y: i32) -> Result<()> {
        self.control(
            p,
            id,
            "BUTTON",
            text,
            10,
            y,
            -1,
            32,
            WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32,
        )?;
        Ok(())
    }
    unsafe fn list(&mut self, p: usize, id: i32, y: i32, h: i32, multi: bool) -> Result<()> {
        self.control(
            p,
            id,
            "LISTBOX",
            "",
            10,
            y,
            -1,
            h,
            WS_BORDER.0
                | WS_VSCROLL.0
                | WS_TABSTOP.0
                | LBS_NOTIFY as u32
                | if multi { LBS_MULTIPLESEL as u32 } else { 0 },
        )?;
        Ok(())
    }
    unsafe fn combo_box(&mut self, p: usize, id: i32, y: i32, options: &[&str]) -> Result<()> {
        self.control(
            p,
            id,
            "COMBOBOX",
            "",
            10,
            y,
            -1,
            200,
            WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0,
        )?;
        for text in options {
            let t = wide(text);
            SendMessageW(
                self.controls[&id],
                CB_ADDSTRING,
                None,
                Some(LPARAM(t.as_ptr() as isize)),
            );
        }
        self.select_combo(id, 0);
        Ok(())
    }
    pub(super) unsafe fn build(&mut self) -> Result<()> {
        let instance = GetModuleHandleW(None).map_err(|_| "模块不可用")?;
        self.header = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("通知转发器 · Rust 原生版\n让电脑继续工作，让结果找到你。"),
            WS_CHILD | WS_VISIBLE,
            20,
            14,
            900,
            44,
            Some(self.root),
            None,
            None,
            None,
        )
        .map_err(|_| "无法创建标题")?;
        self.footer = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("正在初始化…"),
            WS_CHILD | WS_VISIBLE,
            20,
            650,
            900,
            40,
            Some(self.root),
            None,
            None,
            None,
        )
        .map_err(|_| "无法创建状态栏")?;
        for (i, label) in ["首页", "通知来源", "接收渠道", "规则与记录"]
            .iter()
            .enumerate()
        {
            let text = wide(label);
            let h = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                20 + i as i32 * 150,
                65,
                140,
                34,
                Some(self.root),
                Some(HMENU((i + 1) as *mut c_void)),
                None,
                None,
            )
            .map_err(|_| "无法创建导航")?;
            self.nav.push(h);
        }
        self.viewport = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            w!("NotifyRedirectNativeView"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_VSCROLL,
            20,
            112,
            960,
            500,
            Some(self.root),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|_| "无法创建页面")?;
        SetWindowLongPtrW(
            self.viewport,
            GWLP_USERDATA,
            GetWindowLongPtrW(self.root, GWLP_USERDATA),
        );
        for h in [self.header, self.footer]
            .into_iter()
            .chain(self.nav.iter().copied())
        {
            SendMessageW(h, WM_SETFONT, Some(WPARAM(self.font.0 as usize)), None);
        }
        self.control(0, 10, "STATIC", "", 10, 10, -1, 65, 0)?;
        self.button(0, 11, "授权读取通知", 10, 85, 170)?;
        self.button(0, 12, "刷新通知", 190, 85, 130)?;
        self.button(0, 13, "暂停转发", 330, 85, 130)?;
        self.button(0, 14, "退出程序", 470, 85, 130)?;
        self.control(0,15,"STATIC","首次使用：授权 → 勾选来源 → 配置渠道并测试 → 保存转发时间。\n锁屏后保持电脑唤醒和联网；关闭窗口会继续在托盘运行。",10,135,-1,65,0)?;
        self.label(0, 17, "最近捕获的消息", 210)?;
        self.edit(0, 16, 240, 190, true, false)?;
        SendMessageW(self.controls[&16], EM_SETREADONLY, Some(WPARAM(1)), None);
        self.checkbox(0, 18, "登录 Windows 后启动（需系统允许）", 450)?;
        self.control(
            1,
            100,
            "STATIC",
            "请选择需要转发的来源；默认不转发任何应用。应用发出通知后才会出现在这里。",
            10,
            10,
            -1,
            50,
            0,
        )?;
        self.list(1, 110, 65, 230, true)?;
        self.label(1, 120, "包含关键词（每行一个；留空不限）", 310)?;
        self.edit(1, 121, 340, 70, true, false)?;
        self.label(1, 122, "排除关键词（每行一个；排除优先）", 420)?;
        self.edit(1, 123, 450, 70, true, false)?;
        self.button(1, 124, "保存来源与过滤规则", 10, 540, 220)?;
        self.label(2, 129, "接收渠道（凭据由 Windows 当前用户保护）", 10)?;
        self.list(2, 130, 45, 120, false)?;
        self.label(2, 131, "渠道名称", 180)?;
        self.edit(2, 132, 210, 30, false, false)?;
        self.label(2, 133, "平台", 250)?;
        self.combo_box(
            2,
            134,
            280,
            &[
                "飞书群机器人",
                "钉钉旧群机器人（已有 Webhook）",
                "企业微信机器人",
            ],
        )?;
        self.label(2, 135, "Webhook 地址（编辑已有渠道时留空保留）", 320)?;
        self.edit(2, 136, 350, 30, false, true)?;
        self.label(2, 137, "签名密钥（飞书 / 钉钉；留空保留）", 390)?;
        self.edit(2, 138, 420, 30, false, true)?;
        self.checkbox(2, 139, "启用渠道", 460)?;
        self.check(139, true);
        self.button(2, 140, "保存渠道", 10, 510, 130)?;
        self.button(2, 141, "新建", 150, 510, 100)?;
        self.button(2, 142, "发送测试消息", 260, 510, 170)?;
        self.button(2, 143, "删除所选", 440, 510, 140)?;
        self.control(2,144,"STATIC","测试会立即发送，不受转发时段限制。请在手机确认收到；平台可能要求关键词或群权限。\n钉钉仅支持已有旧 Webhook，新机器人方案暂未接入。",10,565,-1,65,0)?;
        self.label(3, 200, "转发模式", 10)?;
        self.combo_box(3, 201, 40, &["全天转发", "按时间段转发", "暂停转发"])?;
        self.label(
            3,
            210,
            "每行：星期 | 开始 | 结束；1–7 为周一至周日，全天写 all",
            80,
        )?;
        self.edit(3, 211, 110, 85, true, false)?;
        self.control(3,212,"STATIC","例：1,2,3,4,5 | 18:30 | 09:00     另起一行：6,7 | all\n跨午夜按开始日计算；时段外消息不积攒补发。",10,205,-1,50,0)?;
        self.label(3, 220, "电脑名称", 270)?;
        self.edit(3, 221, 300, 30, false, false)?;
        self.button(3, 222, "保存时间规则", 10, 345, 170)?;
        self.button(3, 223, "导出配置", 190, 345, 140)?;
        self.button(3, 224, "导入配置", 340, 345, 140)?;
        self.checkbox(3, 231, "启用 Agent 接入口（127.0.0.1:17842）", 395)?;
        self.label(3, 239, "发送记录", 445)?;
        self.list(3, 240, 480, 150, false)?;
        self.edit(3, 241, 640, 135, true, false)?;
        SendMessageW(self.controls[&241], EM_SETREADONLY, Some(WPARAM(1)), None);
        self.button(3, 242, "刷新记录", 10, 790, 130)?;
        self.button(3, 243, "重发所选失败项", 150, 790, 200)?;
        self.button(3, 244, "清空消息历史", 360, 790, 180)?;
        Ok(())
    }
}
