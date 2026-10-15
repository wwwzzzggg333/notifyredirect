namespace NotifyRedirect.Core;
public static class ScheduleEvaluator
{
    public static void Validate(ForwardingRules rules)
    {
        if(!Enum.IsDefined(rules.Mode))throw new ArgumentException("无效转发模式");
        foreach(var w in rules.Windows)
        {
            if(w.Days.Count==0||w.Days.Any(d=>!Enum.IsDefined(d)))throw new ArgumentException("请选择星期");
            if(!w.AllDay&&w.Start==w.End)throw new ArgumentException("开始和结束时间不能相同，请使用全天选项");
        }
        if(rules.Mode==ScheduleMode.Scheduled&&rules.Windows.Count==0)throw new ArgumentException("请添加时间段");
    }
    public static bool IsAllowed(DateTimeOffset instant,TimeZoneInfo zone,ForwardingRules rules)
    {
        if(rules.Mode==ScheduleMode.Paused)return false;
        if(rules.Mode==ScheduleMode.Always)return true;
        var local=TimeZoneInfo.ConvertTime(instant,zone);var time=TimeOnly.FromDateTime(local.DateTime);
        return rules.Windows.Any(w=>w.AllDay?w.Days.Contains(local.DayOfWeek):w.Start<w.End?
            w.Days.Contains(local.DayOfWeek)&&time>=w.Start&&time<w.End:
            w.Start!=w.End&&((time>=w.Start&&w.Days.Contains(local.DayOfWeek))||(time<w.End&&w.Days.Contains(local.AddDays(-1).DayOfWeek))));
    }
}
