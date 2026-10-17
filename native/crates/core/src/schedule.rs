use crate::*;
use chrono::{Datelike, TimeZone, Timelike};
pub fn parse_time(s: &str) -> Result<u32> {
    let t = chrono::NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|_| "时间格式应为 HH:mm".to_string())?;
    Ok(t.num_seconds_from_midnight())
}
pub fn is_allowed<Tz: TimeZone>(now: UtcTime, zone: &Tz, r: &Rules) -> bool {
    match r.mode {
        Mode::Paused => false,
        Mode::Always => true,
        Mode::Scheduled => {
            let l = now.with_timezone(zone);
            let day = l.weekday().number_from_monday();
            let previous = if day == 1 { 7 } else { day - 1 };
            let t = l.num_seconds_from_midnight();
            r.windows.iter().any(|w| {
                if w.all_day {
                    return w.days.contains(&day);
                }
                let (Ok(a), Ok(b)) = (parse_time(&w.start), parse_time(&w.end)) else {
                    return false;
                };
                if a < b {
                    w.days.contains(&day) && t >= a && t < b
                } else {
                    a != b
                        && ((t >= a && w.days.contains(&day))
                            || (t < b && w.days.contains(&previous)))
                }
            })
        }
    }
}
pub fn accept(m: &Message, r: &Rules) -> bool {
    if !r.app_ids.contains(&m.app_id) {
        return false;
    }
    let t = format!("{}\n{}", m.title, m.body).to_lowercase();
    let valid = |s: &&String| !s.trim().is_empty();
    !r.exclude_keywords
        .iter()
        .filter(valid)
        .any(|s| t.contains(&s.to_lowercase()))
        && (!r.include_keywords.iter().any(|s| !s.trim().is_empty())
            || r.include_keywords
                .iter()
                .filter(valid)
                .any(|s| t.contains(&s.to_lowercase())))
}
pub fn parse_windows(text: &str) -> Result<Vec<ScheduleWindow>> {
    let mut result = vec![];
    for line in text.lines().filter(|s| !s.trim().is_empty()) {
        let f: Vec<_> = line.split('|').map(str::trim).collect();
        if f.len() < 2 {
            return Err("每行格式：星期 | 开始 | 结束，或 星期 | all".into());
        }
        let days = f[0]
            .split(',')
            .map(|s| s.trim().parse::<u32>().map_err(|_| "星期无效".to_string()))
            .collect::<Result<Vec<_>>>()?;
        let all = f[1].eq_ignore_ascii_case("all");
        if !all && f.len() != 3 {
            return Err("请填写开始和结束时间".into());
        }
        result.push(ScheduleWindow {
            days,
            start: if all { "00:00" } else { f[1] }.into(),
            end: if all { "00:00" } else { f[2] }.into(),
            all_day: all,
        });
    }
    let s = Settings {
        rules: Rules {
            windows: result.clone(),
            ..Rules::default()
        },
        ..Settings::default()
    };
    s.validate()?;
    Ok(result)
}
