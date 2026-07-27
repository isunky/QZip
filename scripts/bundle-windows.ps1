[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'fetch-sevenzip.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pnpm --filter @qzip/desktop tauri build --config tauri.windows.bundle.json
exit $LASTEXITCODE
