namespace NotifyRedirect.Core;
public enum ListenerAccess { Allowed, Denied, Unspecified, Unavailable }
public interface IWindowsNotificationApi
{
    ListenerAccess GetAccess();
    Task<ListenerAccess> RequestAccessAsync(CancellationToken ct);
    Task<IReadOnlyList<SourceSnapshot>> ReadAsync(CancellationToken ct);
}
public record NotificationRead(ListenerAccess Access,IReadOnlyList<SourceSnapshot> Items);
public sealed class PermissionGate(IWindowsNotificationApi api)
{
    public Task<ListenerAccess> RequestAsync(CancellationToken ct)=>api.RequestAccessAsync(ct);
    public async Task<NotificationRead> ReadAsync(CancellationToken ct)
    {
        var access=api.GetAccess();
        if(access!=ListenerAccess.Allowed)return new(access,[]);
        var items=await api.ReadAsync(ct);
        access=api.GetAccess();
        return new(access,access==ListenerAccess.Allowed?items:[]);
    }
}
