use notifyredirect_core::*;
use chrono::Utc;
#[test] fn validates_new_protocol_and_unicode_limits(){
 let now=Utc::now(); let mut e=serde_json::json!({"version":1,"event_id":"e","source":"agent","title":"🙂".repeat(1000)});
 let m=parse_event(e.to_string().as_bytes(),now).unwrap();assert_eq!(m.state,AgentState::Unknown);assert_eq!(m.app_id,"agent:agent");
 e["title"]=serde_json::json!("🙂".repeat(1001));assert!(parse_event(e.to_string().as_bytes(),now).is_err());
 e["title"]=serde_json::json!("Done");e["state"]=serde_json::json!("Succeeded");assert!(parse_event(e.to_string().as_bytes(),now).is_err());
 e["state"]=serde_json::json!("succeeded");e["occurred_at"]=serde_json::json!((now-chrono::Duration::hours(24)).to_rfc3339());assert!(parse_event(e.to_string().as_bytes(),now).is_ok());
 e["occurred_at"]=serde_json::json!((now-chrono::Duration::hours(24)-chrono::Duration::seconds(1)).to_rfc3339());assert!(parse_event(e.to_string().as_bytes(),now).is_err());
}
#[test] fn splits_without_losing_characters_or_exceeding_final_budget(){
 let p=split("你好🙂世界🙂",8).unwrap();assert_eq!(p.concat(),"你好🙂世界🙂");assert!(p.iter().all(|s|s.len()<=8));
 let m=Message::new("k","a","App","Title",&"🙂".repeat(5000),Utc::now());let p=prepare_parts(&m,ChannelKind::WeCom,"PC").unwrap();assert!(p.len()>1);assert!(p.iter().all(|s|s.len()<=1800));assert!(p[0].starts_with("[1/"));
 assert!(next_delay(5,None).is_none());assert_eq!(next_delay(0,None).unwrap().as_secs(),30);assert_eq!(next_delay(4,None).unwrap().as_secs(),7200);
}
