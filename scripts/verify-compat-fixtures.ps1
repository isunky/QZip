[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$fixtureRoot = Join-Path $repoRoot 'tests\fixtures\compat'
$manifest = Get-Content -Raw (Join-Path $fixtureRoot 'manifest.json') | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or -not $manifest.cases) { throw 'Invalid compatibility fixture manifest.' }
$missing = @()
foreach ($case in $manifest.cases) {
  $path = Join-Path $fixtureRoot $case.file
  if ([string]::IsNullOrWhiteSpace($case.producerVersion) -or [string]::IsNullOrWhiteSpace($case.sha256) -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
    $missing += "$($case.producer) ($($case.format)): $($case.file)"
    continue
  }
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
  if ($actual -ne $case.sha256) { throw "Fixture checksum mismatch: $($case.file)" }
}
if ($missing) {
  throw "Compatibility fixture matrix is incomplete:`n$($missing -join "`n")"
}
Write-Host "Compatibility fixture matrix verified ($($manifest.cases.Count) samples)."
