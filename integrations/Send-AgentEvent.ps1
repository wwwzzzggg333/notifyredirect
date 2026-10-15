param([ValidateSet('claude-code','codex-cli','cursor','generic')][string]$Agent='generic',[string]$PayloadJson,[string]$CliPath)
# Notification hooks always return zero and never print agent control decisions.
$ErrorActionPreference='Stop'
try {
    [Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
    $OutputEncoding=[Text.UTF8Encoding]::new($false)
    if(-not $PayloadJson){$PayloadJson=[Console]::In.ReadToEnd()}
    $taskInput=$PayloadJson | ConvertFrom-Json
    $taskEvent=@{version=1;eventId=[Guid]::NewGuid().ToString('N');source=$Agent;title='Agent response ready';body='';state='TurnEnded';occurredAt=[DateTimeOffset]::UtcNow.ToString('o')}
    switch($Agent){
        'claude-code' {$taskEvent.body=[string]$taskInput.last_assistant_message;$taskEvent.sessionId=[string]$taskInput.session_id;$taskEvent.project=[string]$taskInput.cwd}
        'codex-cli' {if($taskInput.type -ne 'agent-turn-complete'){exit 0};$taskEvent.body=[string]$taskInput.'last-assistant-message';$taskEvent.sessionId=[string]$taskInput.'thread-id';$taskEvent.project=[string]$taskInput.cwd}
        'cursor' {$taskEvent.body=[string]$taskInput.text;$taskEvent.sessionId=[string]$taskInput.conversation_id}
        'generic' {$taskEvent=$taskInput}
    }
    if(-not $CliPath){$taskPackage=Get-AppxPackage -Name NotifyRedirect.Desktop | Select-Object -First 1;if($taskPackage){$CliPath=Join-Path $taskPackage.InstallLocation 'tools/notifyredirect-cli.exe'}}
    if(-not $CliPath -or -not (Test-Path -LiteralPath $CliPath)){exit 0}
    # Keep within ingress limits; no transcript files are read.
    if($taskEvent.body.Length -gt 30000){$taskEvent.body=$taskEvent.body.Substring(0,30000)+' [truncated]'}
    $taskEvent | ConvertTo-Json -Depth 8 -Compress | & $CliPath send --stdin-json | Out-Null
} catch {}
exit 0
