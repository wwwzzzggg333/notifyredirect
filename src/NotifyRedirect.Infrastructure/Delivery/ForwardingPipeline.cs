using NotifyRedirect.Core;
namespace NotifyRedirect.Infrastructure;
public sealed class ForwardingPipeline(SqliteStore store,Func<AppSettings> settings,TimeProvider clock)
{
    public void Handle(ForwardMessage m)
    {
        var now=clock.GetUtcNow();var config=settings();
        if(m.OccurredAt>now.AddMinutes(5)||m.OccurredAt<now.AddHours(-24))return;
        if(!MessageFilter.Accept(m,config.Rules)||!ScheduleEvaluator.IsAllowed(m.OccurredAt,TimeZoneInfo.Local,config.Rules)||!ScheduleEvaluator.IsAllowed(now,TimeZoneInfo.Local,config.Rules))return;
        store.Enqueue(m,config.Channels,now);
    }
}
