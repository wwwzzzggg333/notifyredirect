namespace NotifyRedirect.Core;
public static class NotificationMapper
{
    public static ForwardMessage? Map(SourceSnapshot snapshot)
    {
        if(snapshot.TextElements.All(string.IsNullOrWhiteSpace))return null;
        return new(Guid.NewGuid().ToString("N"),snapshot.SourceKey,snapshot.AppId,snapshot.AppName,snapshot.TextElements.FirstOrDefault()??"",string.Join("\n",snapshot.TextElements.Skip(1)),snapshot.CreatedAt);
    }
}
