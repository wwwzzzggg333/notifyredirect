namespace NotifyRedirect.Core;
public enum ScheduleMode { Always, Scheduled, Paused }
public record ScheduleWindow
{
    public HashSet<DayOfWeek> Days { get; init; }=[];
    public TimeOnly Start { get; init; }=new(18,30);
    public TimeOnly End { get; init; }=new(9,0);
    public bool AllDay { get; init; }
}
public record ForwardingRules
{
    public ScheduleMode Mode { get; init; }=ScheduleMode.Paused;
    public List<ScheduleWindow> Windows { get; init; }=[];
    public HashSet<string> AppIds { get; init; }=[];
    public List<string> IncludeKeywords { get; init; }=[];
    public List<string> ExcludeKeywords { get; init; }=[];
}
