param([string]$PackagePath='artifacts/rust/NotifyRedirect-Native-1.0.0-x64.msix')
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$taskAllowed=[IO.Path]::GetFullPath((Join-Path $taskRoot 'artifacts/rust'))
$taskPackage=[IO.Path]::GetFullPath((Join-Path $taskRoot $PackagePath))
if(-not $taskPackage.StartsWith($taskAllowed+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw '只能签名 artifacts/rust 内的包'}
if(-not (Test-Path -LiteralPath $taskPackage)){throw '请先构建 Rust MSIX'}
$taskKeys=Join-Path $taskRoot '.tools/signing-rust';New-Item -ItemType Directory -Force -Path $taskKeys | Out-Null
$taskRsa=[Security.Cryptography.RSA]::Create(2048)
$taskReq=[Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=NotifyRedirect Development',$taskRsa,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pkcs1)
$taskOids=[Security.Cryptography.OidCollection]::new();$taskOids.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3')) | Out-Null
$taskReq.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($taskOids,$false))
$taskReq.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false,$false,0,$true))
$taskCert=$taskReq.CreateSelfSigned([DateTimeOffset]::Now.AddDays(-1),[DateTimeOffset]::Now.AddYears(1))
$taskPassword=[Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
$taskPfx=Join-Path $taskKeys 'native-dev.pfx'
try{
 [IO.File]::WriteAllBytes($taskPfx,$taskCert.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx,$taskPassword))
 $taskKit=Get-ChildItem 'C:/Program Files (x86)/Windows Kits/10/bin' -Directory | Where-Object {Test-Path (Join-Path $_.FullName 'x64/signtool.exe')} | Sort-Object Name -Descending | Select-Object -First 1
 if(-not $taskKit){throw '需要 SignTool'}
 if(-not (Test-Path -LiteralPath $taskPfx)){throw 'PFX 写入后无法读取'}
 & (Join-Path $taskKit.FullName 'x64/signtool.exe') sign /fd SHA256 /f $taskPfx /p $taskPassword $taskPackage
 if($LASTEXITCODE -ne 0){throw '开发签名失败：请在正常 Windows 用户会话运行此脚本，检查 PFX/私钥访问权限'}
 [IO.File]::WriteAllBytes((Join-Path $taskAllowed 'NotifyRedirect-Native-Development.cer'),$taskCert.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert))
 $taskReport=Join-Path $taskAllowed 'size-report.json'
 if(Test-Path -LiteralPath $taskReport){$taskData=Get-Content $taskReport -Raw | ConvertFrom-Json;$taskData.signed=$true;$taskData.msix_bytes=(Get-Item $taskPackage).Length;$taskData.reduction_percent=[math]::Round((1-$taskData.msix_bytes/$taskData.baseline_msix_bytes)*100,2);$taskData | ConvertTo-Json | Set-Content $taskReport -Encoding utf8}
 Write-Output '开发签名完成；未信任证书、未安装 APP，未使用时间戳服务。'
}finally{if(Test-Path -LiteralPath $taskPfx){Remove-Item -LiteralPath $taskPfx};$taskCert.Dispose();$taskRsa.Dispose()}
