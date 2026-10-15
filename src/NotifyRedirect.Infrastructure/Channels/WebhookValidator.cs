using NotifyRedirect.Core;
namespace NotifyRedirect.Infrastructure;
public static class WebhookValidator
{
    public static Uri Validate(ChannelKind kind,string value)
    {
        if(!Uri.TryCreate(value,UriKind.Absolute,out var uri)||uri.Scheme!="https"||!uri.IsDefaultPort||uri.UserInfo.Length!=0||uri.Fragment.Length!=0)throw new ArgumentException("需要平台官方 HTTPS Webhook 地址");
        var allowed=kind switch {
            ChannelKind.Feishu=>uri.Host=="open.feishu.cn"&&uri.AbsolutePath.StartsWith("/open-apis/bot/v2/hook/",StringComparison.Ordinal)&&uri.AbsolutePath.Length>23,
            ChannelKind.DingTalk=>uri.Host=="oapi.dingtalk.com"&&uri.AbsolutePath=="/robot/send"&&uri.Query.Contains("access_token=",StringComparison.Ordinal),
            ChannelKind.WeCom=>uri.Host=="qyapi.weixin.qq.com"&&uri.AbsolutePath=="/cgi-bin/webhook/send"&&uri.Query.Contains("key=",StringComparison.Ordinal),
            _=>false};
        if(!allowed)throw new ArgumentException("地址与所选平台不符");return uri;
    }
}
