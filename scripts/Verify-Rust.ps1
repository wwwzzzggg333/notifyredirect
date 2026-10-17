param([switch]$SkipUserSession,[switch]$Offline)
$ErrorActionPreference='Stop'
$taskRoot=Split-Path $PSScriptRoot -Parent
$env:CARGO_TARGET_DIR=Join-Path $taskRoot '.tools/rust-build'
$taskManifest=Join-Path $taskRoot 'native/Cargo.toml'
& cargo fmt --manifest-path $taskManifest --all -- --check
if($LASTEXITCODE -ne 0){throw 'Rust 格式检查失败'}
$taskExtra=@();if($Offline){$taskExtra+='--offline'}
& cargo clippy --manifest-path $taskManifest --workspace --all-targets --locked @taskExtra -- -D warnings
if($LASTEXITCODE -ne 0){throw 'Clippy 检查失败'}
$taskTests=@();if(-not $SkipUserSession){$taskTests=@('--','--include-ignored')}
& cargo test --manifest-path $taskManifest --workspace --locked @taskExtra @taskTests
if($LASTEXITCODE -ne 0){throw 'Rust 测试失败'}
if($SkipUserSession){Write-Output '正常用户 DPAPI 与 Explorer 托盘测试已显式跳过，尚需在正常桌面运行完整验证。'}
Write-Output '所选 Rust 验证通过。'
