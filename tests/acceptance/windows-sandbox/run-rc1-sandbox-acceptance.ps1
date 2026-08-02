[CmdletBinding()]
param(
  [ValidateSet('Fresh', 'Upgrade')][string]$Scenario = 'Fresh',
  [string]$BaselineInstaller = 'C:\QZip\target\release\bundle\nsis\QZip_0.1.0_x64-setup.exe',
  [string]$CandidateInstaller = 'C:\QZip\target\release\bundle\nsis\QZip_1.0.0_x64-setup.exe',
  [switch]$Interactive
)

$ErrorActionPreference = 'Stop'
$resultRoot = 'C:\QZipResults'
$logRoot = Join-Path $resultRoot "logs\$Scenario"
$fixtureRoot = Join-Path $env:USERPROFILE 'Documents\QZip-RC1-Acceptance'
New-Item -ItemType Directory -Force -Path $resultRoot, $logRoot, $fixtureRoot | Out-Null

$record = [ordered]@{
  schemaVersion = 3
  scenario = $Scenario
  startedAt = (Get-Date).ToUniversalTime().ToString('o')
  host = [ordered]@{
    caption = (Get-CimInstance Win32_OperatingSystem).Caption
    version = (Get-CimInstance Win32_OperatingSystem).Version
    build = (Get-CimInstance Win32_OperatingSystem).BuildNumber
    user = $env:USERNAME
  }
  installers = [ordered]@{}
  steps = @()
  errors = @()
  result = 'failed'
}

function Add-Step([string]$Name, [hashtable]$Data) {
  $record.steps += [ordered]@{ name = $Name; recordedAt = (Get-Date).ToUniversalTime().ToString('o'); data = $Data }
}

function Installer-Metadata([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Installer was not found: $Path" }
  $file = Get-Item -LiteralPath $Path
  return [ordered]@{
    path = $Path
    sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    fileVersion = $file.VersionInfo.FileVersion
    productVersion = $file.VersionInfo.ProductVersion
    length = $file.Length
  }
}

function Invoke-CheckedProcess([string]$Name, [string]$FilePath, [string[]]$Arguments) {
  $stdout = Join-Path $logRoot "$Name.stdout.log"
  $stderr = Join-Path $logRoot "$Name.stderr.log"
  $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -Wait -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
  Add-Step $Name @{ exitCode = $process.ExitCode; stdout = $stdout; stderr = $stderr }
  if ($process.ExitCode -ne 0) {
    $shellLog = Join-Path $env:LOCALAPPDATA 'QZip\Logs\shell-registration.log'
    if (Test-Path -LiteralPath $shellLog -PathType Leaf) {
      Copy-Item -LiteralPath $shellLog -Destination (Join-Path $logRoot "$Name.shell-registration.log") -Force
    }
    $hookResult = Join-Path $env:TEMP 'qzip-shell-registration-result.log'
    if (Test-Path -LiteralPath $hookResult -PathType Leaf) {
      Copy-Item -LiteralPath $hookResult -Destination (Join-Path $logRoot "$Name.shell-hook-result.log") -Force
    }
    throw "$Name failed with exit code $($process.ExitCode)."
  }
}

function Get-QZipInstall {
  $app = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'QZip') -Filter 'qzip-desktop.exe' -Recurse -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $app) { throw 'Installed QZip executable was not found.' }
  return $app
}

function Test-AssociationRegistration {
  $registered = Get-ItemProperty -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\RegisteredApplications' -Name 'QZip' -ErrorAction Stop
  if ($registered.QZip -ne 'Software\QZip\Capabilities') { throw 'QZip RegisteredApplications value is invalid.' }
  $capabilities = 'Registry::HKEY_CURRENT_USER\Software\QZip\Capabilities\FileAssociations'
  $associations = [ordered]@{
    '.7z' = '7z'; '.zip' = 'zip'; '.rar' = 'rar'; '.tar' = 'tar'; '.gz' = 'gz'; '.tgz' = 'tgz'
    '.xz' = 'xz'; '.txz' = 'txz'; '.bz2' = 'bz2'; '.iso' = 'iso'; '.cab' = 'cab'; '.wim' = 'wim'
  }
  foreach ($entry in $associations.GetEnumerator()) {
    $actual = (Get-ItemProperty -LiteralPath $capabilities -Name $entry.Key -ErrorAction Stop).$($entry.Key)
    $progId = "QZip.Archive.$($entry.Value)"
    if ($actual -ne $progId) { throw "QZip association is missing for $($entry.Key)." }
    $extensionProgId = (Get-Item -LiteralPath "Registry::HKEY_CURRENT_USER\Software\Classes\$($entry.Key)" -ErrorAction Stop).GetValue('')
    if ($extensionProgId -ne $progId) { throw "QZip shell association is not using the per-format ProgID for $($entry.Key)." }
    $classRoot = "Registry::HKEY_CURRENT_USER\Software\Classes\$progId"
    $openCommand = (Get-Item -LiteralPath "$classRoot\shell\open\command" -ErrorAction Stop).GetValue('')
    $defaultIcon = (Get-Item -LiteralPath "$classRoot\DefaultIcon" -ErrorAction Stop).GetValue('')
    if ($openCommand -notmatch 'qzip-desktop\.exe') { throw "QZip open command is missing for $($entry.Key)." }
    if ($defaultIcon -notmatch "file-icons\\$($entry.Value)\.ico") { throw "QZip icon is missing for $($entry.Key)." }
  }
  Add-Step 'fileAssociations' @{ registeredApplication = $registered.QZip; extensions = @($associations.Keys) }
}

