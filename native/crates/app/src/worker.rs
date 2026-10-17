use notifyredirect_core::*;
use notifyredirect_storage::Store;
#[derive(Clone)]
pub struct Job {
    pub message: Message,
    pub channel: ChannelConfig,
    pub part: DeliveryRecord,
    pub computer: String,
}
pub fn should_enqueue(m: &Message, s: &Settings, now: UtcTime) -> bool {
    m.occurred_at >= now - chrono::Duration::hours(24)
        && m.occurred_at <= now + chrono::Duration::minutes(5)
        && accept(m, &s.rules)
        && is_allowed(m.occurred_at, &chrono::Local, &s.rules)
        && is_allowed(now, &chrono::Local, &s.rules)
}
pub fn next_job(db: &mut Store, s: &Settings, now: UtcTime) -> Result<Option<Job>> {
    for mut d in db.due(now)? {
        if d.expires <= now {
            d.status = Status::Expired;
            d.error = Some("超过24小时".into());
            db.mark(&d)?;
            continue;
        }
        if !is_allowed(now, &chrono::Local, &s.rules) {
            return Ok(None);
        }
        let Some(m) = db.message(&d.message_id)? else {
            continue;
        };
        let c = s
            .channels
            .iter()
            .find(|c| c.id == d.channel_id && c.enabled);
        if !accept(&m, &s.rules) || c.is_none() {
            d.status = Status::PermanentFailed;
            d.error = Some("来源或渠道已停用".into());
            db.mark(&d)?;
            continue;
        }
        let c = c.unwrap();
        if d.part < 0 {
            db.expand(&d, &prepare_parts(&m, c.kind, &s.computer_name)?)?;
            continue;
        }
        d.status = Status::InFlight;
        d.attempted = true;
        db.mark(&d)?;
        return Ok(Some(Job {
            message: m,
            channel: c.clone(),
            part: d,
            computer: s.computer_name.clone(),
        }));
    }
    Ok(None)
}
pub fn apply_outcome(
    db: &mut Store,
    part: &DeliveryRecord,
    out: Outcome,
    now: UtcTime,
) -> Result<()> {
    let mut d = part.clone();
    d.status = out.status;
    d.error = out.error;
    d.next = None;
    if d.status == Status::RetryableFailed {
        if let Some(delay) = next_delay(d.retries, out.retry_after) {
            d.retries += 1;
            let t = now + chrono::Duration::from_std(delay).map_err(|_| "重试时间无效")?;
            if t >= d.expires {
                d.status = Status::Expired
            } else {
                d.next = Some(t)
            }
        } else {
            d.status = Status::PermanentFailed
        }
    }
    db.mark(&d)
}
