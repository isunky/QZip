[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$Version,
  [switch]$RequireTrustedSignature,
  [string]$ExpectedPublisher,
  [ValidateSet('trusted', 'unsigned-degraded')]
  [string]$ExpectedSigningMode = 'unsigned-degraded'
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  $windowsPowerShellModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
  if (-not [string]::IsNullOrWhiteSpace($windowsPowerShellModulePath)) { $env:PSModulePath = $windowsPowerShellModulePath }
}
Import-Module Microsoft.PowerShell.Utility -ErrorAction Stop
Import-Module Microsoft.PowerShell.Security -ErrorAction Stop
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
if ($manifest.signing.mode -ne $ExpectedSigningMode) {
  throw "Release signing mode mismatch: expected $ExpectedSigningMode, got $($manifest.signing.mode)."
}
$degradedFeatures = @($manifest.degradedFeatures)
if ($ExpectedSigningMode -eq 'unsigned-degraded') {
  if ($degradedFeatures.Count -ne 1 -or $degradedFeatures[0] -ne 'windows-modern-context-menu') {
    throw 'Unsigned-degraded releases must declare the windows-modern-context-menu degradation.'
  }
  if (-not [string]::IsNullOrWhiteSpace([string]$manifest.signing.publisher)) {
    throw 'Unsigned-degraded releases must not declare a trusted publisher.'
  }
}
elseif ($degradedFeatures.Count -ne 0) {
  throw 'Trusted releases must not declare degraded features.'
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

if ($RequireTrustedSignature -or $ExpectedSigningMode -eq 'trusted') {
  if ([string]::IsNullOrWhiteSpace($ExpectedPublisher)) { throw 'ExpectedPublisher is required when trusted signatures are required.' }
  if ($manifest.signing.publisher -ne $ExpectedPublisher) { throw 'Release manifest publisher does not match ExpectedPublisher.' }
  $signature = Get-AuthenticodeSignature -LiteralPath $setupPath
  if ($signature.Status -ne 'Valid') { throw "A trusted Authenticode signature is required: $setupPath ($($signature.Status))" }
  if ($signature.SignerCertificate.Subject -notlike "*$ExpectedPublisher*") { throw "Unexpected Authenticode publisher: $setupPath" }
}
else {
  $signature = Get-AuthenticodeSignature -LiteralPath $setupPath
  if ($signature.Status -ne 'NotSigned') { throw "Unsigned-degraded installer must be unsigned: $setupPath ($($signature.Status))" }
}

$sevenZip = Join-Path $repoRoot 'third_party\7zip\bin\win-x64\7z.exe'
if (-not (Test-Path -LiteralPath $sevenZip -PathType Leaf)) { throw "7-Zip release verifier is missing: $sevenZip" }
$payloadListing = @(& $sevenZip l -slt $setupPath)
if ($LASTEXITCODE -ne 0) { throw "Unable to inspect NSIS installer payload: $setupPath" }
if ($ExpectedSigningMode -eq 'unsigned-degraded') {
  $forbiddenPayload = @($payloadListing | Select-String -Pattern 'QZip\.Shell\.msix|qzip-shell\.dll|Path = qzip-shell\\Register-QZipShell\.ps1$')
  if ($forbiddenPayload.Count -ne 0) { throw 'Unsigned-degraded installer contains trusted-shell-only payload.' }
  $requiredPayload = @(
    'Path = qzip-shell\\Unregister-QZipShell\.ps1$',
    'Path = file-icons\\zip\.ico$',
    'Path = 7zip\\7z\.exe$'
  )
  foreach ($pattern in $requiredPayload) {
    if (-not ($payloadListing | Select-String -Pattern $pattern)) {
      throw "Unsigned-degraded installer is missing required payload matching: $pattern"
    }
  }
}
else {
  $requiredPayload = @(
    'Path = qzip-shell\\QZip\.Shell\.msix$',
    'Path = qzip-shell\\qzip-shell\.dll$',
    'Path = qzip-shell\\Register-QZipShell\.ps1$',
    'Path = qzip-shell\\Unregister-QZipShell\.ps1$'
  )
  foreach ($pattern in $requiredPayload) {
    if (-not ($payloadListing | Select-String -Pattern $pattern)) {
      throw "Trusted installer is missing required shell payload matching: $pattern"
    }
  }
}

Write-Host "NSIS release assets verified: $releaseRoot"