function Test-InstalledIntegration([string]$ExpectedVersion) {
  $app = Get-QZipInstall
  if ($app.VersionInfo.ProductVersion -notlike "$ExpectedVersion*") { throw "Installed QZip version $($app.VersionInfo.ProductVersion) does not match $ExpectedVersion." }
  $process = Start-Process -FilePath $app.FullName -PassThru
  Start-Sleep -Seconds 5
  $process.Refresh()
  if ($process.MainWindowHandle -eq 0 -or -not $process.Responding) { throw 'Installed QZip window did not become responsive.' }
  Add-Step 'appLaunch' @{ path = $app.FullName; productVersion = $app.VersionInfo.ProductVersion; processId = $process.Id; responsive = $process.Responding }
  $deadline = (Get-Date).AddSeconds(20)
  do {
    $package = Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue
    if ($package -and $package.Status -eq 'Ok') { break }
    Start-Sleep -Seconds 1
  } while ((Get-Date) -lt $deadline)
  if (-not $package) { throw 'QZip sparse Shell MSIX package was not registered after application startup.' }
  if ($package.Status -ne 'Ok') { throw "QZip sparse Shell MSIX status is $($package.Status)." }
  Add-Step 'shellPackage' @{ registered = $true; fullName = $package.PackageFullName; status = $package.Status }
  Test-AssociationRegistration
  return @{ app = $app; process = $process }
}

function New-ExplorerFixtures {
  $source = Join-Path $fixtureRoot 'source'
  $archive = Join-Path $fixtureRoot 'sample.zip'
  $folder = Join-Path $fixtureRoot 'folder-sample'
  New-Item -ItemType Directory -Force -Path $source, $folder | Out-Null
  Set-Content -LiteralPath (Join-Path $source 'unicode-轻压.txt') -Value 'QZip RC1 Windows Sandbox acceptance' -Encoding utf8
  Set-Content -LiteralPath (Join-Path $folder 'folder.txt') -Value 'folder shell command fixture' -Encoding utf8
  Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
  Compress-Archive -Path (Join-Path $source '*') -DestinationPath $archive -Force
  Add-Step 'explorerFixtures' @{ root = $fixtureRoot; archive = $archive; folder = $folder }
  return @{ root = $fixtureRoot; archive = $archive; folder = $folder }
}

function Restart-ExplorerShell {
  Get-Process explorer -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
  Start-Process explorer.exe
  Start-Sleep -Seconds 3
  Add-Step 'explorerShellRestarted' @{ after = 'QZip Shell MSIX registration' }
}

