namespace NotifyRedirect.Core;
public enum AgentState { Unknown, TurnEnded, ProcessExited, Succeeded, Failed, WaitingInput }
public record ForwardMessage(string Id,string SourceKey,string AppId,string AppName,string Title,string Body,DateTimeOffset OccurredAt,string? Project=null,string? SessionId=null,AgentState State=AgentState.Unknown);
public record SourceSnapshot(string SourceKey,string AppId,string AppName,DateTimeOffset CreatedAt,IReadOnlyList<string> TextElements);
