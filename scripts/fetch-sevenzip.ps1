[CmdletBinding()]
param([switch]$VerifyOnly)

$ErrorActionPreference = 'Stop'
$manifestPath = Join-Path $PSScriptRoot '..\third_party\7zip\manifest.json'
$manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
$runtimeDir = Join-Path $PSScriptRoot '..\third_party\7zip\bin\win-x64'
$runtimeDir = [IO.Path]::GetFullPath($runtimeDir)
$sourceDir = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\third_party\7zip\source'))
$sourcePath = Join-Path $sourceDir $manifest.source.archive

function Assert-Hash([string]$Path, [string]$Expected) {
  if (-not (Test-Path -LiteralPath $Path)) { throw "Missing expected file: $Path" }
  $stream = [IO.File]::OpenRead($Path)
  try {
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try { $actual = ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
    finally { $sha256.Dispose() }
  }
  finally { $stream.Dispose() }
  if ($actual -ne $Expected.ToLowerInvariant()) { throw "SHA-256 verification failed for $Path" }
}

if ($VerifyOnly) {
  foreach ($file in $manifest.runtime.files) { if (-not (Test-Path -LiteralPath (Join-Path $runtimeDir $file))) { throw "Missing sidecar file: $file" } }
  Assert-Hash $sourcePath $manifest.source.sha256
  & (Join-Path $runtimeDir '7z.exe') i | Out-Null
  if ($LASTEXITCODE -gt 1) { throw "7-Zip sidecar did not start" }
  Write-Host "7-Zip sidecar is present and runnable."
  exit 0
}

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ('qzip-7zip-' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $tempRoot | Out-Null
try {
  $msiPath = Join-Path $tempRoot $manifest.runtime.archive
  Invoke-WebRequest -Uri $manifest.runtime.url -OutFile $msiPath
  Assert-Hash $msiPath $manifest.runtime.sha256
  $downloadedSource = Join-Path $tempRoot $manifest.source.archive
  Invoke-WebRequest -Uri $manifest.source.url -OutFile $downloadedSource
  Assert-Hash $downloadedSource $manifest.source.sha256
  New-Item -ItemType Directory -Force -Path $sourceDir | Out-Null
  Copy-Item -LiteralPath $downloadedSource -Destination $sourcePath -Force
  $extractDir = Join-Path $tempRoot 'extracted'
  Start-Process -FilePath msiexec.exe -ArgumentList @('/a', $msiPath, '/qn', "TARGETDIR=$extractDir") -Wait -NoNewWindow
  $foundExe = Get-ChildItem -Path $extractDir -Recurse -Filter '7z.exe' | Select-Object -First 1
  $foundDll = Get-ChildItem -Path $extractDir -Recurse -Filter '7z.dll' | Select-Object -First 1
  if ($null -eq $foundExe -or $null -eq $foundDll) { throw 'The official MSI did not contain 7z.exe and 7z.dll.' }
  New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null
  Copy-Item -LiteralPath $foundExe.FullName -Destination (Join-Path $runtimeDir '7z.exe') -Force
  Copy-Item -LiteralPath $foundDll.FullName -Destination (Join-Path $runtimeDir '7z.dll') -Force
  & (Join-Path $runtimeDir '7z.exe') i | Out-Null
  if ($LASTEXITCODE -gt 1) { throw 'Extracted 7-Zip sidecar did not start.' }
  Write-Host "7-Zip $($manifest.version) sidecar extracted to $runtimeDir and source archived locally."
}
finally { Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue }
