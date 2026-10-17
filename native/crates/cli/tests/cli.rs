use notifyredirect_cli::*;
#[test]
fn args_generate_new_protocol_and_reject_unknown_flags() {
    let args: Vec<String> = [
        "send",
        "--source",
        "claude",
        "--title",
        "Done",
        "--body",
        "结果",
        "--state",
        "turn_ended",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let c = parse(&args, "", chrono::Utc::now()).unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&c.body).unwrap();
    assert_eq!(v["state"], "turn_ended");
    assert_eq!(v["source"], "claude");
    assert!(v.get("event_id").is_some());
    let args = vec!["send".into(), "--bad".into(), "x".into()];
    assert!(parse(&args, "", chrono::Utc::now()).is_err());
    assert!(
        parse(&["--help".into()], "", chrono::Utc::now())
            .unwrap()
            .is_none()
    );
}
