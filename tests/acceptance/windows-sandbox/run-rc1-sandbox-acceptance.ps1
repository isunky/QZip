[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$resultRoot = "C:\QZipResults"
New-Item -ItemType Directory -Force -Path $resultRoot | Out-Null
$setup = Get-ChildItem "C:\QZip\target\release\bundle\nsis" -Filter "QZip_1.0.0_x64-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "RC1 NSIS installer was not found in the mapped workspace." }
$record = [ordered]@{ startedAt=(Get-Date).ToUniversalTime().ToString("o"); setup=$setup.FullName; install=$null; shellPackage=$null; appLaunch=$null; uninstall=$null; errors=@() }
try {
  $developmentCertificate = "C:\QZip\artifacts\windows-shell\QZip.Development.cer"
  if (Test-Path -LiteralPath $developmentCertificate) {
    Import-Certificate -FilePath $developmentCertificate -CertStoreLocation "Cert:\CurrentUser\TrustedPeople" | Out-Null
    $record.developmentCertificateTrusted = $true
  } else { throw "Mapped development certificate is missing: $developmentCertificate" }
  $install = Start-Process -FilePath $setup.FullName -ArgumentList "/S" -Wait -PassThru
  $record.install = @{ exitCode=$install.ExitCode }
  $package = Get-AppxPackage -Name "app.qzip.desktop.shell" -ErrorAction SilentlyContinue
  $record.shellPackage = @{ registered=($null -ne $package); fullName=if($package){$package.PackageFullName}else{$null} }
  $app = Get-ChildItem "$env:LOCALAPPDATA\QZip" -Filter "qzip-desktop.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($app) { $process = Start-Process -FilePath $app.FullName -PassThru; Start-Sleep -Seconds 3; $record.appLaunch = @{ started=$true; responsive=($process.Responding); processId=$process.Id }; Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
  else { $record.appLaunch = @{ started=$false; reason="installed executable not found" } }
  $uninstaller = Get-ChildItem "$env:LOCALAPPDATA\QZip" -Filter "uninstall.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($uninstaller) { $uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList "/S" -Wait -PassThru; $record.uninstall = @{ exitCode=$uninstall.ExitCode; shellPackageRemoved=($null -eq (Get-AppxPackage -Name "app.qzip.desktop.shell" -ErrorAction SilentlyContinue)) } }
} catch { $record.errors += $_.Exception.Message }
$record.finishedAt = (Get-Date).ToUniversalTime().ToString("o")
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $resultRoot "rc1-installation-result.json") -Encoding utf8
if ($record.errors.Count -gt 0) { exit 1 }
