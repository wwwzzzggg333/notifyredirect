using System.Text.Json;
namespace NotifyRedirect.Core;
public static class EventValidator
{
    public static ForwardMessage Parse(string json)
    {
        using var doc=JsonDocument.Parse(json);var root=doc.RootElement;
        string Text(string name,bool required=false){var s=root.TryGetProperty(name,out var p)?p.GetString():null;if(required&&string.IsNullOrWhiteSpace(s))throw new ArgumentException("缺少 "+name);return s??"";}
        if(!root.TryGetProperty("version",out var version)||version.GetInt32()!=1)throw new ArgumentException("version 必须为 1");
        var source=Text("source",true);var id=Text("eventId",true);var title=Text("title",true);var body=Text("body");
        if(source.Length>80||id.Length>200||title.Length>1000||body.Length>65536)throw new ArgumentException("字段长度超限");
        var state=AgentState.Unknown;
        if(root.TryGetProperty("state",out var st)&&(!Enum.TryParse(st.GetString(),false,out state)||!Enum.IsDefined(state)))throw new ArgumentException("state 无效");
        var when=root.TryGetProperty("occurredAt",out var w)?w.GetDateTimeOffset():DateTimeOffset.UtcNow;
        if(when>DateTimeOffset.UtcNow.AddMinutes(5)||when<DateTimeOffset.UtcNow.AddHours(-24))throw new ArgumentException("通知时间超出范围");
        return new(Guid.NewGuid().ToString("N"),$"agent:{source}:{id}",$"agent:{source}",source,title,body,when,Text("project"),Text("sessionId"),state);
    }
}
