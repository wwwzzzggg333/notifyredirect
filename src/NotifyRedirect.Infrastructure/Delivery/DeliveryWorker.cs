using NotifyRedirect.Core;
namespace NotifyRedirect.Infrastructure;
public sealed class DeliveryWorker(SqliteStore store,Func<AppSettings> settings,IReadOnlyList<INotificationChannel> channels,TimeProvider clock)
{
    public async Task RunOnceAsync(CancellationToken ct)
    {
        foreach(var part in store.Due(clock.GetUtcNow()))
        {
            var now=clock.GetUtcNow();var config=settings();
            if(now>=part.ExpiresAt){store.Mark(part.Id,DeliveryStatus.Expired,part.RetryCount,null,"消息过期");continue;}
            if(!ScheduleEvaluator.IsAllowed(now,TimeZoneInfo.Local,config.Rules))return;
            var m=store.GetMessage(part.MessageId);var channel=config.Channels.FirstOrDefault(c=>c.Id==part.ChannelId&&c.Enabled);
            if(m==null||channel==null||!MessageFilter.Accept(m,config.Rules)){store.Mark(part.Id,DeliveryStatus.PermanentFailed,part.RetryCount,null,"来源或渠道已停用");continue;}
            var sender=channels.FirstOrDefault(c=>c.Kind==channel.Kind);
            if(sender==null){store.Mark(part.Id,DeliveryStatus.PermanentFailed,part.RetryCount,null,"渠道不可用");continue;}
            if(part.PartIndex<0){try{store.ExpandParts(part,sender.PrepareParts(channel,m));}catch{store.Mark(part.Id,DeliveryStatus.PermanentFailed,0,null,"渠道配置无效，请重新配置");}continue;}
            store.Mark(part.Id,DeliveryStatus.InFlight,part.RetryCount,null,null);
            SendOutcome outcome;
            try{outcome=await sender.SendAsync(channel,m,part,ct);}
            catch(OperationCanceledException){store.Mark(part.Id,DeliveryStatus.Uncertain,part.RetryCount,null,"发送中取消，结果不确定");throw;}
            catch{outcome=new(DeliveryStatus.PermanentFailed,"渠道配置或凭据不可用");}
            var status=outcome.Status;var retries=part.RetryCount;DateTimeOffset? next=null;
            if(status==DeliveryStatus.RetryableFailed)
            {
                var delay=RetryPolicy.NextDelay(retries,outcome.RetryAfter);
                if(delay==null)status=DeliveryStatus.PermanentFailed;
                else {retries++;next=clock.GetUtcNow()+delay.Value;if(next>=part.ExpiresAt)status=DeliveryStatus.Expired;}
            }
            store.Mark(part.Id,status,retries,next,outcome.SafeErrorCode);
        }
    }
}
