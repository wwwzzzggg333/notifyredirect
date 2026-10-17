use chrono::Utc;
use notifyredirect_app::*;
use notifyredirect_core::*;
use notifyredirect_storage::Store;
#[test]
fn pause_expiry_and_uncertainty_never_resend() {
    let now = Utc::now();
    let mut db = Store::open_memory().unwrap();
    let mut s = Settings::default();
    s.channels.push(ChannelConfig {
        id: "c".into(),
        kind: ChannelKind::Feishu,
        name: "test".into(),
        enabled: true,
    });
    s.rules.app_ids.push("app".into());
    s.rules.mode = Mode::Always;
    let m = Message::new("k", "app", "App", "t", "b", now);
    assert!(should_enqueue(&m, &s, now));
    db.ingest(&m, false, &s, true, now).unwrap();
    s.rules.mode = Mode::Paused;
    assert!(next_job(&mut db, &s, now).unwrap().is_none());
    s.rules.mode = Mode::Always;
    assert!(next_job(&mut db, &s, now).unwrap().is_none());
    let job = next_job(&mut db, &s, now).unwrap().unwrap();
    apply_outcome(
        &mut db,
        &job.part,
        Outcome::new(Status::Uncertain, "test"),
        now,
    )
    .unwrap();
    assert!(next_job(&mut db, &s, now).unwrap().is_none());
    assert_eq!(db.history().unwrap()[0].status, Status::Uncertain);
}
#[test]
fn outside_event_window_never_queues() {
    let now = Utc::now();
    let mut s = Settings::default();
    s.rules.mode = Mode::Always;
    s.rules.app_ids.push("app".into());
    let m = Message::new(
        "k",
        "app",
        "App",
        "t",
        "b",
        now - chrono::Duration::hours(25),
    );
    assert!(!should_enqueue(&m, &s, now));
}

#[test]
fn successful_channel_is_not_sent_again_when_other_channel_retries() {
    let now = Utc::now();
    let mut db = Store::open_memory().unwrap();
    let mut s = Settings::default();
    s.rules.mode = Mode::Always;
    s.rules.app_ids.push("a".into());
    for id in ["one", "two"] {
        s.channels.push(ChannelConfig {
            id: id.into(),
            kind: ChannelKind::Feishu,
            name: id.into(),
            enabled: true,
        });
    }
    let m = Message::new("k", "a", "A", "T", "B", now);
    db.ingest(&m, false, &s, true, now).unwrap();
    let mut sent = vec![];
    for _ in 0..10 {
        if let Some(j) = next_job(&mut db, &s, now).unwrap() {
            sent.push(j.channel.id.clone());
            let out = if j.channel.id == "two" {
                Outcome::new(Status::RetryableFailed, "retry")
            } else {
                Outcome::new(Status::Succeeded, "")
            };
            apply_outcome(&mut db, &j.part, out, now).unwrap();
        }
    }
    assert_eq!(sent, vec!["one", "two"]);
    let j = next_job(&mut db, &s, now + chrono::Duration::seconds(31))
        .unwrap()
        .unwrap();
    assert_eq!(j.channel.id, "two");
    apply_outcome(
        &mut db,
        &j.part,
        Outcome::new(Status::Succeeded, ""),
        now + chrono::Duration::seconds(31),
    )
    .unwrap();
    assert!(
        next_job(&mut db, &s, now + chrono::Duration::hours(1))
            .unwrap()
            .is_none()
    );
}
#[test]
fn persisted_retry_budget_stops_after_five_retries() {
    let mut now = Utc::now();
    let mut db = Store::open_memory().unwrap();
    let mut s = Settings::default();
    s.rules.mode = Mode::Always;
    s.rules.app_ids.push("a".into());
    s.channels.push(ChannelConfig {
        id: "c".into(),
        kind: ChannelKind::Feishu,
        name: "C".into(),
        enabled: true,
    });
    let m = Message::new("k", "a", "A", "T", "B", now);
    db.ingest(&m, false, &s, true, now).unwrap();
    next_job(&mut db, &s, now).unwrap();
    let mut sends = 0;
    while let Some(j) = next_job(&mut db, &s, now).unwrap() {
        sends += 1;
        apply_outcome(
            &mut db,
            &j.part,
            Outcome::new(Status::RetryableFailed, "retry"),
            now,
        )
        .unwrap();
        if let Some(next) = db.history().unwrap()[0].next {
            now = next;
        } else {
            break;
        }
    }
    assert_eq!(sends, 6);
    assert_eq!(db.history().unwrap()[0].status, Status::PermanentFailed);
    assert!(
        next_job(&mut db, &s, now + chrono::Duration::hours(3))
            .unwrap()
            .is_none()
    );
}
#[test]
fn exactly_twenty_four_hours_expires_before_send() {
    let now = Utc::now();
    let mut db = Store::open_memory().unwrap();
    let mut s = Settings::default();
    s.rules.mode = Mode::Always;
    s.rules.app_ids.push("a".into());
    s.channels.push(ChannelConfig {
        id: "c".into(),
        kind: ChannelKind::Feishu,
        name: "C".into(),
        enabled: true,
    });
    db.ingest(
        &Message::new("k", "a", "A", "T", "B", now),
        false,
        &s,
        true,
        now,
    )
    .unwrap();
    assert!(
        next_job(&mut db, &s, now + chrono::Duration::hours(24))
            .unwrap()
            .is_none()
    );
    assert_eq!(db.history().unwrap()[0].status, Status::Expired);
}
