[CmdletBinding()]
param(
  [switch]$InstallDevCertificate,
  [switch]$Release
)

$ErrorActionPreference = 'Stop'
if ($Release -and $InstallDevCertificate) {
  throw 'Release and InstallDevCertificate cannot be used together.'
}
if ($Release -and (-not $env:QZIP_WINDOWS_PFX_PATH -or -not $env:QZIP_WINDOWS_PFX_PASSWORD -or -not $env:QZIP_WINDOWS_PUBLISHER)) {
  throw 'Release builds require QZIP_WINDOWS_PFX_PATH, QZIP_WINDOWS_PFX_PASSWORD, and QZIP_WINDOWS_PUBLISHER.'
}
& (Join-Path $PSScriptRoot 'fetch-sevenzip.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& (Join-Path $PSScriptRoot 'build-windows-shell-integration.ps1') -InstallDevCertificate:$InstallDevCertificate -Release:$Release
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

pnpm --filter @qzip/desktop tauri bundle --config tauri.windows.bundle.json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Release) {
  $bundleRoot = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\bundle'
  $targets = @(
    @(Get-ChildItem (Join-Path $bundleRoot 'nsis') -Filter '*.exe' -File)
    @(Get-ChildItem (Join-Path $bundleRoot 'msi') -Filter '*.msi' -File)
  )
  foreach ($target in $targets) {
    & $signTool.FullName sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $env:QZIP_WINDOWS_PFX_PATH /p $env:QZIP_WINDOWS_PFX_PASSWORD $target.FullName
    if ($LASTEXITCODE -ne 0) { throw "Release signing failed for $($target.FullName)." }
    $signature = Get-AuthenticodeSignature -LiteralPath $target.FullName
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike "*$env:QZIP_WINDOWS_PUBLISHER*") {
      throw "Trusted publisher signature validation failed for $($target.FullName)."
    }
  }
}
