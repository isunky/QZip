[CmdletBinding()]
param(
  [switch]$Release,
  [switch]$OpenOutput
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$bundleScript = Join-Path $PSScriptRoot 'bundle-windows.ps1'

Write-Host 'Building QZip NSIS installer for Windows x64...' -ForegroundColor Cyan

$bundleArguments = @{
  Bundles = @('nsis')
  Release = $Release
}
if (-not $Release) {
  # Local sparse MSIX shell integration needs a current-user development certificate.
  $bundleArguments.InstallDevCertificate = $true
}

& $bundleScript @bundleArguments
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$installerDirectory = Join-Path $root 'target\release\bundle\nsis'
$installer = Get-ChildItem -LiteralPath $installerDirectory -Filter 'QZip_*_x64-setup.exe' -File |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1
if (-not $installer) { throw "NSIS installer was not produced in $installerDirectory" }

Write-Host ''
Write-Host 'NSIS installer created:' -ForegroundColor Green
Write-Host $installer.FullName -ForegroundColor Green

if ($OpenOutput) {
  Invoke-Item -LiteralPath $installerDirectory
}
