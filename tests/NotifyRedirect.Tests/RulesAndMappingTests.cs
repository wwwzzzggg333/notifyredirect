using NotifyRedirect.Core;
using Xunit;
namespace NotifyRedirect.Tests;
public class RulesAndMappingTests
{
    [Theory]
    [InlineData("2026-10-16T18:29:59+08:00",false)]
    [InlineData("2026-10-16T18:30:00+08:00",true)]
    [InlineData("2026-10-17T08:59:59+08:00",true)]
    [InlineData("2026-10-17T09:00:00+08:00",false)]
    [InlineData("2026-10-18T08:00:00+08:00",false)]
    public void CrossMidnightUsesStartingDay(string instant,bool expected)
    {
        var rules = new ForwardingRules { Mode=ScheduleMode.Scheduled, Windows=[new(){Days=[DayOfWeek.Friday],Start=new(18,30),End=new(9,0)}] };
        Assert.Equal(expected,ScheduleEvaluator.IsAllowed(DateTimeOffset.Parse(instant),TimeZoneInfo.FindSystemTimeZoneById("China Standard Time"),rules));
    }
    [Fact] public void PausedOverridesAllDay() => Assert.False(ScheduleEvaluator.IsAllowed(DateTimeOffset.UtcNow,TimeZoneInfo.Utc,new(){Mode=ScheduleMode.Paused}));
    [Fact] public void EqualTimesAreInvalid() => Assert.Throws<ArgumentException>(()=>ScheduleEvaluator.Validate(new(){Mode=ScheduleMode.Scheduled,Windows=[new(){Days=[DayOfWeek.Monday],Start=new(9,0),End=new(9,0)}]}));
    [Fact] public void MapsAllBodyElementsWithoutSuccessInference()
    {
        var m=NotificationMapper.Map(new("key","app","ChatGPT",DateTimeOffset.UtcNow,["标题","正文一","正文二"]))!;
        Assert.Equal("标题",m.Title); Assert.Equal("正文一\n正文二",m.Body); Assert.Equal(AgentState.Unknown,m.State);
    }
    [Fact] public void EmptyTextIsSkipped() => Assert.Null(NotificationMapper.Map(new("key","app","App",DateTimeOffset.UtcNow,[])));
    [Fact] public void EmptySelectionRejectsEverything() => Assert.False(MessageFilter.Accept(Message(),new()));
    [Fact] public void ExcludeWinsOverInclude() => Assert.False(MessageFilter.Accept(Message(),new(){AppIds=["app"],IncludeKeywords=["done"],ExcludeKeywords=["error"]}));
    [Fact] public void TitleOnlyIsAllowed() => Assert.Equal("",NotificationMapper.Map(new("key","app","App",DateTimeOffset.UtcNow,["title"]))!.Body);
    static ForwardMessage Message()=>new("id","key","app","App","Done error","body",DateTimeOffset.UtcNow,null,null,AgentState.Unknown);
}
