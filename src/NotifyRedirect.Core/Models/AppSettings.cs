namespace NotifyRedirect.Core;
public enum ChannelKind { Feishu, DingTalk, WeCom }
public enum DeliveryStatus { Pending, InFlight, Succeeded, RetryableFailed, PermanentFailed, Uncertain, Expired, Expanded, Filtered }
public enum Observation { Baseline, Duplicate, New, Changed }
public record ChannelConfig
{
    public string Id { get; init; }=Guid.NewGuid().ToString("N");
    public ChannelKind Kind { get; init; }
    public string Name { get; init; }="";
    public bool Enabled { get; init; }
    public string SecretRef { get; init; }="";
    public override string ToString()=>$"{Name} · {Kind} · {(Enabled?"启用":"停用")} · 待手机实测";
}
public record AppSettings
{
    public ForwardingRules Rules { get; init; }=new();
    public List<ChannelConfig> Channels { get; init; }=[];
    public string ComputerName { get; init; }=Environment.MachineName;
    public bool ApiEnabled { get; init; }
}
public record DeliveryRecord(string Id,string MessageId,string ChannelId,int PartIndex,string? PartBody,DeliveryStatus Status,int RetryCount,DateTimeOffset? NextAttemptAt,DateTimeOffset ExpiresAt,string? ErrorCode)
{
    public override string ToString()=>$"{StatusLabel(Status)} · {MessageId[..Math.Min(8,MessageId.Length)]} · 渠道 {ChannelId[..Math.Min(6,ChannelId.Length)]} · {(PartIndex<0?"整条":$"第 {PartIndex+1} 段")} · {ErrorCode}";
    public static string StatusLabel(DeliveryStatus s)=>s switch { DeliveryStatus.Pending=>"待发送",DeliveryStatus.InFlight=>"发送中",DeliveryStatus.Succeeded=>"成功",DeliveryStatus.RetryableFailed=>"等待重试",DeliveryStatus.PermanentFailed=>"失败",DeliveryStatus.Uncertain=>"结果不确定",DeliveryStatus.Expired=>"已过期",DeliveryStatus.Filtered=>"未转发",_=>"已分段"};
}
public record SendOutcome(DeliveryStatus Status,string? SafeErrorCode=null,TimeSpan? RetryAfter=null);
public interface INotificationChannel
{
    ChannelKind Kind { get; }
    IReadOnlyList<string> PrepareParts(ChannelConfig config,ForwardMessage message);
    Task<SendOutcome> SendAsync(ChannelConfig config,ForwardMessage message,DeliveryRecord part,CancellationToken ct);
}
