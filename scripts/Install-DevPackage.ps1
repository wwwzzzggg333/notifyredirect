param([switch]$TrustDevelopmentCertificate)
$ErrorActionPreference='Stop'
$taskRoot=Split-Path $PSScriptRoot -Parent
if(-not $TrustDevelopmentCertificate){throw '该包使用自签开发证书。确认愿意信任后，使用 -TrustDevelopmentCertificate 运行；此操作修改当前用户受信任证书并安装 APP。'}
Import-Certificate -FilePath "$taskRoot/artifacts/NotifyRedirect-Development.cer" -CertStoreLocation Cert:\CurrentUser\TrustedPeople | Out-Null
Add-AppxPackage -Path "$taskRoot/artifacts/NotifyRedirect-0.1.0-x64.msix"
