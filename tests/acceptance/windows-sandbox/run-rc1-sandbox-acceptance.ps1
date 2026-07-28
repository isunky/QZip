[CmdletBinding()]
param(
  [string]$BaselineInstaller,
  [string]$CandidateInstaller = 'C:\QZip\target\release\bundle\nsis\QZip_1.0.0_x64-setup.exe'
)

$ErrorActionPreference = 'Stop'
$resultRoot = 'C:\QZipResults'
$logRoot = Join-Path $resultRoot 'logs'
New-Item -ItemType Directory -Force -Path $resultRoot, $logRoot | Out-Null

$record = [ordered]@{
  schemaVersion = 2
  startedAt = (Get-Date).ToUniversalTime().ToString('o')
  candidateInstaller = $CandidateInstaller
  baselineInstaller = $BaselineInstaller
  developmentCertificateTrusted = $false
  steps = @()
  errors = @()
  result = 'failed'
}

function Add-Step([string]$Name, [hashtable]$Data) {
  $record.steps += [ordered]@{ name = $Name; recordedAt = (Get-Date).ToUniversalTime().ToString('o'); data = $Data }
}

function Invoke-CheckedProcess([string]$Name, [string]$FilePath, [string[]]$Arguments) {
  $stdout = Join-Path $logRoot "$Name.stdout.log"
  $stderr = Join-Path $logRoot "$Name.stderr.log"
  $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -Wait -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
  Add-Step $Name @{ exitCode = $process.ExitCode; stdout = $stdout; stderr = $stderr }
  if ($process.ExitCode -ne 0) { throw "$Name failed with exit code $($process.ExitCode)." }
}

function Test-AssociationRegistration {
  $registered = Get-ItemProperty -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\RegisteredApplications' -Name 'QZip' -ErrorAction Stop
  if ($registered.QZip -ne 'Software\QZip\Capabilities') { throw 'QZip RegisteredApplications value is invalid.' }
  $capabilities = 'Registry::HKEY_CURRENT_USER\Software\QZip\Capabilities\FileAssociations'
  foreach ($extension in @('.7z', '.zip', '.rar', '.tar', '.gz', '.xz', '.bz2')) {
    $actual = (Get-ItemProperty -LiteralPath $capabilities -Name $extension -ErrorAction Stop).$extension
    if ($actual -ne 'QZip.Archive') { throw "QZip association is missing for $extension." }
  }
  $openCommand = (Get-Item -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\Classes\QZip.Archive\shell\open\command' -ErrorAction Stop).GetValue('')
  if ($openCommand -notmatch 'qzip-desktop\.exe') { throw 'QZip open command is missing.' }
  Add-Step 'fileAssociations' @{ registeredApplication = $registered.QZip; openCommand = $openCommand }
}

try {
  if (-not (Test-Path -LiteralPath $CandidateInstaller -PathType Leaf)) { throw "Candidate installer was not found: $CandidateInstaller" }
  if ($BaselineInstaller -and -not (Test-Path -LiteralPath $BaselineInstaller -PathType Leaf)) { throw "Baseline installer was not found: $BaselineInstaller" }

  $developmentCertificate = 'C:\QZip\artifacts\windows-shell\QZip.Development.cer'
  if (-not (Test-Path -LiteralPath $developmentCertificate -PathType Leaf)) { throw "Mapped development certificate is missing: $developmentCertificate" }
  foreach ($store in @('Cert:\CurrentUser\Root', 'Cert:\CurrentUser\TrustedPeople')) {
    Import-Certificate -FilePath $developmentCertificate -CertStoreLocation $store | Out-Null
  }
  $record.developmentCertificateTrusted = $true
  Add-Step 'developmentCertificate' @{ path = $developmentCertificate; stores = @('CurrentUser/Root', 'CurrentUser/TrustedPeople') }

  if ($BaselineInstaller) { Invoke-CheckedProcess 'baselineInstall' $BaselineInstaller @('/S') }
  Invoke-CheckedProcess 'candidateInstall' $CandidateInstaller @('/S')

  $package = Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue
  if (-not $package) { throw 'QZip sparse Shell MSIX package was not registered.' }
  Add-Step 'shellPackage' @{ registered = $true; fullName = $package.PackageFullName; status = $package.Status }
  Test-AssociationRegistration

  $app = Get-ChildItem "$env:LOCALAPPDATA\QZip" -Filter 'qzip-desktop.exe' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $app) { throw 'Installed QZip executable was not found.' }
  $process = Start-Process -FilePath $app.FullName -PassThru
  Start-Sleep -Seconds 3
  $process.Refresh()
  if ($process.MainWindowHandle -eq 0 -or -not $process.Responding) { throw 'Installed QZip window did not become responsive.' }
  Add-Step 'appLaunch' @{ path = $app.FullName; processId = $process.Id; responsive = $process.Responding }
  Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue

  $uninstaller = Get-ChildItem "$env:LOCALAPPDATA\QZip" -Filter 'uninstall.exe' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $uninstaller) { throw 'QZip uninstaller was not found.' }
  Invoke-CheckedProcess 'uninstall' $uninstaller.FullName @('/S')
  if (Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue) { throw 'QZip sparse Shell MSIX package remained after uninstall.' }
  if (Test-Path -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\QZip\Capabilities') { throw 'QZip capabilities remained after uninstall.' }
  $registered = Get-ItemProperty -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\RegisteredApplications' -Name 'QZip' -ErrorAction SilentlyContinue
  if ($registered) { throw 'QZip RegisteredApplications value remained after uninstall.' }
  Add-Step 'uninstallCleanup' @{ shellPackageRemoved = $true; associationRegistrationRemoved = $true }

  $record.result = 'passed'
}
catch {
  $record.errors += $_.Exception.Message
  $appxEvents = Get-WinEvent -LogName 'Microsoft-Windows-AppXDeploymentServer/Operational' -MaxEvents 20 -ErrorAction SilentlyContinue |
    Select-Object TimeCreated, Id, LevelDisplayName, Message
  if ($appxEvents) {
    $appxLog = Join-Path $logRoot 'appx-deployment-events.json'
    $appxEvents | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $appxLog -Encoding utf8
    Add-Step 'appxDiagnostics' @{ path = $appxLog }
  }
}
finally {
  $record.finishedAt = (Get-Date).ToUniversalTime().ToString('o')
  $record | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $resultRoot 'rc1-installation-result.json') -Encoding utf8
}

if ($record.result -ne 'passed') { exit 1 }
