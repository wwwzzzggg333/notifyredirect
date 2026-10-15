using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
try
{
    if(args.Length==0||args[0] is "--help" or "-h"){Console.WriteLine("notifyredirect-cli send --source NAME --title TITLE --body BODY [--state Unknown]\nnotifyredirect-cli send --stdin-json\n需要桌面 APP 中启用本机 Agent 入口。首次接入后请勾选该 Agent 来源。");return 0;}
    if(args[0]!="send")throw new ArgumentException("只支持 send 命令");
    string json;
    if(args.Contains("--stdin-json"))json=await Console.In.ReadToEndAsync();
    else
    {
        var fields=new Dictionary<string,string>();for(int i=1;i<args.Length;i+=2){if(i+1>=args.Length)throw new ArgumentException("参数缺少值");fields[args[i]]=args[i+1];}
        json=JsonSerializer.Serialize(new{version=1,eventId=Guid.NewGuid().ToString("N"),source=fields.GetValueOrDefault("--source","custom"),title=fields.GetValueOrDefault("--title","Agent 通知"),body=fields.GetValueOrDefault("--body",""),state=fields.GetValueOrDefault("--state","Unknown"),occurredAt=DateTimeOffset.UtcNow});
    }
    EventValidator.Parse(json);
    var directory=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"NotifyRedirect");
    var path=Path.Combine(directory,"notifyredirect.db");
    if(!File.Exists(path))
    {
        var packages=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"Packages");
        path=Directory.Exists(packages)?Directory.EnumerateDirectories(packages,"NotifyRedirect.Desktop_*").Select(p=>Path.Combine(p,"LocalCache","Local","NotifyRedirect","notifyredirect.db")).FirstOrDefault(File.Exists)??path:path;
    }
    if(!File.Exists(path))throw new InvalidOperationException("请先运行桌面 APP");
    // Read only: CLI must not open SqliteStore, which performs delivery crash recovery.
    using var db=new Microsoft.Data.Sqlite.SqliteConnection(new Microsoft.Data.Sqlite.SqliteConnectionStringBuilder{DataSource=path,Mode=Microsoft.Data.Sqlite.SqliteOpenMode.ReadOnly,Pooling=false}.ToString());db.Open();using var command=db.CreateCommand();command.CommandText="SELECT value FROM config WHERE key='api-token'";
    var encrypted=command.ExecuteScalar() as string??throw new InvalidOperationException("请启用 Agent 入口");var token=new SecretProtector().Unprotect(Convert.FromBase64String(encrypted));
    using var client=new HttpClient(new HttpClientHandler{AllowAutoRedirect=false}){Timeout=TimeSpan.FromSeconds(3)};client.DefaultRequestHeaders.Authorization=new AuthenticationHeaderValue("Bearer",token);
    using var response=await client.PostAsync("http://127.0.0.1:17841/v1/events",new StringContent(json,Encoding.UTF8,"application/json"));
    if((int)response.StatusCode!=202)throw new InvalidOperationException("本机入口未接受事件");Console.WriteLine("事件已接收；是否转发取决于来源和时间规则。");return 0;
}
catch{Console.Error.WriteLine("通知未提交，请检查桌面 APP、Agent 入口及参数。源 Agent 无需停止。");return 1;}
