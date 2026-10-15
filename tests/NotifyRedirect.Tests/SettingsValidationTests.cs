using System.Text.Json;
using NotifyRedirect.Core;
using Xunit;
namespace NotifyRedirect.Tests;
public class SettingsValidationTests
{
    [Theory]
    [InlineData("{\"Rules\":{\"Mode\":0,\"AppIds\":null}}")]
    [InlineData("{\"Rules\":null}")]
    [InlineData("{\"Channels\":null}")]
    [InlineData("{\"Rules\":{\"Mode\":99}}")]
    public void MalformedCollectionsAndEnumsAreRejected(string json)=>Assert.Throws<ArgumentException>(()=>AppSettingsValidator.Validate(JsonSerializer.Deserialize<AppSettings>(json)!));
}
