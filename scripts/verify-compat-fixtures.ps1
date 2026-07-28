[CmdletBinding()]
param([switch]$AllowIncomplete)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$fixtureRoot = Join-Path $repoRoot 'tests\fixtures\compat'
$manifest = Get-Content -Raw (Join-Path $fixtureRoot 'manifest.json') | ConvertFrom-Json
if ($manifest.schemaVersion -notin @(1, 2) -or -not $manifest.cases) { throw 'Invalid compatibility fixture manifest.' }
$missing = @(); $verified = 0
foreach ($case in $manifest.cases) {
  if ($case.status -eq 'blocked') { $missing += "$($case.producer) ($($case.format)): $($case.blocker)"; continue }
  $path = Join-Path $fixtureRoot $case.file
  if ([string]::IsNullOrWhiteSpace($case.producerVersion) -or [string]::IsNullOrWhiteSpace($case.sha256) -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
    $missing += "$($case.producer) ($($case.format)): $($case.file)"
    continue
  }
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
  if ($actual -ne $case.sha256) { throw "Fixture checksum mismatch: $($case.file)" }
  $verified++
}
if ($missing) {
  if ($AllowIncomplete) { Write-Warning "Compatibility matrix is incomplete; verified $verified sample(s):`n$($missing -join "`n")"; exit 0 }
  throw "Compatibility fixture matrix is incomplete:`n$($missing -join "`n")"
}
Write-Host "Compatibility fixture matrix verified ($verified samples)."
