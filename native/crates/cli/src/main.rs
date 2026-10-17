use std::io::Read;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdin = String::new();
    if args.iter().any(|s| s == "--stdin-json") {
        let _ = std::io::stdin().take(131073).read_to_string(&mut stdin);
    }
    let result=notifyredirect_cli::parse(&args,&stdin,chrono::Utc::now()).and_then(|s|{if let Some(s)=s{notifyredirect_cli::submit(&s,&notifyredirect_platform::WinHttpTransport)?;println!("事件已接收；是否转发取决于来源、时间和渠道规则。");}else{println!("NotifyRedirect Native\nsend --source NAME --title TITLE --body BODY [--state unknown]\nsend --stdin-json [--data-dir DIRECTORY]\n先在新版 APP 启用 Agent 入口；首次来源需要勾选并保存。");}Ok(())});
    if result.is_err() {
        eprintln!("通知未提交，请检查新版 APP、入口及参数；源 Agent 无需停止。");
        std::process::exit(1);
    }
}
