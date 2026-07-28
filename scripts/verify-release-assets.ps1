[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidatePattern('^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$')]
  [string]$Version,
  [switch]$RequireTrustedSignature,
  [string]$ExpectedPublisher
)

$ErrorActionPreference = 'Stop'
$releaseVersion = $Version.TrimStart('v')
$repoRoot = Split-Path -Parent $PSScriptRoot
$releaseRoot = Join-Path $repoRoot ("artifacts\release\{0}" -f $releaseVersion)
$checksumsPath = Join-Path $releaseRoot 'checksums-sha256.txt'
$manifestPath = Join-Path $releaseRoot 'release-manifest.json'
if (-not (Test-Path -LiteralPath $checksumsPath) -or -not (Test-Path -LiteralPath $manifestPath)) {
  throw "Release manifest or checksums were not found under $releaseRoot."
}
$manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
if ($manifest.version -ne $releaseVersion) { throw 'Release manifest version does not match the requested version.' }
foreach ($line in Get-Content $checksumsPath) {
  if ($line -notmatch '^([0-9a-f]{64}) \*(.+)$') { throw "Invalid checksum line: $line" }
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $releaseRoot $Matches[2])).Hash.ToLowerInvariant()
  if ($actual -ne $Matches[1]) { throw "Checksum mismatch: $($Matches[2])" }
}

$portablePath = Join-Path $releaseRoot ("QZip-$releaseVersion-windows-x64-portable.zip")
$inspectRoot = Join-Path $releaseRoot 'portable-inspect'
if (Test-Path -LiteralPath $inspectRoot) { Remove-Item -LiteralPath $inspectRoot -Recurse -Force }
Expand-Archive -LiteralPath $portablePath -DestinationPath $inspectRoot
try {
  $sidecarManifest = Get-Content -Raw (Join-Path $repoRoot 'third_party\7zip\manifest.json') | ConvertFrom-Json
  foreach ($file in $sidecarManifest.runtime.files) {
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $inspectRoot "7zip\$file")).Hash.ToLowerInvariant()
    if ($actual -ne $sidecarManifest.runtime.fileHashes.$file) { throw "Portable Sidecar checksum mismatch: $file" }
  }
  if ($RequireTrustedSignature) {
    if ([string]::IsNullOrWhiteSpace($ExpectedPublisher)) { throw 'ExpectedPublisher is required when trusted signatures are required.' }
    foreach ($file in @(
      (Join-Path $releaseRoot "QZip-$releaseVersion-windows-x64-setup.exe"),
      (Join-Path $releaseRoot "QZip-$releaseVersion-windows-x64.msi"),
      (Join-Path $inspectRoot 'QZip.exe')
    )) {
      $signature = Get-AuthenticodeSignature -LiteralPath $file
      if ($signature.Status -ne 'Valid') { throw "A trusted Authenticode signature is required: $file ($($signature.Status))" }
      if ($signature.SignerCertificate.Subject -notlike "*$ExpectedPublisher*") { throw "Unexpected Authenticode publisher: $file" }
    }
  }
}
finally {
  if (Test-Path -LiteralPath $inspectRoot) { Remove-Item -LiteralPath $inspectRoot -Recurse -Force }
}
Write-Host "Release assets verified: $releaseRoot"
