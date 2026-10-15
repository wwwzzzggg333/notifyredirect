using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using Xunit;
namespace NotifyRedirect.Tests;
public class SecretProtectionTests
{
    [Fact] public void SecretRoundTripsAndCorruptionIsDetected()
    {
        if(!OperatingSystem.IsWindows())return;
        var protector=new SecretProtector();var encrypted=protector.Protect("private webhook");
        Assert.Equal("private webhook",protector.Unprotect(encrypted));encrypted[encrypted.Length/2]^=0xFF;
        Assert.Throws<System.Security.Cryptography.CryptographicException>(()=>{if(OperatingSystem.IsWindows())protector.Unprotect(encrypted);});
    }
    [Fact] public void ExportAndImportExcludeSecretsAndDisableDestinations()
    {
        if(!OperatingSystem.IsWindows())return;
        using var store=new SqliteStore(":memory:");var repository=new SettingsRepository(store);
        repository.SaveSecret("one",new("https://secret.example","signing-secret"));
        var settings=new AppSettings{Channels=[new(){Id="one",Name="Feishu",Enabled=true,SecretRef="one"}],ApiEnabled=true};
        var json=repository.Export(settings);Assert.DoesNotContain("secret.example",json);Assert.DoesNotContain("signing-secret",json);
        var imported=repository.Import(json);Assert.False(imported.ApiEnabled);Assert.All(imported.Channels,c=>{Assert.False(c.Enabled);Assert.Equal("",c.SecretRef);Assert.NotEqual("one",c.Id);});
    }
}
