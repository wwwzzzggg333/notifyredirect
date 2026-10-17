use crate::*;
use serde::Deserialize;
#[derive(Deserialize)]
struct Event {
    version: u32,
    event_id: String,
    source: String,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    state: AgentState,
    occurred_at: Option<UtcTime>,
    #[serde(default)]
    project: String,
    #[serde(default)]
    session_id: String,
}
pub fn parse_event(data: &[u8], now: UtcTime) -> Result<Message> {
    if data.len() > 131072 {
        return Err("请求体超限".into());
    }
    let e: Event = serde_json::from_slice(data).map_err(|_| "事件 JSON 无效".to_string())?;
    if e.version != 1 {
        return Err("version 必须为 1".into());
    }
    for (s, max, required) in [
        (&e.source, 80, true),
        (&e.event_id, 200, true),
        (&e.title, 1000, true),
        (&e.body, 65536, false),
        (&e.project, 1000, false),
        (&e.session_id, 1000, false),
    ] {
        if s.chars().count() > max || (required && s.trim().is_empty()) {
            return Err("字段为空或超限".into());
        }
    }
    let at = e.occurred_at.unwrap_or(now);
    if at < now - chrono::Duration::hours(24) || at > now + chrono::Duration::minutes(5) {
        return Err("事件时间超出范围".into());
    }
    let mut m = Message::new(
        &format!(
            "agent:{}",
            serde_json::to_string(&(&e.source, &e.event_id)).map_err(|_| "事件标识无效")?
        ),
        &format!("agent:{}", e.source),
        &e.source,
        &e.title,
        &e.body,
        at,
    );
    m.state = e.state;
    m.project = e.project;
    m.session_id = e.session_id;
    Ok(m)
}
pub fn map(s: SourceSnapshot) -> Option<Message> {
    if s.text.iter().all(|s| s.trim().is_empty()) {
        return None;
    }
    Some(Message::new(
        &s.source_key,
        &s.app_id,
        &s.app_name,
        s.text.first().map(String::as_str).unwrap_or(""),
        &s.text
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
        s.created_at,
    ))
}
pub fn fingerprint(m: &Message) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(format!("{}\0{}", m.title, m.body).as_bytes())
    )
}
