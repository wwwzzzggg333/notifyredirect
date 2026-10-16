use notifyredirect_core::*;
use chrono::{DateTime, FixedOffset};
#[test] fn schedule_crosses_midnight_and_stops_at_end() {
 let mut r=Rules::default(); r.mode=Mode::Scheduled;
 r.windows.push(ScheduleWindow{days:vec![5],start:"18:30".into(),end:"09:00".into(),all_day:false});
 let zone=FixedOffset::east_opt(8*3600).unwrap();
 for (time,want) in [("2026-10-16T18:29:59+08:00",false),("2026-10-16T18:30:00+08:00",true),("2026-10-17T08:59:59+08:00",true),("2026-10-17T09:00:00+08:00",false),("2026-10-18T08:00:00+08:00",false)] {
 assert_eq!(is_allowed(DateTime::parse_from_rfc3339(time).unwrap().to_utc(),&zone,&r),want,"{time}"); }
 r.mode=Mode::Paused; assert!(!is_allowed(chrono::Utc::now(),&zone,&r));
}
#[test] fn settings_reject_bad_rules_and_export_secrets() {
 let mut s=Settings::default();assert_eq!(s.rules.mode,Mode::Paused); assert!(s.rules.app_ids.is_empty());
 s.rules.mode=Mode::Scheduled; assert!(s.validate().is_err());
 s.rules.windows.push(ScheduleWindow{days:vec![1],start:"09:00".into(),end:"09:00".into(),all_day:false});assert!(s.validate().is_err());
 s.rules.windows[0].all_day=true; assert!(s.validate().is_ok());
 assert!(serde_json::from_str::<Settings>(r#"{"rules":{"mode":"magic"}}"#).is_err());
}
#[test] fn exclusion_wins_and_blank_words_do_not_block() {
 let mut r=Rules::default();r.app_ids.push("app".into());r.include_keywords=vec!["  ".into(),"DONE".into()];r.exclude_keywords=vec!["error".into()];
 let mut m=Message::new("k","app","App","Done","error",chrono::Utc::now()); assert!(!accept(&m,&r));m.body="完成".into();assert!(accept(&m,&r));
 r.include_keywords=vec![" ".into()];assert!(accept(&m,&r));r.app_ids.clear();assert!(!accept(&m,&r));
}
