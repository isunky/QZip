[CmdletBinding()]
param([string]$RarSample)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $root
try {
  pnpm run test:core
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $sidecarArgs = @{}
  if ($RarSample) { $sidecarArgs.RarSample = $RarSample }
  & (Join-Path $PSScriptRoot 'test-sevenzip.ps1') @sidecarArgs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  Write-Host 'QZip core regression passed.'
}
finally { Pop-Location }
