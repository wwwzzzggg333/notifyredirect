namespace NotifyRedirect.Core;
public static class RetryPolicy
{
    static readonly TimeSpan[] Delays=[TimeSpan.FromSeconds(30),TimeSpan.FromMinutes(2),TimeSpan.FromMinutes(10),TimeSpan.FromMinutes(30),TimeSpan.FromHours(2)];
    public static TimeSpan? NextDelay(int retryCount,TimeSpan? retryAfter)
    {
        if(retryCount<0||retryCount>=Delays.Length)return null;
        return retryAfter>Delays[retryCount]?retryAfter:Delays[retryCount];
    }
}
