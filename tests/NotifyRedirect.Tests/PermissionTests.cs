using NotifyRedirect.Core;
using Xunit;
namespace NotifyRedirect.Tests;
public class PermissionTests
{
    [Fact] public async Task DeniedDoesNotRead()
    {
        var api = new Api { Access = ListenerAccess.Denied };
        var result = await new PermissionGate(api).ReadAsync(default);
        Assert.Equal(ListenerAccess.Denied, result.Access);
        Assert.Equal(0, api.Reads);
    }
    [Fact] public async Task RevokedPermissionIsNotEmptySuccess()
    {
        var api = new Api { Access = ListenerAccess.Allowed };
        var gate = new PermissionGate(api);
        await gate.ReadAsync(default);
        api.Access = ListenerAccess.Denied;
        Assert.Equal(ListenerAccess.Denied, (await gate.ReadAsync(default)).Access);
        Assert.Equal(1, api.Reads);
    }
    [Fact] public async Task UnspecifiedCanRequestAgain()
    {
        var api = new Api { Access = ListenerAccess.Unspecified };
        var gate = new PermissionGate(api);
        Assert.Equal(ListenerAccess.Unspecified, await gate.RequestAsync(default));
        api.Access = ListenerAccess.Allowed;
        Assert.Equal(ListenerAccess.Allowed, await gate.RequestAsync(default));
    }
    private sealed class Api : IWindowsNotificationApi
    {
        public ListenerAccess Access; public int Reads;
        public ListenerAccess GetAccess() => Access;
        public Task<ListenerAccess> RequestAccessAsync(CancellationToken ct) => Task.FromResult(Access);
        public Task<IReadOnlyList<SourceSnapshot>> ReadAsync(CancellationToken ct) { Reads++; return Task.FromResult<IReadOnlyList<SourceSnapshot>>([]); }
    }
}
