using Microsoft.Data.Sqlite;
using NotifyRedirect.Core;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
namespace NotifyRedirect.Infrastructure;
public sealed class SqliteStore : IDisposable
{
    private readonly SqliteConnection db;
    private readonly object gate=new();
    private bool applyingObservation;
    public SqliteStore(string path)
    {
        if(path!=":memory:")Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        db=new(new SqliteConnectionStringBuilder{DataSource=path,Pooling=false}.ToString());db.Open();
        CheckVersion(Convert.ToInt32(Scalar("PRAGMA user_version")));
        Exec("""
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS ledger(source TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, touched TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY, source TEXT NOT NULL, payload TEXT NOT NULL, received TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS delivery(id TEXT PRIMARY KEY,message TEXT NOT NULL,channel TEXT NOT NULL,part INTEGER NOT NULL DEFAULT -1,body TEXT,status INTEGER NOT NULL,retries INTEGER NOT NULL DEFAULT 0,next TEXT,expires TEXT NOT NULL,error TEXT);
        CREATE INDEX IF NOT EXISTS delivery_due ON delivery(status,next);
        CREATE TABLE IF NOT EXISTS config(key TEXT PRIMARY KEY,value TEXT NOT NULL);
        PRAGMA user_version=1;
        """);
        Exec("UPDATE delivery SET status=$s,error='进程退出前发送结果未确认' WHERE status=$old",("$s",(int)DeliveryStatus.Uncertain),("$old",(int)DeliveryStatus.InFlight));
    }
    public void CheckVersion(int version){if(version>1)throw new InvalidOperationException("数据版本高于 APP 版本，请安装较新版本；原数据未删除");}
    SqliteCommand Command(string sql,params (string,object?)[] parameters)
    {
        var c=db.CreateCommand();c.CommandText=sql;foreach(var (k,v) in parameters)c.Parameters.AddWithValue(k,v??DBNull.Value);return c;
    }
    void Exec(string sql,params (string,object?)[] parameters){using var c=Command(sql,parameters);c.ExecuteNonQuery();}
    object? Scalar(string sql,params (string,object?)[] parameters){using var c=Command(sql,parameters);return c.ExecuteScalar();}
    static string Stamp(DateTimeOffset time)=>time.ToUniversalTime().ToString("O");
    public Observation Observe(ForwardMessage m,bool baseline)
    {
        lock(gate)
        {
            var fingerprint=Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(m.Title+"\0"+m.Body)));
            var old=Scalar("SELECT fingerprint FROM ledger WHERE source=$key",("$key",m.SourceKey)) as string;
            Exec("INSERT INTO ledger VALUES($k,$f,$t) ON CONFLICT(source) DO UPDATE SET fingerprint=$f,touched=$t",("$k",m.SourceKey),("$f",fingerprint),("$t",Stamp(DateTimeOffset.UtcNow)));
            return baseline?Observation.Baseline:old==null?Observation.New:old==fingerprint?Observation.Duplicate:Observation.Changed;
        }
    }
    public Observation ObserveAndApply(ForwardMessage m,bool baseline,Action<ForwardMessage> apply)
    {
        lock(gate)
        {
            using var transaction=db.BeginTransaction();applyingObservation=true;
            try
            {
                var result=Observe(m,baseline);
                if(result is Observation.New or Observation.Changed)apply(m);
                transaction.Commit();return result;
            }
            finally {applyingObservation=false;}
        }
    }
    public bool HasBaseline => GetValue("baseline") == "yes";
    public void SetBaseline()=>SetValue("baseline","yes");
    public void Enqueue(ForwardMessage m,IReadOnlyList<ChannelConfig> channels,DateTimeOffset now)
    {
        lock(gate)
        {
            using var transaction=applyingObservation?null:db.BeginTransaction();
            var existing=Scalar("SELECT m.id FROM messages m WHERE m.source=$s AND EXISTS(SELECT 1 FROM delivery d WHERE d.message=m.id) AND NOT EXISTS(SELECT 1 FROM delivery d WHERE d.message=m.id AND (d.status NOT IN(0,7) OR d.retries<>0)) ORDER BY received DESC LIMIT 1",("$s",m.SourceKey)) as string;
            if(existing!=null)
            {
                m=m with {Id=existing};Exec("UPDATE messages SET payload=$p WHERE id=$i",("$p",JsonSerializer.Serialize(m)),("$i",existing));
                var received=DateTimeOffset.Parse((string)Scalar("SELECT received FROM messages WHERE id=$i",("$i",existing))!);
                Exec("DELETE FROM delivery WHERE message=$i",("$i",existing));
                foreach(var channel in channels.Where(c=>c.Enabled))Exec("INSERT INTO delivery(id,message,channel,status,expires) VALUES($i,$m,$c,0,$e)",("$i",Guid.NewGuid().ToString("N")),("$m",existing),("$c",channel.Id),("$e",Stamp(received.AddHours(24))));
            }
            else
            {
                Exec("INSERT INTO messages VALUES($i,$s,$p,$t)",("$i",m.Id),("$s",m.SourceKey),("$p",JsonSerializer.Serialize(m)),("$t",Stamp(now)));
                foreach(var channel in channels.Where(c=>c.Enabled))Exec("INSERT INTO delivery(id,message,channel,status,expires) VALUES($i,$m,$c,0,$e)",("$i",Guid.NewGuid().ToString("N")),("$m",m.Id),("$c",channel.Id),("$e",Stamp(now.AddHours(24))));
            }
            transaction?.Commit();
        }
    }
    public void ExpandParts(DeliveryRecord parent,IReadOnlyList<string> bodies)
    {
        lock(gate)
        {
            using var transaction=db.BeginTransaction();
            for(int i=0;i<bodies.Count;i++)Exec("INSERT INTO delivery(id,message,channel,part,body,status,expires) VALUES($i,$m,$c,$p,$b,0,$e)",("$i",Guid.NewGuid().ToString("N")),("$m",parent.MessageId),("$c",parent.ChannelId),("$p",i),("$b",bodies[i]),("$e",Stamp(parent.ExpiresAt)));
            Mark(parent.Id,DeliveryStatus.Expanded,0,null,null);transaction.Commit();
        }
    }
    public ForwardMessage? GetMessage(string id){lock(gate){var json=Scalar("SELECT payload FROM messages WHERE id=$i",("$i",id)) as string;return json==null?null:JsonSerializer.Deserialize<ForwardMessage>(json);}}
    static DeliveryRecord Read(SqliteDataReader r)=>new(r.GetString(0),r.GetString(1),r.GetString(2),r.GetInt32(3),r.IsDBNull(4)?null:r.GetString(4),(DeliveryStatus)r.GetInt32(5),r.GetInt32(6),r.IsDBNull(7)?null:DateTimeOffset.Parse(r.GetString(7)),DateTimeOffset.Parse(r.GetString(8)),r.IsDBNull(9)?null:r.GetString(9));
    public IReadOnlyList<DeliveryRecord> Due(DateTimeOffset now)
    {
        lock(gate){using var c=Command("SELECT * FROM delivery WHERE status IN (0,3) AND (next IS NULL OR next<=$t) ORDER BY rowid LIMIT 20",("$t",Stamp(now)));using var r=c.ExecuteReader();var rows=new List<DeliveryRecord>();while(r.Read())rows.Add(Read(r));return rows;}
    }
    public IReadOnlyList<DeliveryRecord> History()
    {
        lock(gate){using var c=Command("SELECT * FROM delivery WHERE status<>7 ORDER BY rowid DESC LIMIT 200");using var r=c.ExecuteReader();var rows=new List<DeliveryRecord>();while(r.Read())rows.Add(Read(r));return rows;}
    }
    public void Mark(string id,DeliveryStatus status,int retries,DateTimeOffset? next,string? error)
    {
        lock(gate)Exec("UPDATE delivery SET status=$s,retries=$r,next=$n,error=$e WHERE id=$i",("$s",(int)status),("$r",retries),("$n",next.HasValue?Stamp(next.Value):null),("$e",error),("$i",id));
    }
    public string? GetValue(string key){lock(gate)return Scalar("SELECT value FROM config WHERE key=$k",("$k",key)) as string;}
    public void SetValue(string key,string value){lock(gate)Exec("INSERT INTO config VALUES($k,$v) ON CONFLICT(key) DO UPDATE SET value=$v",("$k",key),("$v",value));}
    public void Purge(DateTimeOffset now)
    {
        lock(gate)
        {
            Exec("UPDATE delivery SET status=6,error='超过 24 小时' WHERE status IN(0,1,3) AND expires<=$t",("$t",Stamp(now)));
            Exec("UPDATE messages SET payload=json_set(payload,'$.Body','') WHERE received<=$t",("$t",Stamp(now.AddDays(-7))));
            Exec("UPDATE delivery SET body=NULL WHERE message IN(SELECT id FROM messages WHERE received<=$t)",("$t",Stamp(now.AddDays(-7))));
            Exec("DELETE FROM delivery WHERE message IN(SELECT id FROM messages WHERE received<=$t)",("$t",Stamp(now.AddDays(-30))));
            Exec("DELETE FROM messages WHERE received<=$t",("$t",Stamp(now.AddDays(-30))));
            Exec("DELETE FROM ledger WHERE touched<=$t",("$t",Stamp(now.AddDays(-30))));
        }
    }
    public void ClearHistory(){lock(gate)Exec("DELETE FROM delivery;DELETE FROM messages;");}
    public void Dispose()=>db.Dispose();
}
