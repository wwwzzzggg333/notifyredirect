using System.Net;
using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using Xunit;
namespace NotifyRedirect.Tests;
public class HttpChannelTests
{
    [Theory]
    [InlineData(200,"{\"code\":0}",DeliveryStatus.Succeeded)]
    [InlineData(200,"{\"code\":19021}",DeliveryStatus.PermanentFailed)]
    [InlineData(429,"{}",DeliveryStatus.RetryableFailed)]
    [InlineData(503,"{}",DeliveryStatus.RetryableFailed)]
    [InlineData(401,"{}",DeliveryStatus.PermanentFailed)]
    [InlineData(200,"not-json",DeliveryStatus.Uncertain)]
    public async Task ClassifiesHttpAndBusinessErrors(int code,string body,DeliveryStatus expected)
    {
        var channel=new WebhookChannel(ChannelKind.Feishu,_=>new("https://open.feishu.cn/open-apis/bot/v2/hook/test",""),()=>"PC",new HttpClient(new Handler((HttpStatusCode)code,body)));
        var message=new ForwardMessage("id","key","app","ChatGPT","标题","正文",DateTimeOffset.UtcNow);
        var outcome=await channel.SendAsync(new(){Kind=ChannelKind.Feishu},message,new("p","id","ch",0,"正文",DeliveryStatus.Pending,0,null,DateTimeOffset.UtcNow.AddHours(24),null),default);
        Assert.Equal(expected,outcome.Status);
    }
    [Fact] public async Task TimeoutIsUncertain()
    {
        var channel=new WebhookChannel(ChannelKind.Feishu,_=>new("https://open.feishu.cn/open-apis/bot/v2/hook/test",""),()=>"PC",new HttpClient(new TimeoutHandler()));
        var outcome=await channel.SendAsync(new(),new("i","k","a","App","t","b",DateTimeOffset.UtcNow),new("p","i","c",0,"b",DeliveryStatus.Pending,0,null,DateTimeOffset.UtcNow.AddHours(24),null),default);
        Assert.Equal(DeliveryStatus.Uncertain,outcome.Status);
    }
    sealed class Handler(HttpStatusCode code,string body):HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken ct)=>Task.FromResult(new HttpResponseMessage(code){Content=new StringContent(body)});
    }
    sealed class TimeoutHandler:HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken ct)=>throw new TaskCanceledException("timed out");
    }
}
