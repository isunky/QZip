[CmdletBinding()]
param(
  [switch]$InstallDevCertificate,
  [switch]$CiDevelopmentSigning,
  [switch]$Release,
  [ValidateSet('nsis', 'msi')]
  [string[]]$Bundles = @('nsis', 'msi')
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  $windowsPowerShellModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
  if (-not [string]::IsNullOrWhiteSpace($windowsPowerShellModulePath)) { $env:PSModulePath = $windowsPowerShellModulePath }
}
if ($Release) { Import-Module Microsoft.PowerShell.Security -ErrorAction Stop }
if ($Release -and ($InstallDevCertificate -or $CiDevelopmentSigning)) {
  throw 'Release, InstallDevCertificate, and CiDevelopmentSigning cannot be used together.'
}
if ($InstallDevCertificate -and $CiDevelopmentSigning) {
  throw 'InstallDevCertificate and CiDevelopmentSigning cannot be used together.'
}
if ($Release -and (-not $env:QZIP_WINDOWS_PFX_PATH -or -not $env:QZIP_WINDOWS_PFX_PASSWORD -or -not $env:QZIP_WINDOWS_PUBLISHER)) {
  throw 'Release builds require QZIP_WINDOWS_PFX_PATH, QZIP_WINDOWS_PFX_PASSWORD, and QZIP_WINDOWS_PUBLISHER.'
}
& (Join-Path $PSScriptRoot 'fetch-sevenzip.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& (Join-Path $PSScriptRoot 'build-windows-shell-integration.ps1') -InstallDevCertificate:$InstallDevCertificate -CiDevelopmentSigning:$CiDevelopmentSigning -Release:$Release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

pnpm --filter @qzip/desktop tauri build --no-bundle --config tauri.windows.bundle.json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Release) {
  $signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $signTool) { throw 'Windows SDK x64 signtool.exe was not found.' }
  $desktopExecutable = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\qzip-desktop.exe'
  & $signTool.FullName sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $env:QZIP_WINDOWS_PFX_PATH /p $env:QZIP_WINDOWS_PFX_PASSWORD $desktopExecutable
  if ($LASTEXITCODE -ne 0) { throw 'Desktop executable release signing failed.' }
  $signature = Get-AuthenticodeSignature -LiteralPath $desktopExecutable
  if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike "*$env:QZIP_WINDOWS_PUBLISHER*") {
    throw "Trusted publisher signature validation failed for $desktopExecutable."
  }
}

pnpm --filter @qzip/desktop tauri bundle --bundles ($Bundles -join ',') --config tauri.windows.bundle.json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Release) {
  $bundleRoot = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\bundle'
  $targets = foreach ($bundle in $Bundles) {
    $extension = if ($bundle -eq 'nsis') { '*.exe' } else { '*.msi' }
    Get-ChildItem (Join-Path $bundleRoot $bundle) -Filter $extension -File
  }
  foreach ($target in $targets) {
    & $signTool.FullName sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $env:QZIP_WINDOWS_PFX_PATH /p $env:QZIP_WINDOWS_PFX_PASSWORD $target.FullName
    if ($LASTEXITCODE -ne 0) { throw "Release signing failed for $($target.FullName)." }
    $signature = Get-AuthenticodeSignature -LiteralPath $target.FullName
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike "*$env:QZIP_WINDOWS_PUBLISHER*") {
      throw "Trusted publisher signature validation failed for $($target.FullName)."
    }
  }
}
