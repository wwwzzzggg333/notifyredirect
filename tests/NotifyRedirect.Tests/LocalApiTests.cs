using System.Net;
using System.Net.Http.Headers;
using System.Net.Sockets;
using System.Text;
using NotifyRedirect.LocalApi;
using Xunit;
namespace NotifyRedirect.Tests;
public class LocalApiTests
{
    [Fact] public async Task RequiresTokenAcceptsValidEventAndRejectsInvalidEvent()
    {
        var socket=new TcpListener(IPAddress.Loopback,0);socket.Start();var port=((IPEndPoint)socket.LocalEndpoint).Port;socket.Stop();
        await using var host=new LocalApiHost(port);var accepted=0;await host.StartAsync("test-token",m=>{Interlocked.Increment(ref accepted);return Task.CompletedTask;},default);
        using var client=new HttpClient();var url=$"http://127.0.0.1:{port}/v1/events";
        using var unauthorized=await client.PostAsync(url,new StringContent("{}",Encoding.UTF8,"application/json"));Assert.Equal(HttpStatusCode.Unauthorized,unauthorized.StatusCode);
        client.DefaultRequestHeaders.Authorization=new("Bearer","test-token");
        using var invalid=await client.PostAsync(url,new StringContent("{}",Encoding.UTF8,"application/json"));Assert.Equal(HttpStatusCode.BadRequest,invalid.StatusCode);
        using var valid=await client.PostAsync(url,new StringContent("{\"version\":1,\"eventId\":\"x\",\"source\":\"agent\",\"title\":\"Done\"}",Encoding.UTF8,"application/json"));Assert.Equal(HttpStatusCode.Accepted,valid.StatusCode);Assert.Equal(1,accepted);
        using var large=await client.PostAsync(url,new StringContent(new string('a',140000),Encoding.UTF8,"application/json"));Assert.Equal(HttpStatusCode.RequestEntityTooLarge,large.StatusCode);Assert.Equal(1,accepted);
    }
}
