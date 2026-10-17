use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub type UtcTime = DateTime<Utc>;
pub type Result<T> = std::result::Result<T, String>;
pub fn id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Always,
    Scheduled,
    #[default]
    Paused,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    #[default]
    Feishu,
    DingTalk,
    WeCom,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    #[default]
    Unknown,
    TurnEnded,
    ProcessExited,
    Succeeded,
    Failed,
    WaitingInput,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InFlight,
    Succeeded,
    RetryableFailed,
    PermanentFailed,
    Uncertain,
    Expired,
    Expanded,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleWindow {
    pub days: Vec<u32>,
    pub start: String,
    pub end: String,
    pub all_day: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Rules {
    pub mode: Mode,
    pub windows: Vec<ScheduleWindow>,
    pub app_ids: Vec<String>,
    pub include_keywords: Vec<String>,
    pub exclude_keywords: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub id: String,
    pub kind: ChannelKind,
    pub name: String,
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelSecret {
    pub webhook: String,
    pub signing_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub rules: Rules,
    pub channels: Vec<ChannelConfig>,
    pub computer_name: String,
    pub api_enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: Rules::default(),
            channels: vec![],
            computer_name: std::env::var("COMPUTERNAME").unwrap_or("PC".into()),
            api_enabled: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.computer_name.trim().is_empty() {
            return Err("请填写电脑名称".into());
        }
        if self.rules.mode == Mode::Scheduled && self.rules.windows.is_empty() {
            return Err("请添加时间段".into());
        }
        for w in &self.rules.windows {
            if w.days.is_empty() || w.days.iter().any(|d| !(1..=7).contains(d)) {
                return Err("星期必须为 1–7".into());
            }
            if !w.all_day
                && (crate::parse_time(&w.start).is_err()
                    || crate::parse_time(&w.end).is_err()
                    || w.start == w.end)
            {
                return Err("时间段无效，请使用 HH:mm，起止不可相同".into());
            }
        }
        let mut ids = std::collections::HashSet::new();
        for c in &self.channels {
            if c.id.is_empty() || c.name.trim().is_empty() || !ids.insert(&c.id) {
                return Err("渠道名称或标识无效".into());
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub source_key: String,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub occurred_at: UtcTime,
    pub project: String,
    pub session_id: String,
    pub state: AgentState,
}
impl Message {
    pub fn new(key: &str, app: &str, name: &str, title: &str, body: &str, time: UtcTime) -> Self {
        Self {
            id: id(),
            source_key: key.into(),
            app_id: app.into(),
            app_name: name.into(),
            title: title.into(),
            body: body.into(),
            occurred_at: time,
            project: String::new(),
            session_id: String::new(),
            state: AgentState::Unknown,
        }
    }
}
#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    pub source_key: String,
    pub app_id: String,
    pub app_name: String,
    pub text: Vec<String>,
    pub created_at: UtcTime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRecord {
    pub id: String,
    pub message_id: String,
    pub channel_id: String,
    pub part: i32,
    pub body: Option<String>,
    pub status: Status,
    pub retries: u32,
    #[serde(default)]
    pub attempted: bool,
    pub next: Option<UtcTime>,
    pub expires: UtcTime,
    pub error: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Outcome {
    pub status: Status,
    pub error: Option<String>,
    pub retry_after: Option<std::time::Duration>,
}
impl Outcome {
    pub fn new(status: Status, error: &str) -> Self {
        Self {
            status,
            error: if error.is_empty() {
                None
            } else {
                Some(error.into())
            },
            retry_after: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    Baseline,
    Duplicate,
    New,
    Changed,
}
