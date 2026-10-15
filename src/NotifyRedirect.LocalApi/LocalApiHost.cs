using System;
using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Logging;
using NotifyRedirect.Core;
namespace NotifyRedirect.LocalApi;
public sealed class LocalApiHost(int port=17841) : IAsyncDisposable
{
    WebApplication? app;
    public async Task StartAsync(string token,Func<ForwardMessage,Task> onMessage,CancellationToken ct)
    {
        var builder=WebApplication.CreateSlimBuilder();builder.Logging.ClearProviders();
        builder.WebHost.ConfigureKestrel(k=>{k.Listen(IPAddress.Loopback,port);k.Limits.MaxRequestBodySize=131072;});
        app=builder.Build();
        app.MapPost("/v1/events",async (HttpContext context)=>
        {
            var supplied=context.Request.Headers.Authorization.ToString();
            if(!supplied.StartsWith("Bearer ",StringComparison.Ordinal)||!CryptographicOperations.FixedTimeEquals(Encoding.UTF8.GetBytes(supplied[7..]),Encoding.UTF8.GetBytes(token))){context.Response.StatusCode=401;return;}
            if(context.Request.ContentType?.StartsWith("application/json",StringComparison.OrdinalIgnoreCase)!=true){context.Response.StatusCode=415;return;}
            try
            {
                using var reader=new System.IO.StreamReader(context.Request.Body);var json=await reader.ReadToEndAsync(context.RequestAborted);
                var message=EventValidator.Parse(json);await onMessage(message);
                context.Response.StatusCode=202;await context.Response.WriteAsync("{\"accepted\":true,\"delivery\":\"check-history\"}");
            }
            catch(BadHttpRequestException){context.Response.StatusCode=413;}
            catch(Exception ex) when(ex is ArgumentException or System.Text.Json.JsonException or InvalidOperationException or FormatException){context.Response.StatusCode=400;await context.Response.WriteAsync("{\"error\":\"invalid-event\"}");}
        });
        await app.StartAsync(ct);
    }
    public async ValueTask DisposeAsync(){if(app!=null){await app.StopAsync();await app.DisposeAsync();app=null;}}
}
