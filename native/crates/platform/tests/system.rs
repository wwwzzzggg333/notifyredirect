use notifyredirect_platform::*;
#[test] fn dpapi_roundtrip_and_tamper_rejection(){let sealed=protect("测试 token🙂").unwrap();assert!(!sealed.contains("token"));assert_eq!(unprotect(&sealed).unwrap(),"测试 token🙂");assert!(unprotect("invalid").is_err());}
#[test] fn new_paths_are_independent(){let p=app_db().unwrap();assert!(p.to_string_lossy().contains("NotifyRedirectNative"));assert!(!p.to_string_lossy().contains("NotifyRedirect.Desktop"));}
