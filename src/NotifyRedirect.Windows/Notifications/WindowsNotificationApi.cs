using NotifyRedirect.Core;
using Windows.UI.Notifications;
using Windows.UI.Notifications.Management;
namespace NotifyRedirect.Windows;
public sealed class WindowsNotificationApi : IWindowsNotificationApi, IDisposable
{
    static ListenerAccess Convert(UserNotificationListenerAccessStatus status)=>status switch {UserNotificationListenerAccessStatus.Allowed=>ListenerAccess.Allowed,UserNotificationListenerAccessStatus.Denied=>ListenerAccess.Denied,_=>ListenerAccess.Unspecified};
    bool subscribed;
    public event Action? Changed;
    public WindowsNotificationApi()=>Subscribe();
    public void Subscribe()
    {
        if(subscribed||GetAccess()!=ListenerAccess.Allowed)return;
        try{UserNotificationListener.Current.NotificationChanged+=OnChanged;subscribed=true;}catch{}
    }
    void OnChanged(UserNotificationListener sender,UserNotificationChangedEventArgs args)=>Changed?.Invoke();
    public void Dispose(){if(subscribed)UserNotificationListener.Current.NotificationChanged-=OnChanged;}
    public ListenerAccess GetAccess()
    {
        try { return Convert(UserNotificationListener.Current.GetAccessStatus()); }
        catch { return ListenerAccess.Unavailable; }
    }
    public async Task<ListenerAccess> RequestAccessAsync(CancellationToken ct)
    {
        try { return Convert(await UserNotificationListener.Current.RequestAccessAsync().AsTask(ct)); }
        catch(OperationCanceledException) { throw; }
        catch { return ListenerAccess.Unavailable; }
    }
    public async Task<IReadOnlyList<SourceSnapshot>> ReadAsync(CancellationToken ct)
    {
        if(GetAccess()!=ListenerAccess.Allowed)return [];
        var result=new List<SourceSnapshot>();
        var notifications=await UserNotificationListener.Current.GetNotificationsAsync(NotificationKinds.Toast).AsTask(ct);
        foreach(var n in notifications)
        {
            try
            {
                var appId=n.AppInfo.AppUserModelId;
                var key=$"{appId}:{n.Id}:{n.CreationTime.UtcTicks}";
                var binding=n.Notification.Visual.GetBinding(KnownNotificationBindings.ToastGeneric);
                if(binding==null)continue;
                result.Add(new(key,appId,n.AppInfo.DisplayInfo.DisplayName,n.CreationTime,binding.GetTextElements().Select(t=>t.Text).ToArray()));
            }
            catch { /* 一条异常通知不影响其他通知。 */ }
        }
        return result;
    }
}
