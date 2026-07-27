[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidatePattern('^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$')]
  [string]$Version,
  [switch]$SkipBundle
)

$ErrorActionPreference = 'Stop'
$releaseVersion = $Version.TrimStart('v')
$productVersion = ($releaseVersion -split '-', 2)[0]
$repoRoot = Split-Path -Parent $PSScriptRoot
$tauriConfigPath = Join-Path $repoRoot 'apps\desktop\src-tauri\tauri.conf.json'
$tauriConfig = Get-Content -Raw $tauriConfigPath | ConvertFrom-Json
if ($tauriConfig.version -ne $productVersion) {
  throw "Requested release $releaseVersion requires MSI-compatible product version $productVersion, but tauri.conf.json has $($tauriConfig.version)."
}

& (Join-Path $PSScriptRoot 'fetch-sevenzip.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not $SkipBundle) {
  & (Join-Path $PSScriptRoot 'bundle-windows.ps1')
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$bundleRoot = Join-Path $repoRoot 'target\release\bundle'
$bundleNamePattern = 'QZip_{0}_*' -f $productVersion
$nsis = @(Get-ChildItem (Join-Path $bundleRoot 'nsis') -Filter '*.exe' -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -like $bundleNamePattern })
$msi = @(Get-ChildItem (Join-Path $bundleRoot 'msi') -Filter '*.msi' -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -like $bundleNamePattern })
if ($nsis.Count -ne 1 -or $msi.Count -ne 1) {
  throw "Expected exactly one NSIS EXE and one MSI for product version $productVersion."
}

$desktopExecutable = Join-Path $repoRoot 'target\release\qzip-desktop.exe'
if (-not (Test-Path -LiteralPath $desktopExecutable -PathType Leaf)) {
  throw "Desktop executable was not found: $desktopExecutable"
}

$releaseRoot = Join-Path $repoRoot ("artifacts\release\{0}" -f $releaseVersion)
New-Item -ItemType Directory -Force -Path $releaseRoot | Out-Null
$setupName = "QZip-$releaseVersion-windows-x64-setup.exe"
$msiName = "QZip-$releaseVersion-windows-x64.msi"
$portableName = "QZip-$releaseVersion-windows-x64-portable.zip"
Copy-Item -LiteralPath $nsis[0].FullName -Destination (Join-Path $releaseRoot $setupName) -Force
Copy-Item -LiteralPath $msi[0].FullName -Destination (Join-Path $releaseRoot $msiName) -Force

$portableRoot = Join-Path $releaseRoot 'portable-content'
if (Test-Path -LiteralPath $portableRoot) { Remove-Item -LiteralPath $portableRoot -Recurse -Force }
New-Item -ItemType Directory -Path $portableRoot | Out-Null
Copy-Item -LiteralPath $desktopExecutable -Destination (Join-Path $portableRoot 'QZip.exe')
Copy-Item -LiteralPath (Join-Path $repoRoot 'third_party\7zip\bin\win-x64') -Destination (Join-Path $portableRoot '7zip') -Recurse
$portablePath = Join-Path $releaseRoot $portableName
if (Test-Path -LiteralPath $portablePath) { Remove-Item -LiteralPath $portablePath -Force }
Compress-Archive -Path (Join-Path $portableRoot '*') -DestinationPath $portablePath -CompressionLevel Optimal
Remove-Item -LiteralPath $portableRoot -Recurse -Force

$assetNames = @($setupName, $msiName, $portableName)
$checksumLines = foreach ($assetName in $assetNames) {
  $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $releaseRoot $assetName)).Hash.ToLowerInvariant()
  "$hash *$assetName"
}
$checksumsPath = Join-Path $releaseRoot 'checksums-sha256.txt'
[System.IO.File]::WriteAllLines($checksumsPath, [string[]]$checksumLines, [System.Text.UTF8Encoding]::new($false))
$commit = (git -C $repoRoot rev-parse HEAD).Trim()
$sidecarManifest = Get-Content -Raw (Join-Path $repoRoot 'third_party\7zip\manifest.json') | ConvertFrom-Json
$releaseManifest = [ordered]@{
  version = $releaseVersion
  commit = $commit
  platform = 'windows-x64'
  assets = $assetNames
  sidecar = [ordered]@{ version = $sidecarManifest.version; fileHashes = $sidecarManifest.runtime.fileHashes }
}
[System.IO.File]::WriteAllText((Join-Path $releaseRoot 'release-manifest.json'), ($releaseManifest | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))
Write-Host "Prepared release assets in $releaseRoot"
