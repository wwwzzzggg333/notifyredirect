namespace NotifyRedirect.Core;
public static class MessageFilter
{
    public static bool Accept(ForwardMessage message,ForwardingRules rules)
    {
        if(!rules.AppIds.Contains(message.AppId))return false;
        var text=message.Title+"\n"+message.Body;
        bool Matches(string word)=>!string.IsNullOrWhiteSpace(word)&&text.Contains(word,StringComparison.OrdinalIgnoreCase);
        return !rules.ExcludeKeywords.Any(Matches)&&(rules.IncludeKeywords.Count==0||rules.IncludeKeywords.Any(Matches));
    }
}
