use notifyredirect_app::*;
use notifyredirect_core::*;
use std::time::Duration;
#[test]
fn persistence_receipts_and_pause_are_serialized() {
    let path = std::env::temp_dir()
        .join(format!("native-coord-{}", id()))
        .join("notifyredirect.db");
    let b = Backend::start(path.clone(), true).unwrap();
    let mut s = Settings::default();
    s.rules.mode = Mode::Always;
    b.tx.send(Command::Rules(
        s.rules.mode,
        s.rules.windows,
        s.computer_name,
    ))
    .unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    b.tx.send(Command::Ingest(
        Message::new("k", "agent:a", "A", "T", "B", chrono::Utc::now()),
        tx,
    ))
    .unwrap();
    rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
    b.tx.send(Command::Pause).unwrap();
    b.shutdown();
    let s = notifyredirect_storage::Store::read_only(&path).unwrap();
    assert_eq!(s.settings().unwrap().rules.mode, Mode::Paused);
    assert!(s.get("sources").unwrap().unwrap().contains("agent:a"));
    drop(s);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn saving_sources_does_not_restore_stale_rules() {
    let path = std::env::temp_dir()
        .join(format!("coord-rules-{}", id()))
        .join("notifyredirect.db");
    let b = Backend::start(path.clone(), true).unwrap();
    b.tx.send(Command::Rules(Mode::Always, vec![], "PC".into()))
        .unwrap();
    b.tx.send(Command::Pause).unwrap();
    b.tx.send(Command::Sources(vec!["a".into()], vec![], vec![]))
        .unwrap();
    b.shutdown();
    let db = notifyredirect_storage::Store::read_only(&path).unwrap();
    assert_eq!(db.settings().unwrap().rules.mode, Mode::Paused);
    assert_eq!(db.settings().unwrap().rules.app_ids, vec!["a"]);
    drop(db);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
