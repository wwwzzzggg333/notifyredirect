fn smoke_report() -> serde_json::Value {
    let dir = std::env::temp_dir().join(format!("notify-ui-{}", notifyredirect_core::id()));
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_NotifyRedirect"))
        .args(["--smoke-ui", "--data-dir", dir.to_str().unwrap()])
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("UI smoke timed out")
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let report = std::fs::read_to_string(dir.join("ui-smoke.json")).unwrap();
    let j: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(j["pages"], 4);
    assert_eq!(j["close_hides"], true);
    assert_eq!(j["import_keeps_edits"], true);

    assert!(j["controls"].as_u64().unwrap() > 40);
    std::fs::remove_dir_all(dir).unwrap();
    j
}
#[test]
fn native_window_survives_close_and_exit() {
    smoke_report();
}
#[test]
#[ignore = "Requires interactive Explorer desktop; run --ignored in normal user session"]
fn native_tray_registers() {
    assert_eq!(smoke_report()["tray_created"], true);
}
#[test]
fn single_instance_is_acquired_before_data_access() {
    let first = notifyredirect_app::ui::acquire_instance(true)
        .unwrap()
        .unwrap();
    assert!(
        notifyredirect_app::ui::acquire_instance(true)
            .unwrap()
            .is_none()
    );
    drop(first);
    assert!(
        notifyredirect_app::ui::acquire_instance(true)
            .unwrap()
            .is_some()
    );
}