function Test-UninstallCleanup {
  $uninstaller = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'QZip') -Filter 'uninstall.exe' -Recurse -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if (-not $uninstaller) { throw 'QZip uninstaller was not found.' }
  Invoke-CheckedProcess 'uninstall' $uninstaller.FullName @('/S')
  if (Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue) { throw 'QZip sparse Shell MSIX package remained after uninstall.' }
  if (Test-Path -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\QZip\Capabilities') { throw 'QZip capabilities remained after uninstall.' }
  if (Test-Path -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\Classes\QZip.Archive') { throw 'QZip ProgID remained after uninstall.' }
  foreach ($slug in @('7z', 'zip', 'rar', 'tar', 'gz', 'tgz', 'xz', 'txz', 'bz2', 'iso', 'cab', 'wim')) {
    if (Test-Path -LiteralPath "Registry::HKEY_CURRENT_USER\Software\Classes\QZip.Archive.$slug") { throw "QZip $slug ProgID remained after uninstall." }
  }
  $registered = Get-ItemProperty -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\RegisteredApplications' -Name 'QZip' -ErrorAction SilentlyContinue
  if ($registered) { throw 'QZip RegisteredApplications value remained after uninstall.' }
  $installRoot = Join-Path $env:LOCALAPPDATA 'QZip'
  $installedPayloads = @(
    'qzip-desktop.exe',
    'uninstall.exe',
    'qzip-shell',
    '7zip'
  ) | ForEach-Object { Join-Path $installRoot $_ }
  $remainingPayloads = @($installedPayloads | Where-Object { Test-Path -LiteralPath $_ })
  if ($remainingPayloads.Count -gt 0) {
    throw "QZip installed payload remained after uninstall: $($remainingPayloads -join ', ')"
  }
  Add-Step 'uninstallCleanup' @{ shellPackageRemoved = $true; associationRegistrationRemoved = $true; installedPayloadRemoved = $true; preservedUserDataRoot = (Test-Path -LiteralPath $installRoot) }
}

function Add-TrustedPackageCertificate([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Development certificate was not found: $Path" }
  $certificateObject = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($Path)
  $store = [System.Security.Cryptography.X509Certificates.X509Store]::new('TrustedPeople', 'CurrentUser')
  try {
    $store.Open('ReadWrite')
    $store.Add($certificateObject)
  }
  finally {
    $store.Close()
  }
  & certutil.exe -addstore -f Root $Path | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "Unable to add the development certificate to LocalMachine/Root." }
  return $certificateObject.Thumbprint
}

function Test-ShellRegistrationPrerequisite {
  $probeRoot = 'C:\QZipShellProbe'
  $probeExecutable = Join-Path $probeRoot 'qzip-desktop.exe'
  $registrationScript = 'C:\QZip\native\windows\qzip-shell\Register-QZipShell.ps1'
  $shellPackage = 'C:\QZip\artifacts\windows-shell\qzip-shell\QZip.Shell.msix'
  New-Item -ItemType Directory -Force -Path $probeRoot | Out-Null
  Copy-Item -LiteralPath 'C:\QZip\target\release\qzip-desktop.exe' -Destination $probeExecutable -Force
  & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $registrationScript -InstallPath $probeRoot -PackagePath $shellPackage
  if ($LASTEXITCODE -ne 0) { throw "Direct Shell registration prerequisite failed with exit code $LASTEXITCODE." }
  $package = Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction Stop
  Add-Step 'shellRegistrationPrerequisite' @{ installPath = $probeRoot; package = $package.PackageFullName; status = $package.Status }
  Remove-AppxPackage -Package $package.PackageFullName -ErrorAction Stop
}

try {
  $record.installers.candidate = Installer-Metadata $CandidateInstaller
  if ($Scenario -eq 'Upgrade') { $record.installers.baseline = Installer-Metadata $BaselineInstaller }

  $certificate = 'C:\QZip\artifacts\windows-shell\QZip.Development.cer'
  if (-not (Test-Path -LiteralPath $certificate -PathType Leaf)) { throw "Mapped development certificate is missing: $certificate" }
  $certificateThumbprints = @([ordered]@{ path = $certificate; thumbprint = (Add-TrustedPackageCertificate $certificate) })
  if ($Scenario -eq 'Upgrade') {
    $baselineCertificate = 'C:\QZipBaseline\QZip.Development.cer'
    $certificateThumbprints += [ordered]@{ path = $baselineCertificate; thumbprint = (Add-TrustedPackageCertificate $baselineCertificate) }
  }
  Add-Step 'developmentCertificate' @{ certificates = $certificateThumbprints; stores = @('CurrentUser/TrustedPeople', 'LocalMachine/Root'); importer = 'X509Store.Add + certutil' }
  Test-ShellRegistrationPrerequisite

  if ($Scenario -eq 'Upgrade') {
    Invoke-CheckedProcess 'baselineInstall' $BaselineInstaller @('/S')
    $baseline = Get-QZipInstall
    Add-Step 'baselineInstalled' @{ path = $baseline.FullName; productVersion = $baseline.VersionInfo.ProductVersion }
  }

  Invoke-CheckedProcess 'candidateInstall' $CandidateInstaller @('/S')
  $integration = Test-InstalledIntegration '1.0.0'
  $integration.process | Stop-Process -Force -ErrorAction SilentlyContinue

  if ($Interactive) {
    Restart-ExplorerShell
    $fixtures = New-ExplorerFixtures
    $ready = [ordered]@{ scenario = $Scenario; fixtureRoot = $fixtures.root; archive = $fixtures.archive; folder = $fixtures.folder; app = $integration.app.FullName }
    $ready | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $resultRoot 'rc1-ui-ready.json') -Encoding utf8
    Start-Process explorer.exe -ArgumentList $fixtures.root
    $deadline = (Get-Date).AddMinutes(30)
    $completePath = Join-Path $resultRoot 'rc1-ui-complete.json'
    while (-not (Test-Path -LiteralPath $completePath)) {
      if ((Get-Date) -gt $deadline) { throw 'Timed out waiting for Sandbox UI acceptance evidence.' }
      Start-Sleep -Seconds 2
    }
    $uiResult = Get-Content -LiteralPath $completePath -Raw | ConvertFrom-Json
    foreach ($required in @('fileAssociationOpened', 'fileMenuVisible', 'folderMenuVisible', 'dpi100', 'dpi150', 'dpi200', 'windowControls')) {
      if (-not $uiResult.$required) { throw "Sandbox UI acceptance did not confirm $required." }
    }
    Add-Step 'interactiveUiEvidence' @{ path = $completePath; evidence = $uiResult }
  }

  Test-UninstallCleanup
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
  $record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $resultRoot "rc1-$($Scenario.ToLowerInvariant())-installation-result.json") -Encoding utf8
}

if ($record.result -ne 'passed') { exit 1 }
