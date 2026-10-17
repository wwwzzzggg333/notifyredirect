use crate::{api, worker::*};
use notifyredirect_core::*;
use notifyredirect_platform::*;
use notifyredirect_storage::Store;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
pub enum Command {
    Sources(Vec<String>, Vec<String>, Vec<String>),
    Rules(Mode, Vec<ScheduleWindow>, String),
    Channel(ChannelConfig, Option<String>, Option<String>),
    DeleteChannel(String),
    Pause,
    Ingest(Message, SyncSender<Result<()>>),
    Batch(Access, Vec<SourceSnapshot>),
    Feedback(String),
    Access(Access),
    Startup(bool),
    Refresh,
    Api(bool),
    Test(String),
    Retry(String),
    Clear,
    Import(PathBuf),
    Export(PathBuf),
    Done(DeliveryRecord, Outcome),
    TestDone(Outcome),
    Exit,
}
#[derive(Clone)]
pub struct Snapshot {
    pub import_generation: u64,
    pub settings: Settings,
    pub sources: BTreeMap<String, String>,
    pub history: Vec<(DeliveryRecord, Option<Message>)>,
    pub preview: String,
    pub feedback: String,
    pub access: Access,
    pub startup: bool,
}
pub struct Backend {
    pub tx: Sender<Command>,
    pub rx: Receiver<Snapshot>,
    cancel: Arc<AtomicBool>,
    done: Receiver<()>,
    join: Option<thread::JoinHandle<()>>,
}
impl Backend {
    pub fn start(path: PathBuf, isolated: bool) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let (updates, snapshots) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let (done, finished) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let sender = tx.clone();
        let join = thread::spawn(move || {
            initialize_runtime();
            let result = (|| -> Result<()> {
                let mut db = Store::open(&path)?;
                let mut s = db.settings()?;
                let mut sources: BTreeMap<String, String> = db
                    .get("sources")?
                    .map(|j| serde_json::from_str(&j).map_err(|_| "来源记录无效".to_string()))
                    .transpose()?
                    .unwrap_or_default();
                let mut feedback =
                    "首次使用：授权通知 → 选择来源 → 添加渠道 → 保存时间。".to_string();
                let mut preview = String::new();
                let mut access = Access::Unavailable;
                let mut startup = if isolated {
                    false
                } else {
                    startup_enabled().unwrap_or(false)
                };
                let mut active = false;
                let mut import_generation = 0u64;
                let mut pending_completion: Option<(DeliveryRecord, Outcome, UtcTime)> = None;
                let mut api_handle = None;
                let (wake, signal) = mpsc::channel();
                if !isolated {
                    let tx = sender.clone();
                    let stop = stop.clone();
                    let w = wake.clone();
                    thread::spawn(move || {
                        initialize_runtime();
                        let mut listener = WindowsNotifications::new().ok();
                        while !stop.load(Ordering::SeqCst) {
                            if let Some(l) = listener.as_mut() {
                                l.subscribe(w.clone());
                                match read_gated(l) {
                                    Ok((a, items)) => {
                                        let _ = tx.send(Command::Batch(a, items));
                                    }
                                    Err(e) => {
                                        let _ = tx.send(Command::Feedback(e));
                                    }
                                }
                            } else {
                                listener = WindowsNotifications::new().ok();
                                let _ = tx.send(Command::Access(Access::Unavailable));
                            }
                            let _ = signal.recv_timeout(Duration::from_secs(5));
                            while signal.try_recv().is_ok() {}
                        }
                    });
                }
                if s.api_enabled && !isolated {
                    match open_api(&mut db, sender.clone()) {
                        Ok(h) => api_handle = Some(h),
                        Err(e) => {
                            s.api_enabled = false;
                            db.save_settings(&s)?;
                            feedback = e;
                        }
                    }
                }
                let _ = ready.send(Ok(()));
                db.purge(chrono::Utc::now())?;
                let mut last_purge = Instant::now();
                let mut last_snapshot = Instant::now();
                let mut dirty = true;
                loop {
                    let command = rx.recv_timeout(Duration::from_millis(100));
                    let mut exiting = false;
                    if let Ok(command) = command {
                        dirty = true;
                        let action = (|| -> Result<()> {
                            match command {
                                Command::Exit => {
                                    exiting = true;
                                }
                                Command::Sources(ids, include, exclude) => {
                                    let mut next = s.clone();
                                    next.rules.app_ids = ids;
                                    next.rules.include_keywords = include;
                                    next.rules.exclude_keywords = exclude;
                                    db.save_settings(&next)?;
                                    s = next;
                                    feedback = "来源与关键词已保存".into();
                                }
                                Command::Rules(mode, windows, computer) => {
                                    let mut next = s.clone();
                                    next.rules.mode = mode;
                                    next.rules.windows = windows;
                                    next.computer_name = computer;
                                    db.save_settings(&next)?;
                                    s = next;
                                    feedback = "时间规则已保存".into();
                                }
                                Command::Pause => {
                                    s.rules.mode = Mode::Paused;
                                    db.save_settings(&s)?;
                                    feedback = "已暂停自动转发".into();
                                }
                                Command::Channel(c, url, key) => {
                                    let existing = db
                                        .get(&format!("secret:{}", c.id))?
                                        .and_then(|v| unprotect(&v).ok())
                                        .and_then(|v| {
                                            serde_json::from_str::<ChannelSecret>(&v).ok()
                                        });
                                    let secret = ChannelSecret {
                                        webhook: url
                                            .or_else(|| {
                                                existing.as_ref().map(|x| x.webhook.clone())
                                            })
                                            .unwrap_or_default(),
                                        signing_key: key
                                            .or_else(|| existing.map(|x| x.signing_key))
                                            .unwrap_or_default(),
                                    };
                                    let _ = build_request(
                                        c.kind,
                                        &secret,
                                        "验证配置",
                                        chrono::Utc::now(),
                                    )?;
                                    let encrypted = protect(
                                        &serde_json::to_string(&secret)
                                            .map_err(|_| "凭据编码失败")?,
                                    )?;
                                    let mut next = s.clone();
                                    if let Some(old) =
                                        next.channels.iter_mut().find(|x| x.id == c.id)
                                    {
                                        *old = c.clone()
                                    } else {
                                        next.channels.push(c.clone())
                                    }
                                    next.validate()?;
                                    db.save_channel(&next, &c.id, &encrypted)?;
                                    s = next;
                                    feedback = "渠道已保存，请发送测试消息并在手机确认".into();
                                }
                                Command::DeleteChannel(id) => {
                                    let mut next = s.clone();
                                    next.channels.retain(|c| c.id != id);
                                    db.save_settings(&next)?;
                                    db.set(&format!("secret:{id}"), "")?;
                                    s = next;
                                    feedback = "渠道已删除".into();
                                }
                                Command::Ingest(m, receipt) => {
                                    let r = (|| -> Result<()> {
                                        sources.insert(m.app_id.clone(), m.app_name.clone());
                                        db.set(
                                            "sources",
                                            &serde_json::to_string(&sources)
                                                .map_err(|_| "来源编码失败")?,
                                        )?;
                                        db.ingest(
                                            &m,
                                            false,
                                            &s,
                                            should_enqueue(&m, &s, chrono::Utc::now()),
                                            chrono::Utc::now(),
                                        )?;
                                        preview =
                                            format!("{} · {}\n{}", m.app_name, m.title, m.body);
                                        Ok(())
                                    })();
                                    let _ = receipt.send(r.clone());
                                    r?;
                                }
                                Command::Batch(a, items) => {
                                    access = a;
                                    if a == Access::Allowed {
                                        let baseline = !db.has_baseline()?;
                                        for item in items {
                                            sources
                                                .insert(item.app_id.clone(), item.app_name.clone());
                                            if let Some(m) = map(item) {
                                                let o = db.ingest(
                                                    &m,
                                                    baseline,
                                                    &s,
                                                    should_enqueue(&m, &s, chrono::Utc::now()),
                                                    chrono::Utc::now(),
                                                )?;
                                                if matches!(
                                                    o,
                                                    Observation::New | Observation::Changed
                                                ) {
                                                    preview = format!(
                                                        "{} · {}\n{}",
                                                        m.app_name, m.title, m.body
                                                    );
                                                }
                                            }
                                        }
                                        db.set(
                                            "sources",
                                            &serde_json::to_string(&sources)
                                                .map_err(|_| "来源编码失败")?,
                                        )?;
                                        if baseline {
                                            db.set_baseline()?;
                                            feedback = "已建立当前通知基线，不补发旧通知".into();
                                        }
                                    }
                                }
                                Command::Access(a) => {
                                    access = a;
                                    if a == Access::Allowed {
                                        let _ = wake.send(());
                                    }
                                }
                                Command::Feedback(f) => feedback = f,
                                Command::Refresh => {
                                    let _ = wake.send(());
                                }
                                Command::Startup(enabled) => {
                                    if isolated {
                                        return Err("测试模式不改变启动项".into());
                                    }
                                    startup = set_startup(enabled)?;
                                    feedback = if enabled && !startup {
                                        "Windows 未允许登录启动"
                                    } else {
                                        "启动项已更新"
                                    }
                                    .into();
                                }
                                Command::Api(enabled) => {
                                    if enabled && isolated {
                                        return Err("测试模式不启用生产入口".into());
                                    }
                                    if enabled && api_handle.is_none() {
                                        api_handle = Some(open_api(&mut db, sender.clone())?);
                                    } else if !enabled {
                                        api_handle = None;
                                    }
                                    s.api_enabled = enabled;
                                    db.save_settings(&s)?;
                                    feedback = if enabled {
                                        "Agent 入口已开启：127.0.0.1:17842"
                                    } else {
                                        "Agent 入口已关闭"
                                    }
                                    .into();
                                }
                                Command::Test(id) => {
                                    if isolated {
                                        return Err("测试模式禁止外发".into());
                                    }
                                    let c = s
                                        .channels
                                        .iter()
                                        .find(|c| c.id == id)
                                        .ok_or("请选择渠道")?;
                                    let secret = load_secret(&db, &id)?;
                                    let request = build_request(
                                        c.kind,
                                        &secret,
                                        "[测试消息] NotifyRedirect Native 渠道测试，请在手机确认收到。",
                                        chrono::Utc::now(),
                                    )?;
                                    let tx = sender.clone();
                                    thread::spawn(move || {
                                        let outcome = classify(
                                            WinHttpTransport.send(&request),
                                            chrono::Utc::now(),
                                        );
                                        let _ = tx.send(Command::TestDone(outcome));
                                    });
                                    feedback = "正在发送测试消息…".into();
                                }
                                Command::TestDone(o) => {
                                    feedback = if o.status == Status::Succeeded {
                                        "平台已接受测试消息，请在手机确认".into()
                                    } else {
                                        o.error.unwrap_or("测试失败".into())
                                    }
                                }
                                Command::Retry(id) => {
                                    db.retry(&id, chrono::Utc::now())?;
                                    feedback =
                                        "已安排重发，仍遵守时间规则；可能重复，请检查手机".into();
                                }
                                Command::Clear => {
                                    db.clear_history()?;
                                    preview.clear();
                                    feedback = "历史及待发队列已清空".into();
                                }
                                Command::Import(path) => {
                                    let text = std::fs::read_to_string(path)
                                        .map_err(|_| "无法读取配置文件")?;
                                    let new = notifyredirect_storage::import(&text)?;
                                    db.save_settings(&new)?;
                                    api_handle = None;
                                    s = new;
                                    import_generation += 1;
                                    feedback = "已导入；渠道和入口停用，请重新配置凭据".into();
                                }
                                Command::Export(path) => {
                                    std::fs::write(path, notifyredirect_storage::export(&s)?)
                                        .map_err(|_| "无法保存导出文件")?;
                                    feedback = "已导出配置，不含凭据和消息历史".into();
                                }
                                Command::Done(part, outcome) => {
                                    complete(
                                        &mut db,
                                        part,
                                        outcome,
                                        chrono::Utc::now(),
                                        &mut active,
                                        &mut pending_completion,
                                    )?;
                                }
                            }
                            Ok(())
                        })();
                        if let Err(e) = action {
                            feedback = e;
                        }
                    }
                    if exiting {
                        break;
                    }
                    if last_purge.elapsed() >= Duration::from_secs(86400) {
                        if let Err(e) = db.purge(chrono::Utc::now()) {
                            feedback = e
                        }
                        last_purge = Instant::now();
                    }
                    if let Some((part, outcome, at)) = &pending_completion
                        && apply_outcome(&mut db, part, outcome.clone(), *at).is_ok()
                    {
                        pending_completion = None;
                    }
                    if !active && pending_completion.is_none() && !isolated {
                        match next_job(&mut db, &s, chrono::Utc::now()) {
                            Ok(Some(job)) => {
                                let request =
                                    load_secret(&db, &job.channel.id).and_then(|secret| {
                                        build_request(
                                            job.channel.kind,
                                            &secret,
                                            job.part.body.as_deref().unwrap_or(""),
                                            chrono::Utc::now(),
                                        )
                                    });
                                match request {
                                    Ok(request) => {
                                        active = true;
                                        let tx = sender.clone();
                                        thread::spawn(move || {
                                            let outcome = classify(
                                                WinHttpTransport.send(&request),
                                                chrono::Utc::now(),
                                            );
                                            let _ = tx.send(Command::Done(job.part, outcome));
                                        });
                                    }
                                    Err(e) => {
                                        apply_outcome(
                                            &mut db,
                                            &job.part,
                                            Outcome::new(Status::PermanentFailed, &e),
                                            chrono::Utc::now(),
                                        )?;
                                    }
                                }
                                dirty = true;
                            }
                            Ok(None) => {}
                            Err(e) => feedback = e,
                        }
                    }
                    if dirty || last_snapshot.elapsed() >= Duration::from_secs(2) {
                        let history = db
                            .history()?
                            .into_iter()
                            .map(|d| {
                                let m = db.message(&d.message_id).ok().flatten();
                                (d, m)
                            })
                            .collect();
                        let _ = updates.send(Snapshot {
                            import_generation,
                            settings: s.clone(),
                            sources: sources.clone(),
                            history,
                            preview: preview.clone(),
                            feedback: feedback.clone(),
                            access,
                            startup,
                        });
                        last_snapshot = Instant::now();
                        dirty = false;
                    }
                }
                drop(api_handle);
                Ok(())
            })();
            if let Err(e) = result {
                let _ = ready.send(Err(e.clone()));
                let _ = updates.send(Snapshot {
                    import_generation: 0,
                    settings: Settings::default(),
                    sources: BTreeMap::new(),
                    history: vec![],
                    preview: String::new(),
                    feedback: e,
                    access: Access::Unavailable,
                    startup: false,
                });
            }
            stop.store(true, Ordering::SeqCst);
            let _ = done.send(());
        });
        started
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "后台初始化超时")??;
        Ok(Self {
            tx,
            rx: snapshots,
            cancel,
            done: finished,
            join: Some(join),
        })
    }
    pub fn shutdown(mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Command::Exit);
        if self.done.recv_timeout(Duration::from_secs(5)).is_ok()
            && let Some(j) = self.join.take()
        {
            let _ = j.join();
        }
    }
}
fn load_secret(db: &Store, id: &str) -> Result<ChannelSecret> {
    let secret = db
        .get(&format!("secret:{id}"))?
        .ok_or("请重新配置渠道凭据")?;
    serde_json::from_str(&unprotect(&secret)?).map_err(|_| "凭据格式无效".into())
}
fn open_api(db: &mut Store, tx: Sender<Command>) -> Result<api::ApiHandle> {
    let token = if let Some(t) = db.get("api-token")? {
        unprotect(&t)?
    } else {
        let token = format!("{}{}", id(), id());
        db.set("api-token", &protect(&token)?)?;
        token
    };
    api::start(
        17842,
        token,
        Arc::new(move |m| {
            let (receipt, response) = mpsc::sync_channel(1);
            tx.send(Command::Ingest(m, receipt))
                .map_err(|_| "后台不可用")?;
            response
                .recv_timeout(Duration::from_secs(2))
                .map_err(|_| "后台接收超时")?
        }),
    )
}

