use crate::http::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use notifyredirect_core::*;
use sha2::Sha256;
use std::time::Duration;
fn sign(key: &[u8], body: &[u8]) -> String {
    let mut h = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    h.update(body);
    STANDARD.encode(h.finalize().into_bytes())
}
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub fn build_request(
    kind: ChannelKind,
    s: &ChannelSecret,
    text: &str,
    now: UtcTime,
) -> Result<HttpRequest> {
    let (secure, host, port, path) = parse_url(&s.webhook).map_err(|_| "Webhook 地址无效")?;
    let ok = secure
        && port == 443
        && match kind {
            ChannelKind::Feishu => {
                host == "open.feishu.cn"
                    && path.starts_with("/open-apis/bot/v2/hook/")
                    && path.len() > 23
            }
            ChannelKind::DingTalk => {
                host == "oapi.dingtalk.com"
                    && path.starts_with("/robot/send?")
                    && path.split_once('?').is_some_and(|(_, q)| {
                        q.split('&').any(|p| {
                            p.strip_prefix("access_token=")
                                .is_some_and(|v| !v.is_empty())
                        })
                    })
            }
            ChannelKind::WeCom => {
                host == "qyapi.weixin.qq.com"
                    && path.starts_with("/cgi-bin/webhook/send?")
                    && path.split_once('?').is_some_and(|(_, q)| {
                        q.split('&')
                            .any(|p| p.strip_prefix("key=").is_some_and(|v| !v.is_empty()))
                    })
            }
        };
    if !ok {
        return Err("需要所选平台的官方 HTTPS Webhook 地址".into());
    }
    let mut url = s.webhook.clone();
    let mut payload = match kind {
        ChannelKind::Feishu => serde_json::json!({"msg_type":"text","content":{"text":text}}),
        _ => serde_json::json!({"msgtype":"text","text":{"content":text}}),
    };
    if !s.signing_key.is_empty() {
        match kind {
            ChannelKind::Feishu => {
                let stamp = now.timestamp().to_string();
                payload["timestamp"] = stamp.clone().into();
                payload["sign"] =
                    sign(format!("{stamp}\n{}", s.signing_key).as_bytes(), b"").into();
            }
            ChannelKind::DingTalk => {
                let stamp = now.timestamp_millis();
                url.push_str(&format!(
                    "&timestamp={stamp}&sign={}",
                    encode(&sign(
                        s.signing_key.as_bytes(),
                        format!("{stamp}\n{}", s.signing_key).as_bytes()
                    ))
                ));
            }
            _ => {}
        }
    }
    Ok(HttpRequest {
        url,
        headers: vec![(
            "Content-Type".into(),
            "application/json; charset=utf-8".into(),
        )],
        body: payload.to_string().into_bytes(),
        timeout: Duration::from_secs(15),
        max_response_bytes: 65536,
    })
}
pub fn classify(response: std::result::Result<HttpResponse, HttpError>, now: UtcTime) -> Outcome {
    let r = match response {
        Ok(r) => r,
        Err(HttpError::NotSent) => return Outcome::new(Status::RetryableFailed, "未建立连接"),
        Err(_) => return Outcome::new(Status::Uncertain, "发送结果未确认，请先检查手机"),
    };
    let retry = r
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, v)| {
            v.parse::<u64>().ok().map(Duration::from_secs).or_else(|| {
                chrono::DateTime::parse_from_rfc2822(v)
                    .ok()
                    .map(|d| (d.to_utc() - now).to_std().unwrap_or_default())
            })
        });
    if r.status == 429 || r.status >= 500 {
        let mut o = Outcome::new(Status::RetryableFailed, "平台限流或暂不可用");
        o.retry_after = retry;
        return o;
    }
    if !(200..300).contains(&r.status) {
        return Outcome::new(
            Status::PermanentFailed,
            &format!("平台拒绝请求 HTTP {}", r.status),
        );
    }
    if r.body.len() > 65536 {
        return Outcome::new(Status::Uncertain, "响应超限");
    }
    let code = serde_json::from_slice::<serde_json::Value>(&r.body)
        .ok()
        .and_then(|j| {
            ["code", "errcode", "StatusCode"]
                .iter()
                .find_map(|k| j.get(k).and_then(|v| v.as_i64()))
        });
    match code {
        Some(0) => Outcome::new(Status::Succeeded, ""),
        Some(99991400 | 130101 | 45009) => {
            let mut o = Outcome::new(Status::RetryableFailed, "平台限流");
            o.retry_after = Some(Duration::from_secs(60));
            o
        }
        Some(code) => Outcome::new(Status::PermanentFailed, &format!("平台业务错误 {code}")),
        None => Outcome::new(Status::Uncertain, "无法确认平台响应"),
    }
}
