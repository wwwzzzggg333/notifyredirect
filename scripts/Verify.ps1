$ErrorActionPreference='Stop'
. "$PSScriptRoot/Use-LocalSdk.ps1"
dotnet restore "$taskRoot/NotifyRedirect.sln" --configfile "$taskRoot/NuGet.Config" --locked-mode -r win-x64 -p:SelfContained=true
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
dotnet build "$taskRoot/NotifyRedirect.sln" -c Release --no-restore
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
dotnet test "$taskRoot/NotifyRedirect.sln" -c Release --no-build --no-restore
exit $LASTEXITCODE
