use notifyredirect_core::*;
use notifyredirect_platform::*;
use notifyredirect_storage::Store;
use std::{collections::HashMap, path::PathBuf};
pub struct Submission {
    pub body: Vec<u8>,
    pub data_dir: Option<PathBuf>,
}
pub fn parse(args: &[String], stdin: &str, now: UtcTime) -> Result<Option<Submission>> {
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        return Ok(None);
    }
    if args[0] != "send" {
        return Err("只支持 send 命令".into());
    }
    let mut fields = HashMap::new();
    let mut json = false;
    let mut i = 1;
    while i < args.len() {
        let key = &args[i];
        if key == "--stdin-json" {
            if json {
                return Err("重复参数".into());
            }
            json = true;
            i += 1;
            continue;
        }
        if !["--source", "--title", "--body", "--state", "--data-dir"].contains(&key.as_str())
            || i + 1 >= args.len()
        {
            return Err("参数无效或缺少值".into());
        }
        if fields.insert(key.as_str(), args[i + 1].as_str()).is_some() {
            return Err("重复参数".into());
        }
        i += 2;
    }
    let body = if json {
        if fields.keys().any(|k| *k != "--data-dir") {
            return Err("stdin-json 不可混用事件参数".into());
        }
        stdin.as_bytes().to_vec()
    } else {
        serde_json::json!({"version":1,"event_id":id(),"source":fields.get("--source").copied().unwrap_or("custom"),"title":fields.get("--title").copied().unwrap_or("Agent 通知"),"body":fields.get("--body").copied().unwrap_or(""),"state":fields.get("--state").copied().unwrap_or("unknown"),"occurred_at":now}).to_string().into_bytes()
    };
    parse_event(&body, now)?;
    Ok(Some(Submission {
        body,
        data_dir: fields.get("--data-dir").map(PathBuf::from),
    }))
}
pub fn submit(s: &Submission, transport: &dyn Transport) -> Result<()> {
    let path = cli_db(s.data_dir.as_deref())?;
    let store = Store::read_only(&path)?;
    let token = unprotect(&store.get("api-token")?.ok_or("请启用新版 Agent 入口")?)?;
    let r = transport
        .send(&HttpRequest {
            url: "http://127.0.0.1:17842/v1/events".into(),
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("Authorization".into(), format!("Bearer {token}")),
            ],
            body: s.body.clone(),
            timeout: std::time::Duration::from_secs(3),
            max_response_bytes: 4096,
        })
        .map_err(|_| "本机入口请求失败")?;
    if r.status == 202 {
        Ok(())
    } else {
        Err("本机入口未接受事件".into())
    }
}
