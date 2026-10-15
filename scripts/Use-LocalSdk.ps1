$taskRoot = Split-Path $PSScriptRoot -Parent
$env:DOTNET_CLI_HOME = Join-Path $taskRoot '.tools/cli-home'
$env:NUGET_PACKAGES = Join-Path $taskRoot '.tools/packages'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_NOLOGO = '1'
$env:DOTNET_ROOT = Join-Path $taskRoot '.tools/dotnet'
$env:PATH = $env:DOTNET_ROOT + ';' + $env:PATH
