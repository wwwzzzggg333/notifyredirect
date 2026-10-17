use chrono::{Duration, Utc};
use notifyredirect_core::*;
use notifyredirect_storage::*;
fn setup() -> (Store, Settings, Message) {
    let now = Utc::now();
    let s = Store::open_memory().unwrap();
    let mut c = Settings::default();
    c.rules.mode = Mode::Always;
    c.rules.app_ids.push("app".into());
    c.channels.push(ChannelConfig {
        id: "c".into(),
        kind: ChannelKind::Feishu,
        name: "test".into(),
        enabled: true,
    });
    (
        s,
        c,
        Message::new("key", "app", "App", "title", "body", now),
    )
}
#[test]
fn deduplicates_and_updates_only_unattempted_messages() {
    let (mut s, c, m) = setup();
    assert_eq!(
        s.ingest(&m, false, &c, true, m.occurred_at).unwrap(),
        Observation::New
    );
    assert_eq!(
        s.ingest(&m, false, &c, true, m.occurred_at).unwrap(),
        Observation::Duplicate
    );
    let mut changed = m.clone();
    changed.body = "new".into();
    changed.id = id();
    assert_eq!(
        s.ingest(&changed, false, &c, true, m.occurred_at).unwrap(),
        Observation::Changed
    );
    assert_eq!(s.history().unwrap().len(), 1);
    let d = s.history().unwrap().pop().unwrap();
    assert_eq!(s.message(&d.message_id).unwrap().unwrap().body, "new");
    let mut d = d;
    d.status = Status::InFlight;
    s.mark(&d).unwrap();
    changed.body = "third".into();
    changed.id = id();
    s.ingest(&changed, false, &c, true, m.occurred_at).unwrap();
    assert_eq!(s.history().unwrap().len(), 2);
}
#[test]
fn failed_ingress_rolls_back_ledger() {
    let (mut s, c, m) = setup();
    s.execute(
        "CREATE TRIGGER fail BEFORE INSERT ON deliveries BEGIN SELECT RAISE(ABORT,'test'); END;",
    )
    .unwrap();
    assert!(s.ingest(&m, false, &c, true, m.occurred_at).is_err());
    s.execute("DROP TRIGGER fail").unwrap();
    assert_eq!(
        s.ingest(&m, false, &c, true, m.occurred_at).unwrap(),
        Observation::New
    );
}
#[test]
fn retention_and_clear_cancel_queue() {
    let (mut s, c, m) = setup();
    s.ingest(&m, false, &c, true, m.occurred_at).unwrap();
    s.purge(m.occurred_at + Duration::days(7)).unwrap();
    assert_eq!(s.message(&m.id).unwrap().unwrap().body, "");
    assert_eq!(s.history().unwrap()[0].status, Status::Expired);
    s.purge(m.occurred_at + Duration::days(30)).unwrap();
    assert!(s.history().unwrap().is_empty());
    s.ingest(
        &Message {
            source_key: "k2".into(),
            id: id(),
            ..m.clone()
        },
        false,
        &c,
        true,
        m.occurred_at,
    )
    .unwrap();
    s.clear_history().unwrap();
    assert!(s.due(Utc::now()).unwrap().is_empty());
}
#[test]
fn reopening_recovers_inflight_but_readonly_does_not() {
    let path = std::env::temp_dir().join(format!("notify-native-{}.db", id()));
    {
        let mut s = Store::open(&path).unwrap();
        let (_, c, m) = setup();
        s.ingest(&m, false, &c, true, m.occurred_at).unwrap();
        let mut d = s.history().unwrap().pop().unwrap();
        d.status = Status::InFlight;
        s.mark(&d).unwrap();
    }
    {
        let s = Store::read_only(&path).unwrap();
        assert_eq!(s.history().unwrap()[0].status, Status::InFlight);
    }
    {
        let s = Store::open(&path).unwrap();
        assert_eq!(s.history().unwrap()[0].status, Status::Uncertain);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn manual_retry_preserves_attempt_evidence() {
    let (mut db, c, m) = setup();
    db.ingest(&m, false, &c, true, m.occurred_at).unwrap();
    let parent = db.history().unwrap().pop().unwrap();
    db.expand(&parent, &["old".into()]).unwrap();
    let mut d = db.history().unwrap().pop().unwrap();
    d.status = Status::InFlight;
    db.mark(&d).unwrap();
    d.status = Status::Uncertain;
    db.mark(&d).unwrap();
    db.retry(&d.id, m.occurred_at).unwrap();
    let changed = Message {
        id: id(),
        body: "new".into(),
        ..m.clone()
    };
    db.ingest(&changed, false, &c, true, m.occurred_at).unwrap();
    assert_eq!(db.history().unwrap().len(), 2);
    assert_eq!(db.message(&m.id).unwrap().unwrap().body, "body");
}
#[test]
fn channel_secret_and_settings_commit_together() {
    let (mut db, s, _) = setup();
    db.save_settings(&s).unwrap();
    db.set("secret:c", "old").unwrap();
    db.execute("CREATE TRIGGER fail BEFORE UPDATE ON config WHEN NEW.key='settings' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    let mut changed = s.clone();
    changed.channels[0].enabled = false;
    assert!(db.save_channel(&changed, "c", "new").is_err());
    assert_eq!(db.get("secret:c").unwrap().unwrap(), "old");
    assert!(db.settings().unwrap().channels[0].enabled);
}

#[test]
fn new_export_import_disables_channels_and_changes_ids() {
    let (_, s, _) = setup();
    let text = export(&s).unwrap();
    let imported = import(&text).unwrap();
    assert!(!imported.api_enabled);
    assert!(!imported.channels[0].enabled);
    assert_ne!(imported.channels[0].id, "c");
}
#[test]
fn higher_schema_is_refused_without_modifying_file() {
    let path = std::env::temp_dir().join(format!("schema-{}.db", id()));
    {
        let mut db = Store::open(&path).unwrap();
        db.execute("PRAGMA user_version=999;").unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(path).unwrap();
}
