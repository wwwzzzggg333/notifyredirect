using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using NotifyRedirect.Core;
using System.Runtime.Versioning;
namespace NotifyRedirect.Infrastructure;
[SupportedOSPlatform("windows")]
public sealed class SecretProtector
{
    public byte[] Protect(string value)=>ProtectedData.Protect(Encoding.UTF8.GetBytes(value),null,DataProtectionScope.CurrentUser);
    public string Unprotect(byte[] value)=>Encoding.UTF8.GetString(ProtectedData.Unprotect(value,null,DataProtectionScope.CurrentUser));
}
public record ChannelSecret(string Webhook,string SigningKey);
[SupportedOSPlatform("windows")]
public sealed class SettingsRepository(SqliteStore store)
{
    readonly SecretProtector protector=new();
    public AppSettings Load(){var data=JsonSerializer.Deserialize<AppSettings>(store.GetValue("settings")??"{}")??new();AppSettingsValidator.Validate(data);return data;}
    public void Save(AppSettings settings){AppSettingsValidator.Validate(settings);store.SetValue("settings",JsonSerializer.Serialize(settings));}
    public void SaveSecret(string id,ChannelSecret secret)=>SaveProtected("secret:"+id,JsonSerializer.Serialize(secret));
    public ChannelSecret GetSecret(string id)=>JsonSerializer.Deserialize<ChannelSecret>(ReadProtected("secret:"+id))??throw new InvalidOperationException("需要重新配置凭据");
    public void SaveProtected(string key,string value)=>store.SetValue(key,Convert.ToBase64String(protector.Protect(value)));
    public string ReadProtected(string key)=>protector.Unprotect(Convert.FromBase64String(store.GetValue(key)??throw new InvalidOperationException("需要重新配置凭据")));
    public string Export(AppSettings settings)=>JsonSerializer.Serialize(settings with {Channels=settings.Channels.Select(c=>c with {SecretRef="",Enabled=false}).ToList(),ApiEnabled=false},new JsonSerializerOptions{WriteIndented=true});
    public AppSettings Import(string json)
    {
        var data=JsonSerializer.Deserialize<AppSettings>(json)??throw new ArgumentException("配置为空");AppSettingsValidator.Validate(data);
        return data with {Channels=data.Channels.Select(c=>c with {Id=Guid.NewGuid().ToString("N"),SecretRef="",Enabled=false}).ToList(),ApiEnabled=false};
    }
}
