param([string]$PackagePath='artifacts/NotifyRedirect-0.1.0-x64.msix')
$ErrorActionPreference='Stop'
$taskRoot=Split-Path $PSScriptRoot -Parent
$taskPackage=[IO.Path]::GetFullPath((Join-Path $taskRoot $PackagePath))
if(-not $taskPackage.StartsWith($taskRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw '包必须位于项目内'}
$taskKeys=Join-Path $taskRoot '.tools/signing';New-Item -ItemType Directory -Force -Path $taskKeys | Out-Null
$taskRsa=[Security.Cryptography.RSA]::Create(2048)
$taskReq=[Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=NotifyRedirect Development',$taskRsa,[Security.Cryptography.HashAlgorithmName]::SHA256,[Security.Cryptography.RSASignaturePadding]::Pkcs1)
$taskOids=[Security.Cryptography.OidCollection]::new();$taskOids.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3')) | Out-Null
$taskReq.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($taskOids,$false))
$taskReq.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false,$false,0,$true))
$taskCert=$taskReq.CreateSelfSigned([DateTimeOffset]::Now.AddDays(-1),[DateTimeOffset]::Now.AddYears(1))
$taskPassword=[Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
$taskPfx=Join-Path $taskKeys 'dev.pfx';[IO.File]::WriteAllBytes($taskPfx,$taskCert.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx,$taskPassword))
[IO.File]::WriteAllBytes((Join-Path $taskRoot 'artifacts/NotifyRedirect-Development.cer'),$taskCert.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert))
$taskKit=Get-ChildItem 'C:/Program Files (x86)/Windows Kits/10/bin' -Directory | Where-Object {Test-Path (Join-Path $_.FullName 'x64/signtool.exe')} | Sort-Object Name -Descending | Select-Object -First 1
& (Join-Path $taskKit.FullName 'x64/signtool.exe') sign /fd SHA256 /f $taskPfx /p $taskPassword $taskPackage
if($LASTEXITCODE -ne 0){throw '开发签名失败'}
Remove-Item -LiteralPath $taskPfx
$taskCert.Dispose();$taskRsa.Dispose()
Write-Output '已完成本地开发签名；未向系统证书存储添加信任，也未安装 APP。'
