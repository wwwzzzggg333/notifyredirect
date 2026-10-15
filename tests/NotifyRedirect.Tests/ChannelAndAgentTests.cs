using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using Xunit;
using System.Text;
namespace NotifyRedirect.Tests;
public class ChannelAndAgentTests
{
    [Fact] public void SplitPreservesChineseAndEmoji()
    {
        var text="你好🙂世界🙂";var parts=MessageSplitter.Split(text,8);
        Assert.Equal(text,string.Concat(parts));Assert.All(parts,p=>Assert.InRange(Encoding.UTF8.GetByteCount(p),1,8));
    }
    [Fact] public void RejectsUntrustedWebhook() => Assert.Throws<ArgumentException>(()=>WebhookValidator.Validate(ChannelKind.Feishu,"https://evil.example/open-apis/bot/v2/hook/test"));
    [Fact] public void RejectsNonHttps() => Assert.Throws<ArgumentException>(()=>WebhookValidator.Validate(ChannelKind.WeCom,"http://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=x"));
    [Fact] public void UnknownAgentStateCannotBecomeSuccess()=>Assert.Throws<ArgumentException>(()=>EventValidator.Parse("{\"version\":1,\"eventId\":\"x\",\"source\":\"test\",\"title\":\"Done\",\"body\":\"b\",\"state\":\"Magic\"}"));
    [Fact] public void AgentDefaultsToUnknown()
    {
        var m=EventValidator.Parse("{\"version\":1,\"eventId\":\"x\",\"source\":\"test\",\"title\":\"Done\",\"body\":\"b\"}");
        Assert.Equal(AgentState.Unknown,m.State);Assert.Equal("agent:test",m.AppId);
    }
}
