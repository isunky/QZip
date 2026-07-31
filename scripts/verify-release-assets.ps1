[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$Version,
  [switch]$RequireTrustedSignature,
  [string]$ExpectedPublisher
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  $windowsPowerShellModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
  if (-not [string]::IsNullOrWhiteSpace($windowsPowerShellModulePath)) { $env:PSModulePath = $windowsPowerShellModulePath }
}
Import-Module Microsoft.PowerShell.Utility -ErrorAction Stop
if ($RequireTrustedSignature) { Import-Module Microsoft.PowerShell.Security -ErrorAction Stop }
$releaseVersion = $Version.Trim()
if ($releaseVersion -match '[\r\n]' -or $releaseVersion.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0) {
  throw 'Version must be safe for a Windows filename.'
}
$repoRoot = Split-Path -Parent $PSScriptRoot
$releaseRoot = Join-Path $repoRoot ("artifacts\release\{0}" -f $releaseVersion)
$checksumsPath = Join-Path $releaseRoot 'checksums-sha256.txt'
$manifestPath = Join-Path $releaseRoot 'release-manifest.json'
if (-not (Test-Path -LiteralPath $checksumsPath) -or -not (Test-Path -LiteralPath $manifestPath)) {
  throw "Release manifest or checksums were not found under $releaseRoot."
}

$setupName = "QZip-$releaseVersion-windows-x64-setup.exe"
$manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
if ($manifest.version -ne $releaseVersion) { throw 'Release manifest version does not match the requested version.' }
if (@($manifest.assets).Count -ne 1 -or $manifest.assets[0] -ne $setupName) {
  throw 'Release manifest must contain exactly one NSIS setup asset.'
}

$checksumLines = @(Get-Content $checksumsPath)
if ($checksumLines.Count -ne 1 -or $checksumLines[0] -notmatch '^([0-9a-f]{64}) \*(.+)$') {
  throw 'Release checksums must contain exactly one SHA-256 entry.'
}
$checksumAsset = $Matches[2]
if ($checksumAsset -ne $setupName) { throw "Unexpected checksum asset: $checksumAsset" }
$setupPath = Join-Path $releaseRoot $setupName
if (-not (Test-Path -LiteralPath $setupPath -PathType Leaf)) { throw "NSIS setup was not found: $setupPath" }
$actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $setupPath).Hash.ToLowerInvariant()
if ($actualHash -ne $Matches[1]) { throw "Checksum mismatch: $setupName" }

if ($RequireTrustedSignature) {
  if ([string]::IsNullOrWhiteSpace($ExpectedPublisher)) { throw 'ExpectedPublisher is required when trusted signatures are required.' }
  $signature = Get-AuthenticodeSignature -LiteralPath $setupPath
  if ($signature.Status -ne 'Valid') { throw "A trusted Authenticode signature is required: $setupPath ($($signature.Status))" }
  if ($signature.SignerCertificate.Subject -notlike "*$ExpectedPublisher*") { throw "Unexpected Authenticode publisher: $setupPath" }
}

Write-Host "NSIS release assets verified: $releaseRoot"
