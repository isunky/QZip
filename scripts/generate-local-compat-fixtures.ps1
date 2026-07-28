[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$fixtureRoot = Join-Path $repoRoot "tests\fixtures\compat"
$source = Join-Path ([IO.Path]::GetTempPath()) "qzip-compat-synthetic"
$sevenZip = Join-Path $repoRoot "third_party\7zip\bin\win-x64\7z.exe"
$bandizip = "C:\Program Files\Bandizip\bz.exe"
$tar = "C:\Windows\System32\tar.exe"
if (Test-Path -LiteralPath $source) { Remove-Item -LiteralPath $source -Force -Recurse }
New-Item -ItemType Directory -Force -Path (Join-Path $source "unicode"), (Join-Path $source "empty") | Out-Null
[IO.File]::WriteAllText((Join-Path $source "hello.txt"), "QZip compatibility fixture`n", [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $source "unicode\summary.txt"), "Synthetic Unicode content.`n", [Text.UTF8Encoding]::new($false))

function New-Fixture([string]$File, [scriptblock]$Build) {
  $target = Join-Path $fixtureRoot $File
  if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force }
  & $Build $target
  if (-not (Test-Path -LiteralPath $target -PathType Leaf)) { throw "Fixture was not created: $File" }
  $target
}
$seven7z = New-Fixture "7zip-7z.7z" { param($target) & $sevenZip a -t7z $target (Join-Path $source "*") | Out-Null; if ($LASTEXITCODE) { throw "7-Zip 7z creation failed." } }
$sevenZipFile = New-Fixture "7zip-zip.zip" { param($target) & $sevenZip a -tzip $target (Join-Path $source "*") | Out-Null; if ($LASTEXITCODE) { throw "7-Zip ZIP creation failed." } }
$bandizipFile = $null
if (Test-Path -LiteralPath $bandizip) { $bandizipFile = New-Fixture "bandizip-zip.zip" { param($target) & $bandizip c -fmt:zip -y $target (Join-Path $source "*") | Out-Null; if ($LASTEXITCODE) { throw "Bandizip ZIP creation failed." } } }
$explorerFile = New-Fixture "windows-zip.zip" { param($target); [IO.File]::WriteAllBytes($target, [byte[]](0x50,0x4b,0x05,0x06,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)); $shell = New-Object -ComObject Shell.Application; $shell.Namespace($target).CopyHere($source,16); Start-Sleep -Seconds 2 }
$bsdtarFile = $null
if (Test-Path -LiteralPath $tar) { $bsdtarFile = New-Fixture "windows-bsdtar-xz.tar.xz" { param($target) & $tar -cJf $target -C $source .; if ($LASTEXITCODE) { throw "bsdtar creation failed." } } }

$manifestPath = Join-Path $fixtureRoot "manifest.json"
$manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
foreach ($case in $manifest.cases) { $case | Add-Member -Force -NotePropertyName status -NotePropertyValue "blocked"; $case | Add-Member -Force -NotePropertyName blocker -NotePropertyValue "Producer unavailable on this machine." }
function Mark-Ready([string]$Producer, [string]$File, [string]$Version) { $case = $manifest.cases | Where-Object { $_.producer -eq $Producer -and $_.file -eq $File } | Select-Object -First 1; if ($case) { $case.status = "ready"; $case.blocker = ""; $case.producerVersion = $Version; $case.sha256 = (Get-FileHash -LiteralPath (Join-Path $fixtureRoot $File) -Algorithm SHA256).Hash.ToLowerInvariant() } }
Mark-Ready "7-Zip" "7zip-7z.7z" "QZip bundled 7-Zip"
Mark-Ready "7-Zip" "7zip-zip.zip" "QZip bundled 7-Zip"
if ($bandizipFile) { Mark-Ready "Bandizip" "bandizip-zip.zip" "Bandizip local console" }
Mark-Ready "Windows Explorer" "windows-zip.zip" "Windows 11 Shell.Application"
if ($bsdtarFile) { $case = [pscustomobject]@{ producer="Windows bsdtar"; format="tar.xz"; file="windows-bsdtar-xz.tar.xz"; producerVersion="Windows 11 libarchive"; sha256=(Get-FileHash -LiteralPath $bsdtarFile -Algorithm SHA256).Hash.ToLowerInvariant(); status="ready"; blocker="" }; $manifest.cases += $case }
$manifest.schemaVersion = 2
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding utf8
Write-Host "Generated local compatibility fixtures."
