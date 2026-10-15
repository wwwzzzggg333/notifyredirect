using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using NotifyRedirect.Core;
using NotifyRedirect.Infrastructure;
using NotifyRedirect.Windows;
using NotifyRedirect.LocalApi;
using Windows.ApplicationModel;
using Forms=System.Windows.Forms;
namespace NotifyRedirect.App;
public partial class MainWindow : Window
{
    readonly SqliteStore store;
    readonly SettingsRepository repository;
    readonly WindowsNotificationApi notificationApi=new();
    readonly PermissionGate permission;
    readonly HttpClient client=new(new HttpClientHandler{AllowAutoRedirect=false}){Timeout=TimeSpan.FromSeconds(20)};
    readonly List<INotificationChannel> channels;
    readonly ForwardingPipeline pipeline;
    readonly DeliveryWorker worker;
    readonly DispatcherTimer timer=new(){Interval=TimeSpan.FromSeconds(5)};
    readonly CancellationTokenSource cancellation=new();
    readonly Forms.NotifyIcon tray;
    readonly Dictionary<string,string> sources=[];
    AppSettings settings;
    LocalApiHost? api;
    bool busy,exiting,loaded;
    DateTimeOffset lastPurge=DateTimeOffset.MinValue;
    public MainWindow()
    {
        InitializeComponent();
        var directory=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"NotifyRedirect");
        store=new(Path.Combine(directory,"notifyredirect.db"));repository=new(store);settings=repository.Load();permission=new(notificationApi);
        var savedSources=store.GetValue("sources");
        if(savedSources!=null)foreach(var pair in System.Text.Json.JsonSerializer.Deserialize<Dictionary<string,string>>(savedSources)??[])sources[pair.Key]=pair.Value;
        channels=Enum.GetValues<ChannelKind>().Select(k=>(INotificationChannel)new WebhookChannel(k,repository.GetSecret,()=>settings.ComputerName,client)).ToList();
        pipeline=new(store,()=>settings,TimeProvider.System);worker=new(store,()=>settings,channels,TimeProvider.System);
        ModeBox.SelectedIndex=(int)settings.Rules.Mode;ComputerName.Text=settings.ComputerName;
        if(settings.Rules.Windows.Count>0)WindowsText.Text=string.Join("\n",settings.Rules.Windows.Select(w=>string.Join(",",w.Days.OrderBy(d=>(int)d).Select(d=>d==DayOfWeek.Sunday?7:(int)d))+" | "+(w.AllDay?"all":$"{w.Start:HH:mm} | {w.End:HH:mm}")));
        IncludeWords.Text=string.Join("\n",settings.Rules.IncludeKeywords);ExcludeWords.Text=string.Join("\n",settings.Rules.ExcludeKeywords);
        AddKnownSources();RenderChannels();RenderHistory();
        tray=new(){Text="通知转发器",Icon=System.Drawing.SystemIcons.Information,Visible=true};
        var menu=new Forms.ContextMenuStrip();menu.Items.Add("打开",null,(_,_)=>Dispatcher.Invoke(OpenWindow));menu.Items.Add("暂停转发",null,(_,_)=>Dispatcher.Invoke(()=>Pause_Click(this,new())));menu.Items.Add("退出",null,(_,_)=>Dispatcher.Invoke(()=>Exit_Click(this,new())));tray.ContextMenuStrip=menu;tray.DoubleClick+=(_,_)=>Dispatcher.Invoke(OpenWindow);
        Closing+=OnClosing;timer.Tick+=async (_,_)=>await RefreshAsync();
        notificationApi.Changed+=()=>Dispatcher.BeginInvoke(async ()=>await RefreshAsync());
        Loaded+=async (_,_)=>{loaded=true;ApiEnabled.IsChecked=settings.ApiEnabled;await RefreshAsync();await LoadStartupAsync();timer.Start();};
    }
    void OpenWindow(){Show();WindowState=WindowState.Normal;Activate();}
    void OnClosing(object? sender,CancelEventArgs e){if(!exiting){e.Cancel=true;Hide();}}
    void Report(string message)=>Feedback.Text=message;
    void Save(){repository.Save(settings);UpdateStatus();}
    void UpdateStatus()
    {
        var access=notificationApi.GetAccess();
        StatusText.Text=access switch {ListenerAccess.Allowed=>"通知读取已授权 · 后台监听中",ListenerAccess.Denied=>"通知读取被拒绝，请在 Windows 设置中重新允许",ListenerAccess.Unspecified=>"尚未授权读取通知",_=>"通知接口不可用：请安装 MSIX 包后运行"};
        ScheduleStatus.Text=$"当前：{(ScheduleEvaluator.IsAllowed(DateTimeOffset.Now,TimeZoneInfo.Local,settings.Rules)?"允许自动转发":"不转发")} · {TimeZoneInfo.Local.DisplayName} · 启用 {settings.Channels.Count(c=>c.Enabled)} 个渠道";
    }
    void RegisterSource(string id,string name)
    {
        if(sources.TryAdd(id,name)){store.SetValue("sources",System.Text.Json.JsonSerializer.Serialize(sources));AddKnownSources();}
    }
    void AddKnownSources()
    {
        var existing=SourceList.Items.OfType<CheckBox>().Select(c=>c.Tag as string).ToHashSet();
        foreach(var pair in sources.OrderBy(p=>p.Value))if(!existing.Contains(pair.Key))SourceList.Items.Add(new CheckBox{Content=pair.Value,Tag=pair.Key,IsChecked=settings.Rules.AppIds.Contains(pair.Key),ToolTip=pair.Key});
    }
    async Task RefreshAsync()
    {
        if(busy||exiting)return;busy=true;
        try
        {
            var result=await permission.ReadAsync(cancellation.Token);
            if(result.Access==ListenerAccess.Allowed)
            {
                var baseline=!store.HasBaseline;
                foreach(var snapshot in result.Items)
                {
                    RegisterSource(snapshot.AppId,snapshot.AppName);
                    var message=NotificationMapper.Map(snapshot);if(message==null)continue;
                    var observed=store.ObserveAndApply(message,baseline,pipeline.Handle);
                    if(observed is Observation.New or Observation.Changed){PreviewText.Text=$"{message.AppName} · {message.Title}\n{message.Body}";}
                }
                if(baseline){store.SetBaseline();Report($"已建立 {result.Items.Count} 条现有通知基线；仅转发之后的新通知。");}
            }
            if(DateTimeOffset.UtcNow-lastPurge>TimeSpan.FromHours(24)){store.Purge(DateTimeOffset.UtcNow);lastPurge=DateTimeOffset.UtcNow;}
            await worker.RunOnceAsync(cancellation.Token);UpdateStatus();RenderHistory();
        }
        catch(OperationCanceledException) when(cancellation.IsCancellationRequested){}
        catch(Exception ex){Report("后台处理失败："+ex.GetType().Name+"；稍后自动重试。");}
        finally {busy=false;}
    }
    async void Authorize_Click(object sender,RoutedEventArgs e)
    {
        var access=await permission.RequestAsync(cancellation.Token);notificationApi.Subscribe();UpdateStatus();
        Report(access==ListenerAccess.Allowed?"授权成功，请前往通知来源勾选应用。":"授权未完成；已拒绝时请在 Windows 设置中允许通知读取。");await RefreshAsync();
    }
    async void Refresh_Click(object sender,RoutedEventArgs e)=>await RefreshAsync();
    void Pause_Click(object sender,RoutedEventArgs e){settings=settings with {Rules=settings.Rules with {Mode=ScheduleMode.Paused}};ModeBox.SelectedIndex=2;Save();Report("已暂停自动转发。");}
    void SaveSources_Click(object sender,RoutedEventArgs e)
    {
        settings=settings with {Rules=settings.Rules with {AppIds=SourceList.Items.OfType<CheckBox>().Where(c=>c.IsChecked==true).Select(c=>(string)c.Tag).ToHashSet(),IncludeKeywords=Lines(IncludeWords.Text),ExcludeKeywords=Lines(ExcludeWords.Text)}};
        TrySave("来源和关键词已保存。");
    }
    static List<string> Lines(string text)=>text.Split(['\r','\n'],StringSplitOptions.RemoveEmptyEntries|StringSplitOptions.TrimEntries).ToList();
    void TrySave(string success){try{Save();Report(success);}catch(ArgumentException ex){Report(ex.Message);}}
    void SaveRules_Click(object sender,RoutedEventArgs e)
    {
        try
        {
            var windows=new List<ScheduleWindow>();
            foreach(var line in Lines(WindowsText.Text))
            {
                var fields=line.Split('|',StringSplitOptions.TrimEntries);
                if(fields.Length<2)throw new ArgumentException("时间段格式错误");
                var days=fields[0].Split(',',StringSplitOptions.TrimEntries).Select(int.Parse).ToArray();
                if(days.Any(d=>d<1||d>7))throw new ArgumentException("星期必须为 1–7");
                var all=fields[1].Equals("all",StringComparison.OrdinalIgnoreCase);
                if(!all&&fields.Length!=3)throw new ArgumentException("需要开始和结束时间");
                windows.Add(new(){Days=days.Select(d=>(DayOfWeek)(d%7)).ToHashSet(),AllDay=all,Start=all?default:TimeOnly.ParseExact(fields[1],"HH:mm",CultureInfo.InvariantCulture),End=all?default:TimeOnly.ParseExact(fields[2],"HH:mm",CultureInfo.InvariantCulture)});
            }
            var rules=settings.Rules with {Mode=(ScheduleMode)ModeBox.SelectedIndex,Windows=windows};ScheduleEvaluator.Validate(rules);
            settings=settings with {Rules=rules,ComputerName=ComputerName.Text.Trim()};Save();Report("时间规则已保存。");
        }
        catch(Exception ex) when(ex is ArgumentException or FormatException or OverflowException){Report("无法保存："+ex.Message);}
    }
    void RenderChannels()=>ChannelList.ItemsSource=settings.Channels;
    sealed record HistoryEntry(DeliveryRecord Part,string Label,string Detail);
    void RenderHistory()
    {
        var selected=(HistoryList.SelectedItem as HistoryEntry)?.Part.Id;
        var entries=store.History().Select(p=>
        {
            var m=store.GetMessage(p.MessageId);var name=settings.Channels.FirstOrDefault(c=>c.Id==p.ChannelId)?.Name??"已删除渠道";
            return new HistoryEntry(p,$"{DeliveryRecord.StatusLabel(p.Status)} · {m?.AppName} · {m?.Title} · {name} · {(p.PartIndex<0?"整条":$"第 {p.PartIndex+1} 段")}", $"来源：{m?.AppName}\n时间：{m?.OccurredAt.ToLocalTime():yyyy-MM-dd HH:mm:ss}\n渠道：{name}\n标题：{m?.Title}\n{m?.Body}\n状态：{p.ErrorCode}");
        }).ToList();HistoryList.ItemsSource=entries;HistoryList.SelectedItem=entries.FirstOrDefault(p=>p.Part.Id==selected);
    }
    void History_Selected(object sender,SelectionChangedEventArgs e)=>HistoryDetail.Text=(HistoryList.SelectedItem as HistoryEntry)?.Detail??"";
    void Channel_Selected(object sender,SelectionChangedEventArgs e)
    {
        if(ChannelList.SelectedItem is not ChannelConfig channel)return;
        ChannelName.Text=channel.Name;ChannelKindBox.SelectedIndex=(int)channel.Kind;ChannelEnabled.IsChecked=channel.Enabled;WebhookBox.Clear();SigningBox.Clear();
    }
    void NewChannel_Click(object sender,RoutedEventArgs e){ChannelList.SelectedItem=null;ChannelName.Clear();WebhookBox.Clear();SigningBox.Clear();ChannelEnabled.IsChecked=true;}
    void SaveChannel_Click(object sender,RoutedEventArgs e)
    {
        try
        {
            var old=ChannelList.SelectedItem as ChannelConfig;var id=old?.Id??Guid.NewGuid().ToString("N");var kind=(ChannelKind)ChannelKindBox.SelectedIndex;
            if(string.IsNullOrWhiteSpace(ChannelName.Text))throw new ArgumentException("请填写渠道名称");
            ChannelSecret? previous=null;if(old!=null){try{previous=repository.GetSecret(id);}catch{}}
            var webhook=WebhookBox.Password.Length>0?WebhookBox.Password:previous?.Webhook??"";
            WebhookValidator.Validate(kind,webhook);
            var secret=new ChannelSecret(webhook,SigningBox.Password.Length>0?SigningBox.Password:previous?.SigningKey??"");repository.SaveSecret(id,secret);
            var channel=new ChannelConfig{Id=id,Name=ChannelName.Text.Trim(),Kind=kind,Enabled=ChannelEnabled.IsChecked==true,SecretRef=id};
            settings=settings with {Channels=settings.Channels.Where(c=>c.Id!=id).Append(channel).ToList()};Save();RenderChannels();ChannelList.SelectedItem=channel;Report("渠道已保存，请发送测试消息并在手机确认。");
        }
        catch(Exception ex){Report(ex is ArgumentException?ex.Message:"保存失败，请重新填写凭据。");}
    }
    async void TestChannel_Click(object sender,RoutedEventArgs e)
    {
        if(ChannelList.SelectedItem is not ChannelConfig channel){Report("请先保存并选择渠道。");return;}
        try
        {
            var m=new ForwardMessage(Guid.NewGuid().ToString("N"),"test","test","NotifyRedirect","测试消息","这是一条手动测试消息，手机收到后说明此渠道连接成功。",DateTimeOffset.Now);
            var senderChannel=channels.First(c=>c.Kind==channel.Kind);var bodies=senderChannel.PrepareParts(channel,m);
            var result=await senderChannel.SendAsync(channel,m,new("test",m.Id,channel.Id,0,bodies[0],DeliveryStatus.Pending,0,null,DateTimeOffset.UtcNow.AddHours(24),null),cancellation.Token);
            Report(DeliveryRecord.StatusLabel(result.Status)+"："+(result.SafeErrorCode??"平台已接受，请检查手机。"));
        }
        catch{Report("测试失败，请检查渠道凭据。");}
    }
    void DeleteChannel_Click(object sender,RoutedEventArgs e){if(ChannelList.SelectedItem is ChannelConfig channel){settings=settings with {Channels=settings.Channels.Where(c=>c.Id!=channel.Id).ToList()};Save();RenderChannels();Report("渠道已删除。");}}
    void History_Click(object sender,RoutedEventArgs e)=>RenderHistory();
    void Retry_Click(object sender,RoutedEventArgs e)
    {
        if(HistoryList.SelectedItem is not HistoryEntry entry)return;var part=entry.Part;
        if(part.Status is not (DeliveryStatus.PermanentFailed or DeliveryStatus.Uncertain or DeliveryStatus.RetryableFailed)){Report("只能重发失败或结果不确定的记录。");return;}
        if(!ScheduleEvaluator.IsAllowed(DateTimeOffset.UtcNow,TimeZoneInfo.Local,settings.Rules)){Report("当前不在转发时间内，请调整时间规则。");return;}
        if(part.ExpiresAt<=DateTimeOffset.UtcNow){Report("消息已超过 24 小时，不再重发。");return;}
        if(part.Status==DeliveryStatus.Uncertain&&MessageBox.Show("平台可能已收到，重发可能产生重复消息。继续？","重发确认",MessageBoxButton.YesNo)!=MessageBoxResult.Yes)return;
        store.Mark(part.Id,DeliveryStatus.Pending,0,null,null);RenderHistory();Report("已加入重发队列。");
    }
    void ClearHistory_Click(object sender,RoutedEventArgs e){if(MessageBox.Show("清空记录也会取消全部待发消息。继续？","清空历史",MessageBoxButton.YesNo)==MessageBoxResult.Yes){store.ClearHistory();RenderHistory();}}
    void Export_Click(object sender,RoutedEventArgs e){var dialog=new Microsoft.Win32.SaveFileDialog{Filter="JSON 配置|*.json",FileName="notifyredirect-config.json"};if(dialog.ShowDialog()==true){File.WriteAllText(dialog.FileName,repository.Export(settings));Report("已导出，不含密钥和消息历史。");}}
    async void Import_Click(object sender,RoutedEventArgs e)
    {
        var dialog=new Microsoft.Win32.OpenFileDialog{Filter="JSON 配置|*.json"};if(dialog.ShowDialog()!=true)return;
        try
        {
            var imported=repository.Import(File.ReadAllText(dialog.FileName));repository.Save(imported);
            if(api!=null){await api.DisposeAsync();api=null;}
            settings=imported;loaded=false;
            ApiEnabled.IsChecked=false;ModeBox.SelectedIndex=(int)settings.Rules.Mode;ComputerName.Text=settings.ComputerName;
            IncludeWords.Text=string.Join("\n",settings.Rules.IncludeKeywords);ExcludeWords.Text=string.Join("\n",settings.Rules.ExcludeKeywords);
            WindowsText.Text=string.Join("\n",settings.Rules.Windows.Select(w=>string.Join(",",w.Days.Select(d=>d==DayOfWeek.Sunday?7:(int)d))+" | "+(w.AllDay?"all":$"{w.Start:HH:mm} | {w.End:HH:mm}")));
            SourceList.Items.Clear();AddKnownSources();RenderChannels();UpdateStatus();loaded=true;
            Report("已导入全部表单；渠道和本机入口已停用，请重新填写密钥。");
        }
        catch{loaded=true;Report("配置格式无效或无法保存，未完成导入。");}
    }
    async Task LoadStartupAsync(){try{var task=await StartupTask.GetAsync("NotifyRedirectStartup");StartupCheck.IsChecked=task.State==StartupTaskState.Enabled;}catch{Report("登录启动需要安装 MSIX 包。");}}
    async void Startup_Changed(object sender,RoutedEventArgs e)
    {
        if(!loaded)return;try{var task=await StartupTask.GetAsync("NotifyRedirectStartup");if(StartupCheck.IsChecked==true){var state=await task.RequestEnableAsync();Report(state==StartupTaskState.Enabled?"登录启动已启用。":"Windows 未允许登录启动，请检查任务管理器的启动应用。");}else{task.Disable();Report("登录启动已停用。");}}catch{Report("登录启动不可用，请安装 MSIX 包。");}
    }
    async void Api_Changed(object sender,RoutedEventArgs e)
    {
        if(!loaded)return;
        try
        {
            if(ApiEnabled.IsChecked==true)
            {
                if(api==null)
                {
                    string token;
                    try{token=repository.ReadProtected("api-token");}
                    catch{token=Convert.ToBase64String(RandomNumberGenerator.GetBytes(32));repository.SaveProtected("api-token",token);}
                    api=new();await api.StartAsync(token,m=>Dispatcher.InvokeAsync(()=>{RegisterSource(m.AppId,m.AppName);store.ObserveAndApply(m,false,pipeline.Handle);PreviewText.Text=m.Title+"\n"+m.Body;}).Task,cancellation.Token);
                }
                settings=settings with {ApiEnabled=true};Save();Report("本机 Agent 入口已开启；来源出现后请勾选并保存。");
            }
            else {if(api!=null){await api.DisposeAsync();api=null;}settings=settings with {ApiEnabled=false};Save();}
        }
        catch{if(api!=null){await api.DisposeAsync();api=null;}settings=settings with {ApiEnabled=false};Save();Report("Agent 入口启动失败，可能端口被占用或密钥损坏。");}
    }
    async void Exit_Click(object sender,RoutedEventArgs e)
    {
        if(exiting)return;exiting=true;timer.Stop();cancellation.Cancel();
        if(api!=null)await api.DisposeAsync();while(busy)await Task.Delay(30);
        notificationApi.Dispose();tray.Visible=false;tray.Dispose();client.Dispose();store.Dispose();cancellation.Dispose();Close();Application.Current.Shutdown();
    }
}
