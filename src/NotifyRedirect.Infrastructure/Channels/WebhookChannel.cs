using System.Net;
using System.Net.Http.Json;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using NotifyRedirect.Core;
namespace NotifyRedirect.Infrastructure;
public sealed class WebhookChannel(ChannelKind kind,Func<string,ChannelSecret> secrets,Func<string> computerName,HttpClient client) : INotificationChannel
{
    public ChannelKind Kind=>kind;
    public IReadOnlyList<string> PrepareParts(ChannelConfig config,ForwardMessage message)
    {
        var state=message.State switch {AgentState.TurnEnded=>"一轮回复结束",AgentState.ProcessExited=>"进程退出",AgentState.Succeeded=>"任务成功",AgentState.Failed=>"任务失败",AgentState.WaitingInput=>"等待输入",_=>"通知"};
        var text=$"[{state}] {message.AppName}\n{message.Title}\n{message.Body}\n电脑：{computerName()}\n时间：{message.OccurredAt.ToLocalTime():yyyy-MM-dd HH:mm:ss}";
        if(!string.IsNullOrWhiteSpace(message.Project))text+=$"\n项目：{message.Project}";
        text=MessageSplitter.Limit(text,65536);
        // Conservative plain-text budgets. Actual account/platform limits must be confirmed during acceptance.
        var parts=MessageSplitter.Split(text,kind==ChannelKind.WeCom?1800:15000);
        return parts.Count==1?parts:parts.Select((p,i)=>$"[{i+1}/{parts.Count}]\n{p}").ToArray();
    }
    public async Task<SendOutcome> SendAsync(ChannelConfig config,ForwardMessage message,DeliveryRecord part,CancellationToken ct)
    {
        var secret=secrets(config.Id);var uri=WebhookValidator.Validate(kind,secret.Webhook);var text=part.PartBody??throw new InvalidOperationException("未分段");
        object payload;
        if(kind==ChannelKind.Feishu)
        {
            var stamp=DateTimeOffset.UtcNow.ToUnixTimeSeconds().ToString();
            if(secret.SigningKey.Length==0)payload=new{msg_type="text",content=new{text}};
            else {using var hmac=new HMACSHA256(Encoding.UTF8.GetBytes(stamp+"\n"+secret.SigningKey));payload=new{msg_type="text",content=new{text},timestamp=stamp,sign=Convert.ToBase64String(hmac.ComputeHash([]))};}
        }
        else
        {
            payload=new{msgtype="text",text=new{content=text}};
            if(kind==ChannelKind.DingTalk&&secret.SigningKey.Length>0)
            {
                var stamp=DateTimeOffset.UtcNow.ToUnixTimeMilliseconds().ToString();using var hmac=new HMACSHA256(Encoding.UTF8.GetBytes(secret.SigningKey));
                var sign=Uri.EscapeDataString(Convert.ToBase64String(hmac.ComputeHash(Encoding.UTF8.GetBytes(stamp+"\n"+secret.SigningKey))));
                uri=new(uri+"&timestamp="+stamp+"&sign="+sign);
            }
        }
        using var timeout=CancellationTokenSource.CreateLinkedTokenSource(ct);timeout.CancelAfter(TimeSpan.FromSeconds(15));
        try
        {
            using var response=await client.PostAsJsonAsync(uri,payload,timeout.Token);
            TimeSpan? retry=response.Headers.RetryAfter?.Delta;
            if(response.Headers.RetryAfter?.Date is { } date)retry=date-DateTimeOffset.UtcNow;
            if(response.StatusCode==HttpStatusCode.TooManyRequests)return new(DeliveryStatus.RetryableFailed,"平台限流",retry);
            if((int)response.StatusCode>=500)return new(DeliveryStatus.RetryableFailed,"平台暂时不可用");
            if(!response.IsSuccessStatusCode)return new(DeliveryStatus.PermanentFailed,$"平台拒绝请求 HTTP {(int)response.StatusCode}");
            var body=await response.Content.ReadAsStringAsync(timeout.Token);
            if(body.Length>65536)return new(DeliveryStatus.Uncertain,"平台响应异常，结果未确认");
            using var doc=JsonDocument.Parse(body);
            int code;
            if(doc.RootElement.TryGetProperty("code",out var prop)||doc.RootElement.TryGetProperty("errcode",out prop)||doc.RootElement.TryGetProperty("StatusCode",out prop))code=prop.GetInt32();
            else return new(DeliveryStatus.Uncertain,"平台响应未包含结果码");
            if(code==0)return new(DeliveryStatus.Succeeded);
            if(code is 99991400 or 130101 or 45009)return new(DeliveryStatus.RetryableFailed,$"平台限流（{code}）",TimeSpan.FromMinutes(1));
            return new(DeliveryStatus.PermanentFailed,$"平台业务错误（{code}），请检查凭据和安全规则");
        }
        catch(OperationCanceledException) when(!ct.IsCancellationRequested){return new(DeliveryStatus.Uncertain,"请求超时，可能已送达，请先检查手机");}
        catch(HttpRequestException){return new(DeliveryStatus.RetryableFailed,"网络连接失败");}
        catch(JsonException){return new(DeliveryStatus.Uncertain,"无法确认平台响应");}
    }
}