fn complete(
    db: &mut Store,
    part: DeliveryRecord,
    outcome: Outcome,
    at: UtcTime,
    active: &mut bool,
    pending: &mut Option<(DeliveryRecord, Outcome, UtcTime)>,
) -> Result<()> {
    *active = false;
    if let Err(e) = apply_outcome(db, &part, outcome.clone(), at) {
        *pending = Some((part, outcome, at));
        return Err(e);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_write_failure_releases_slot_and_retains_result() {
        let mut db = Store::open_memory().unwrap();
        let now = chrono::Utc::now();
        let mut s = Settings::default();
        s.channels.push(ChannelConfig {
            id: "c".into(),
            kind: ChannelKind::Feishu,
            name: "C".into(),
            enabled: true,
        });
        let m = Message::new("k", "a", "A", "T", "B", now);
        db.ingest(&m, false, &s, true, now).unwrap();
        let mut part = db.history().unwrap().pop().unwrap();
        part.status = Status::InFlight;
        db.mark(&part).unwrap();
        db.execute("CREATE TRIGGER fail BEFORE UPDATE ON deliveries BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        let mut active = true;
        let mut pending = None;
        assert!(
            complete(
                &mut db,
                part,
                Outcome::new(Status::Succeeded, ""),
                now,
                &mut active,
                &mut pending
            )
            .is_err()
        );
        assert!(!active);
        assert!(pending.is_some());
        assert!(db.due(now).unwrap().is_empty());
        db.execute("DROP TRIGGER fail").unwrap();
        let (part, out, at) = pending.take().unwrap();
        apply_outcome(&mut db, &part, out, at).unwrap();
        assert_eq!(db.history().unwrap()[0].status, Status::Succeeded);
    }
}
