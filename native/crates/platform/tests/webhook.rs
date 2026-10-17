use notifyredirect_core::*;
use notifyredirect_platform::*;
#[test]
fn rejects_wrong_hosts_and_builds_platform_requests() {
    let now = chrono::Utc::now();
    let mut s = ChannelSecret {
        webhook: "https://evil.test/open-apis/bot/v2/hook/a".into(),
        signing_key: String::new(),
    };
    assert!(build_request(ChannelKind::Feishu, &s, "x", now).is_err());
    s.webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/a".into();
    let r = build_request(ChannelKind::Feishu, &s, "你好", now).unwrap();
    let j: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(j["content"]["text"], "你好");
}
#[test]
fn response_failures_are_not_misreported_as_success() {
    for (code, body, status) in [
        (200, r#"{"code":0}"#, Status::Succeeded),
        (200, r#"{"code":19021}"#, Status::PermanentFailed),
        (429, "{}", Status::RetryableFailed),
        (503, "{}", Status::RetryableFailed),
        (401, "{}", Status::PermanentFailed),
        (200, "invalid", Status::Uncertain),
        (200, "{}", Status::Uncertain),
    ] {
        assert_eq!(
            classify(
                Ok(HttpResponse {
                    status: code,
                    headers: vec![],
                    body: body.as_bytes().to_vec()
                }),
                chrono::Utc::now()
            )
            .status,
            status
        );
    }
    assert_eq!(
        classify(Err(HttpError::Uncertain), chrono::Utc::now()).status,
        Status::Uncertain
    );
}

#[test]
fn signatures_match_independently_calculated_vectors() {
    let now = chrono::DateTime::from_timestamp(0, 0).unwrap();
    let feishu = ChannelSecret {
        webhook: "https://open.feishu.cn/open-apis/bot/v2/hook/a".into(),
        signing_key: "secret".into(),
    };
    let req = build_request(ChannelKind::Feishu, &feishu, "T", now).unwrap();
    let j: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(j["sign"], "iiUAFIK545Ca+5jMMyYI9vHpIzzWe2Rq4bb/sYP7KwY=");
    let ding = ChannelSecret {
        webhook: "https://oapi.dingtalk.com/robot/send?access_token=x".into(),
        signing_key: "secret".into(),
    };
    let req = build_request(ChannelKind::DingTalk, &ding, "T", now).unwrap();
    assert!(
        req.url
            .ends_with("&timestamp=0&sign=c3E3sYg8%2F8R9e5Be1VaOgM2cQZvu6iZYUqKt%2B%2F0tXdM%3D")
    );
}
