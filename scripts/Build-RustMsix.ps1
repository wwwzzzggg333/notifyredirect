param([string]$OutputDirectory='artifacts/rust',[switch]$Offline)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$taskAllowed=[IO.Path]::GetFullPath((Join-Path $taskRoot 'artifacts/rust'))
$taskOutput=[IO.Path]::GetFullPath((Join-Path $taskRoot $OutputDirectory))
if($taskOutput -ne $taskAllowed -and -not $taskOutput.StartsWith($taskAllowed+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw '输出目录必须位于 artifacts/rust 内'}
$env:CARGO_TARGET_DIR=Join-Path $taskRoot '.tools/rust-build'
$taskArgs=@('build','--manifest-path',"$taskRoot/native/Cargo.toml",'--workspace','--release','--locked','--target','x86_64-pc-windows-msvc','--config','target.x86_64-pc-windows-msvc.rustflags=["-C","target-feature=+crt-static"]')
if($Offline){$taskArgs+='--offline'}
& cargo @taskArgs
if($LASTEXITCODE -ne 0){throw 'Rust Release 构建失败'}
$taskStage=[IO.Path]::GetFullPath((Join-Path $taskOutput 'msix-stage'))
if(-not $taskStage.StartsWith($taskAllowed+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw '暂存路径无效'}
if(Test-Path -LiteralPath $taskStage){Remove-Item -LiteralPath $taskStage -Recurse -Force}
New-Item -ItemType Directory -Force -Path "$taskStage/tools" | Out-Null
$taskRelease=Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc/release'
Copy-Item -LiteralPath "$taskRelease/NotifyRedirect.exe" -Destination $taskStage
Copy-Item -LiteralPath "$taskRelease/notifyredirect-cli.exe" -Destination "$taskStage/tools"
Copy-Item -LiteralPath "$taskRoot/packaging/rust/AppxManifest.xml" -Destination "$taskStage/AppxManifest.xml"
Copy-Item -LiteralPath "$taskRoot/packaging/Assets" -Destination $taskStage -Recurse
$taskFiles=@(Get-ChildItem -LiteralPath $taskStage -File -Recurse)
$taskSize=($taskFiles | Measure-Object Length -Sum).Sum
if($taskSize -gt 20MB){throw "未压缩内容超过目标 20 MiB：$taskSize 字节"}
if($taskFiles.Name -match '(?i)coreclr|hostfxr|presentationframework|webview|chromium|libssl'){throw '发现不应捆绑的运行时'}
[xml]$taskManifest=Get-Content -LiteralPath "$taskStage/AppxManifest.xml" -Raw
if($taskManifest.Package.Identity.Name -ne 'NotifyRedirect.Native' -or $taskManifest.Package.Identity.Version -ne '1.0.0.0'){throw '包身份或版本错误'}
$taskKit=Get-ChildItem 'C:/Program Files (x86)/Windows Kits/10/bin' -Directory | Where-Object {Test-Path (Join-Path $_.FullName 'x64/makeappx.exe')} | Sort-Object Name -Descending | Select-Object -First 1
if(-not $taskKit){throw '需要 Windows SDK MakeAppx'}
$taskPackage=Join-Path $taskOutput 'NotifyRedirect-Native-1.0.0-x64.msix'
& (Join-Path $taskKit.FullName 'x64/makeappx.exe') pack /d $taskStage /p $taskPackage /o
if($LASTEXITCODE -ne 0){throw 'Rust MSIX 打包失败'}
$taskPackageSize=(Get-Item -LiteralPath $taskPackage).Length
if($taskPackageSize -gt 15MB){throw "MSIX 超过目标 15 MiB：$taskPackageSize 字节"}
$taskReport=[ordered]@{app_bytes=(Get-Item "$taskStage/NotifyRedirect.exe").Length;cli_bytes=(Get-Item "$taskStage/tools/notifyredirect-cli.exe").Length;stage_bytes=$taskSize;msix_bytes=$taskPackageSize;baseline_msix_bytes=132549351;reduction_percent=[math]::Round((1-$taskPackageSize/132549351)*100,2);signed=$false;package=$taskPackage}
$taskReport | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskOutput 'size-report.json') -Encoding utf8
$taskReport | ConvertTo-Json
Write-Output '包尚未签名；下一步运行 Sign-RustDevPackage.ps1。未安装 APP。'
