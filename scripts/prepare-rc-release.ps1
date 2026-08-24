[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$Version,
  [switch]$SkipBundle,
  [switch]$Release,
  [ValidateSet('trusted', 'unsigned-degraded')]
  [string]$SigningMode = 'unsigned-degraded',
  [string]$Publisher
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  $windowsPowerShellModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
  if (-not [string]::IsNullOrWhiteSpace($windowsPowerShellModulePath)) { $env:PSModulePath = $windowsPowerShellModulePath }
}
Import-Module Microsoft.PowerShell.Utility -ErrorAction Stop
$releaseVersion = $Version.Trim()
if ($releaseVersion -match '[\r\n]' -or $releaseVersion.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0) {
  throw 'Version must be safe for a Windows filename.'
}
$repoRoot = Split-Path -Parent $PSScriptRoot
$tauriConfigPath = Join-Path $repoRoot 'apps\desktop\src-tauri\tauri.conf.json'
$tauriConfig = Get-Content -Raw $tauriConfigPath | ConvertFrom-Json
$productVersion = $tauriConfig.version
if ($SigningMode -eq 'trusted' -and [string]::IsNullOrWhiteSpace($Publisher)) {
  throw 'Publisher is required for a trusted release manifest.'
}
if ($Release -and $SigningMode -ne 'trusted') {
  throw 'The legacy -Release switch is only valid with -SigningMode trusted.'
}

& (Join-Path $PSScriptRoot 'fetch-sevenzip.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not $SkipBundle) {
  if ($SigningMode -eq 'trusted') {
    & (Join-Path $PSScriptRoot 'build-nsis.ps1') -Release
  } else {
    & (Join-Path $PSScriptRoot 'build-nsis.ps1') -UnsignedRelease
  }
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$bundleRoot = Join-Path $repoRoot 'target\release\bundle'
$bundleNamePattern = "QZip_${productVersion}_x64-setup.exe"
$nsis = @(Get-ChildItem (Join-Path $bundleRoot 'nsis') -Filter $bundleNamePattern -File -ErrorAction SilentlyContinue)
if ($nsis.Count -ne 1) {
  throw "Expected exactly one NSIS setup EXE for product version $productVersion."
}

$releaseRoot = Join-Path $repoRoot ("artifacts\release\{0}" -f $releaseVersion)
if (Test-Path -LiteralPath $releaseRoot) { Remove-Item -LiteralPath $releaseRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path $releaseRoot | Out-Null
$setupName = "QZip-$releaseVersion-windows-x64-setup.exe"
Copy-Item -LiteralPath $nsis[0].FullName -Destination (Join-Path $releaseRoot $setupName) -Force

$assetNames = @($setupName)
$checksumLines = foreach ($assetName in $assetNames) {
  $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $releaseRoot $assetName)).Hash.ToLowerInvariant()
  "$hash *$assetName"
}
$checksumsPath = Join-Path $releaseRoot 'checksums-sha256.txt'
[System.IO.File]::WriteAllLines($checksumsPath, [string[]]$checksumLines, [System.Text.UTF8Encoding]::new($false))
$commit = (git -C $repoRoot rev-parse HEAD).Trim()
$sidecarManifest = Get-Content -Raw (Join-Path $repoRoot 'third_party\7zip\manifest.json') | ConvertFrom-Json
$degradedFeatures = @()
if ($SigningMode -eq 'unsigned-degraded') {
  $degradedFeatures = @('windows-modern-context-menu')
}
$releaseManifest = [ordered]@{
  version = $releaseVersion
  commit = $commit
  platform = 'windows-x64'
  assets = $assetNames
  signing = [ordered]@{
    mode = $SigningMode
    publisher = if ($SigningMode -eq 'trusted') { $Publisher } else { $null }
  }
  degradedFeatures = $degradedFeatures
  sidecar = [ordered]@{ version = $sidecarManifest.version; fileHashes = $sidecarManifest.runtime.fileHashes }
}
[System.IO.File]::WriteAllText((Join-Path $releaseRoot 'release-manifest.json'), ($releaseManifest | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
Write-Host "Prepared NSIS release assets in $releaseRoot"
