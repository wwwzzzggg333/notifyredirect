using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using Xunit;
namespace NotifyRedirect.Tests;
public class StorageAndDeliveryTests
{
    [Fact] public void FailedIngressDoesNotConsumeNotification()
    {
        using var s=new SqliteStore(":memory:");var m=Message();
        Assert.Throws<InvalidOperationException>(()=>s.ObserveAndApply(m,false,_=>throw new InvalidOperationException()));
        Assert.Equal(Observation.New,s.Observe(m,false));
    }
    [Fact] public void UpdateBeforeFirstSendReplacesExpandedParts()
    {
        using var s=new SqliteStore(":memory:");var m=Message();var channels=new[]{new ChannelConfig{Id="ch",Enabled=true}};
        s.Enqueue(m,channels,DateTimeOffset.UtcNow);s.ExpandParts(s.History()[0],["old part"]);
        s.Enqueue(m with {Id="new",Body="updated"},channels,DateTimeOffset.UtcNow);
        var queued=Assert.Single(s.History());Assert.Equal(-1,queued.PartIndex);Assert.Equal("updated",s.GetMessage(queued.MessageId)!.Body);
    }
    [Fact] public void BaselineDuplicateAndUpdateAreDistinct()
    {
        using var store=new SqliteStore(":memory:");var m=Message();
        Assert.Equal(Observation.Baseline,store.Observe(m,true));
        Assert.Equal(Observation.Duplicate,store.Observe(m,false));
        Assert.Equal(Observation.Changed,store.Observe(m with {Body="new"},false));
        Assert.Equal(Observation.New,store.Observe(m with {SourceKey="other"},false));
    }
    [Fact] public void OutsideWindowNeverQueues()
    {
        using var store=new SqliteStore(":memory:");
        var settings=new AppSettings {Rules=new(){Mode=ScheduleMode.Paused,AppIds=["app"]},Channels=[new(){Id="ch",Enabled=true}]};
        new ForwardingPipeline(store,()=>settings,TimeProvider.System).Handle(Message());
        Assert.Empty(store.History());
    }
    [Fact] public void QueuePersistsAndSuccessDoesNotRetry()
    {
        var file=Path.Combine(Path.GetTempPath(),Guid.NewGuid()+".db");
        try {
            using(var s=new SqliteStore(file))s.Enqueue(Message(),[new(){Id="ch",Enabled=true}],DateTimeOffset.UtcNow);
            using(var s=new SqliteStore(file)){Assert.Single(s.History());s.Mark(s.History()[0].Id,DeliveryStatus.Succeeded,0,null,null);Assert.Empty(s.Due(DateTimeOffset.UtcNow));}
        } finally {foreach(var f in Directory.GetFiles(Path.GetDirectoryName(file)!,Path.GetFileName(file)+"*"))File.Delete(f);}
    }
    [Fact] public void RetryStopsAfterFiveRetries()
    {
        Assert.NotNull(RetryPolicy.NextDelay(0,null));Assert.NotNull(RetryPolicy.NextDelay(4,null));Assert.Null(RetryPolicy.NextDelay(5,null));
    }
    [Fact] public void PendingNotificationUpdateReplacesPayload()
    {
        using var s=new SqliteStore(":memory:");var m=Message();var channels=new[]{new ChannelConfig{Id="ch",Enabled=true}};
        s.Enqueue(m,channels,DateTimeOffset.UtcNow);s.Enqueue(m with {Id="new",Body="updated"},channels,DateTimeOffset.UtcNow);
        Assert.Single(s.History());Assert.Equal("updated",s.GetMessage(s.History()[0].MessageId)!.Body);
    }
    [Fact] public void NewerSchemaRefusesWithoutDeletingDatabase()
    {
        using var s=new SqliteStore(":memory:"); Assert.Throws<InvalidOperationException>(()=>s.CheckVersion(999));
    }
    static ForwardMessage Message()=>new("id","key","app","App","title","body",DateTimeOffset.UtcNow);
}
