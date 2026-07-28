[CmdletBinding()]
param([switch]$InstallDevCertificate)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'fetch-sevenzip.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& (Join-Path $PSScriptRoot 'build-windows-shell-integration.ps1') -InstallDevCertificate:$InstallDevCertificate
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pnpm --filter @qzip/desktop tauri build --config tauri.windows.bundle.json
exit $LASTEXITCODE
