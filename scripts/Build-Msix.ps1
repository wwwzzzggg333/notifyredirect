param([string]$Configuration='Release',[string]$OutputDirectory='artifacts',[string]$SigningCertificateThumbprint,[switch]$SelfContained)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/Use-LocalSdk.ps1"
$taskRoot=Split-Path $PSScriptRoot -Parent
$taskOutput=[IO.Path]::GetFullPath((Join-Path $taskRoot $OutputDirectory))
if(-not $taskOutput.StartsWith($taskRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw '输出目录必须位于项目内'}
$taskStage=Join-Path $taskOutput 'msix-stage'
if(Test-Path -LiteralPath $taskStage){Remove-Item -LiteralPath $taskStage -Recurse -Force}
New-Item -ItemType Directory -Force -Path $taskStage | Out-Null
dotnet publish "$taskRoot/src/NotifyRedirect.App/NotifyRedirect.App.csproj" -c $Configuration -r win-x64 --self-contained $($SelfContained.IsPresent.ToString().ToLowerInvariant()) -o $taskStage --no-restore
if($LASTEXITCODE -ne 0){throw '发布失败'}
dotnet publish "$taskRoot/src/NotifyRedirect.Cli/NotifyRedirect.Cli.csproj" -c $Configuration -r win-x64 --self-contained $($SelfContained.IsPresent.ToString().ToLowerInvariant()) -o "$taskStage/tools" --no-restore
if($LASTEXITCODE -ne 0){throw 'CLI 发布失败'}
Copy-Item -LiteralPath "$taskRoot/packaging/AppxManifest.xml" -Destination "$taskStage/AppxManifest.xml"
Copy-Item -LiteralPath "$taskRoot/packaging/Assets" -Destination $taskStage -Recurse -Force
$taskKit=Get-ChildItem 'C:/Program Files (x86)/Windows Kits/10/bin' -Directory | Where-Object {Test-Path (Join-Path $_.FullName 'x64/makeappx.exe')} | Sort-Object Name -Descending | Select-Object -First 1
if(-not $taskKit){throw '需要 Windows SDK MakeAppx 和 SignTool'}
$taskPackage=Join-Path $taskOutput 'NotifyRedirect-0.1.0-x64.msix'
& (Join-Path $taskKit.FullName 'x64/makeappx.exe') pack /d $taskStage /p $taskPackage /o
if($LASTEXITCODE -ne 0){throw 'MSIX 打包失败'}
if($SigningCertificateThumbprint){
 & (Join-Path $taskKit.FullName 'x64/signtool.exe') sign /fd SHA256 /sha1 $SigningCertificateThumbprint $taskPackage
 if($LASTEXITCODE -ne 0){throw '签名失败'}
}else{Write-Output '包尚未签名，不可直接安装。请使用开发签名脚本，或提供个人证书指纹。'}
Write-Output $taskPackage
