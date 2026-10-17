param([ValidateSet('claude-code','codex-cli','cursor','generic')][string]$Agent='generic',[string]$PayloadJson,[string]$CliPath)
# Notification hooks always return zero and never print agent control decisions.
$ErrorActionPreference='Stop'
try {
    [Console]::InputEncoding=[Text.UTF8Encoding]::new($false)
    $OutputEncoding=[Text.UTF8Encoding]::new($false)
    if(-not $PayloadJson){$PayloadJson=[Console]::In.ReadToEnd()}
    $taskInput=$PayloadJson | ConvertFrom-Json
    $taskEvent=@{version=1;event_id=[Guid]::NewGuid().ToString('N');source=$Agent;title='Agent response ready';body='';state='turn_ended';occurred_at=[DateTimeOffset]::UtcNow.ToString('o')}
    switch($Agent){
        'claude-code' {$taskEvent.body=[string]$taskInput.last_assistant_message;$taskEvent.session_id=[string]$taskInput.session_id;$taskEvent.project=[string]$taskInput.cwd}
        'codex-cli' {if($taskInput.type -ne 'agent-turn-complete'){exit 0};$taskEvent.body=[string]$taskInput.'last-assistant-message';$taskEvent.session_id=[string]$taskInput.'thread-id';$taskEvent.project=[string]$taskInput.cwd}
        'cursor' {$taskEvent.body=[string]$taskInput.text;$taskEvent.session_id=[string]$taskInput.conversation_id}
        'generic' {$taskEvent=$taskInput}
    }
    if(-not $CliPath){$taskPackage=Get-AppxPackage -Name NotifyRedirect.Native | Select-Object -First 1;if($taskPackage){$CliPath=Join-Path $taskPackage.InstallLocation 'tools/notifyredirect-cli.exe'}}
    if(-not $CliPath -or -not (Test-Path -LiteralPath $CliPath)){exit 0}
    # Keep within ingress limits; no transcript files are read.
    if($taskEvent.body.Length -gt 20000){$taskText=[string]$taskEvent.body;$taskEnd=20000;if([char]::IsHighSurrogate($taskText[$taskEnd-1])){$taskEnd--};$taskEvent.body=$taskText.Substring(0,$taskEnd)+' [truncated]'}
    $taskEvent | ConvertTo-Json -Depth 8 -Compress | & $CliPath send --stdin-json | Out-Null
} catch {}
exit 0
