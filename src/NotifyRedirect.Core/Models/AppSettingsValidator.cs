namespace NotifyRedirect.Core;
public static class AppSettingsValidator
{
    public static void Validate(AppSettings data)
    {
        if(data==null||data.Rules==null||data.Channels==null||string.IsNullOrWhiteSpace(data.ComputerName))throw new ArgumentException("配置结构不完整");
        var r=data.Rules;
        if(!Enum.IsDefined(r.Mode)||r.AppIds==null||r.Windows==null||r.IncludeKeywords==null||r.ExcludeKeywords==null)throw new ArgumentException("通知规则无效");
        if(r.AppIds.Any(string.IsNullOrWhiteSpace)||r.IncludeKeywords.Any(k=>k==null)||r.ExcludeKeywords.Any(k=>k==null))throw new ArgumentException("通知来源或关键词无效");
        if(r.Windows.Any(w=>w==null||w.Days==null||w.Days.Any(d=>!Enum.IsDefined(d))))throw new ArgumentException("时间段无效");
        if(data.Channels.Any(c=>c==null||!Enum.IsDefined(c.Kind)||string.IsNullOrWhiteSpace(c.Id)||string.IsNullOrWhiteSpace(c.Name))||data.Channels.Select(c=>c.Id).Distinct().Count()!=data.Channels.Count)throw new ArgumentException("渠道配置无效");
        ScheduleEvaluator.Validate(r);
    }
}
