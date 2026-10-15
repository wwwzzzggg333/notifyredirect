using System;
using System.Threading;
using System.Windows;
namespace NotifyRedirect.App;
public partial class App : Application
{
    private Mutex? mutex;
    protected override void OnStartup(StartupEventArgs e)
    {
        mutex=new Mutex(true,"Local\\NotifyRedirect.Desktop",out var first);
        if(!first){MessageBox.Show("通知转发器已经在运行，请从系统托盘打开。", "通知转发器");Shutdown();return;}
        base.OnStartup(e);
        try { var window=new MainWindow();MainWindow=window;window.Show(); }
        catch(Exception ex){MessageBox.Show("无法启动："+ex.GetType().Name+"。请查看数据目录或重新安装。","通知转发器");Shutdown(1);}
    }
    protected override void OnExit(ExitEventArgs e){mutex?.Dispose();base.OnExit(e);}
}
