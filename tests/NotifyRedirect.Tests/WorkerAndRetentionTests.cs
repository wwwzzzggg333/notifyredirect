using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using Xunit;
namespace NotifyRedirect.Tests;
public class WorkerAndRetentionTests
{
    readonly DateTimeOffset now=DateTimeOffset.UtcNow;
    ForwardMessage Message()=>new(Guid.NewGuid().ToString("N"),Guid.NewGuid().ToString("N"),"app","App","t","b",now);
    static AppSettings Settings(ScheduleMode mode=ScheduleMode.Always)=>new(){Rules=new(){Mode=mode,AppIds=["app"]},Channels=[new(){Id="one",Enabled=true},new(){Id="two",Enabled=true}]};
    [Fact] public async Task OutsideCurrentWindowNeverSends()
    {
        using var s=new SqliteStore(":memory:");var config=Settings();s.Enqueue(Message(),config.Channels,now);var sender=new Sender();config=config with {Rules=config.Rules with {Mode=ScheduleMode.Paused}};
        await new DeliveryWorker(s,()=>config,[sender],new Clock(now)).RunOnceAsync(default);
        Assert.Equal(0,sender.Sends);Assert.All(s.History(),r=>Assert.Equal(DeliveryStatus.Pending,r.Status));
    }
    [Fact] public async Task SuccessDoesNotRepeatAndFailedChannelRetriesSeparately()
    {
        using var s=new SqliteStore(":memory:");var config=Settings();s.Enqueue(Message(),config.Channels,now);var sender=new Sender{FailChannel="two"};var clock=new Clock(now);var worker=new DeliveryWorker(s,()=>config,[sender],clock);
        await worker.RunOnceAsync(default);await worker.RunOnceAsync(default);Assert.Equal(2,sender.Sends);
        clock.Now=now.AddMinutes(3);await worker.RunOnceAsync(default);
        Assert.Equal(3,sender.Sends);Assert.Single(s.History(),r=>r.Status==DeliveryStatus.Succeeded);
    }
    [Fact] public async Task Exactly24HoursExpiresWithoutSend()
    {
        using var s=new SqliteStore(":memory:");var config=Settings();s.Enqueue(Message(),config.Channels,now);var sender=new Sender();await new DeliveryWorker(s,()=>config,[sender],new Clock(now.AddHours(24))).RunOnceAsync(default);
        Assert.Equal(0,sender.Sends);Assert.All(s.History(),r=>Assert.Equal(DeliveryStatus.Expired,r.Status));
    }
    [Fact] public async Task UncertainDoesNotAutomaticallyRetry()
    {
        using var s=new SqliteStore(":memory:");var config=Settings();s.Enqueue(Message(),config.Channels,now);var sender=new Sender{Outcome=DeliveryStatus.Uncertain};var worker=new DeliveryWorker(s,()=>config,[sender],new Clock(now));await worker.RunOnceAsync(default);await worker.RunOnceAsync(default);await worker.RunOnceAsync(default);Assert.Equal(2,sender.Sends);
    }
    [Fact] public void RetentionClearsBodyAtSevenDaysAndStatusAtThirtyDays()
    {
        using var s=new SqliteStore(":memory:");var m=Message();s.Enqueue(m,Settings().Channels,now);
        s.Purge(now.AddDays(7));Assert.Equal("",s.GetMessage(m.Id)!.Body);Assert.NotEmpty(s.History());s.Purge(now.AddDays(30));Assert.Null(s.GetMessage(m.Id));Assert.Empty(s.History());
    }
    [Fact] public void ReusedSourceIdInDifferentAppsDoesNotDeduplicate()
    {
        using var s=new SqliteStore(":memory:");var m=Message();Assert.Equal(Observation.New,s.Observe(m,false));Assert.Equal(Observation.New,s.Observe(m with {SourceKey="other:1"},false));
    }
    sealed class Clock(DateTimeOffset time):TimeProvider{public DateTimeOffset Now=time;public override DateTimeOffset GetUtcNow()=>Now;}
    sealed class Sender:INotificationChannel
    {
        public ChannelKind Kind=>ChannelKind.Feishu;public int Sends;public string? FailChannel;public DeliveryStatus Outcome=DeliveryStatus.Succeeded;
        public IReadOnlyList<string> PrepareParts(ChannelConfig c,ForwardMessage m)=>[m.Body];
        public Task<SendOutcome> SendAsync(ChannelConfig c,ForwardMessage m,DeliveryRecord p,CancellationToken ct){Sends++;return Task.FromResult(new SendOutcome(c.Id==FailChannel?DeliveryStatus.RetryableFailed:Outcome));}
    }
}
