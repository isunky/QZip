[CmdletBinding()]
param([string]$RarSample)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$sidecar = Join-Path $root 'third_party\7zip\bin\win-x64'
$work = Join-Path ([IO.Path]::GetTempPath()) ('qzip-m1-' + [Guid]::NewGuid())
$inputDirectoryName = ([string][char]0x8F93) + ([char]0x5165) + ' ' + ([char]0x6587) + ([char]0x4EF6) + ([char]0x5939)
$fixtureFileName = ([string][char]0x4F60) + ([char]0x597D) + ' world.txt'
function Get-Sha256([string]$Path) {
  $stream = [IO.File]::OpenRead($Path)
  try {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-', '') }
    finally { $sha.Dispose() }
  }
  finally { $stream.Dispose() }
}
try {
  New-Item -ItemType Directory -Path $work | Out-Null
  $sourceDirectory = Join-Path $work $inputDirectoryName
  New-Item -ItemType Directory -Path $sourceDirectory | Out-Null
  [IO.File]::WriteAllText((Join-Path $sourceDirectory $fixtureFileName), 'QZip M1 Unicode fixture')
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar capabilities | Select-String '26.02' | Out-Null
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar create --format 7z --output (Join-Path $work 'archive name.7z') $sourceDirectory | Select-String 'completed' | Out-Null
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar list --archive (Join-Path $work 'archive name.7z') | Select-String 'world.txt' | Out-Null
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar test --archive (Join-Path $work 'archive name.7z') | Select-String '"valid":true' | Out-Null
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar extract --archive (Join-Path $work 'archive name.7z') --output (Join-Path $work 'output') | Select-String 'completed' | Out-Null
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar create --format zip --output (Join-Path $work 'archive.zip') $sourceDirectory | Select-String 'completed' | Out-Null
  cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar test --archive (Join-Path $work 'archive.zip') | Select-String '"valid":true' | Out-Null
  $original = Get-Sha256 (Join-Path $sourceDirectory $fixtureFileName)
  $extracted = Get-Sha256 (Join-Path (Join-Path $work 'output') (Join-Path $inputDirectoryName $fixtureFileName))
  if ($original -ne $extracted) { throw 'Extracted Unicode fixture hash differs from source.' }
  $password = 'M1-password-not-for-logs'
  $secretArchive = Join-Path $work 'secret.7z'
  $password | cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar create --format 7z --output $secretArchive --password-stdin $sourceDirectory | Select-String 'completed' | Out-Null
  $password | cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar test --archive $secretArchive --password-stdin | Select-String '"valid":true' | Out-Null
  $wrongOutput = 'incorrect' | cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar test --archive $secretArchive --password-stdin 2>&1
  $wrongExit = $LASTEXITCODE
  $wrongText = $wrongOutput -join "`n"
  if (($wrongExit -ne 4) -or ($wrongText -notmatch 'WRONG_PASSWORD')) { throw 'Wrong password was not mapped to the expected structured error.' }
  if ($wrongText -match $password) { throw 'Password leaked to CLI output.' }
  $corruptArchive = Join-Path $work 'corrupt.7z'
  Copy-Item -LiteralPath (Join-Path $work 'archive name.7z') -Destination $corruptArchive
  $bytes = [IO.File]::ReadAllBytes($corruptArchive)
  $bytes[$bytes.Length - 1] = $bytes[$bytes.Length - 1] -bxor 0xff
  [IO.File]::WriteAllBytes($corruptArchive, $bytes)
  $corruptOutput = cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar test --archive $corruptArchive 2>&1
  $corruptExit = $LASTEXITCODE
  $corruptText = $corruptOutput -join "`n"
  if (($corruptExit -ne 4) -or ($corruptText -notmatch 'CORRUPT_ARCHIVE')) { throw 'Corrupt archive was not mapped to the expected structured error.' }
  if ($RarSample) {
    if (-not (Test-Path -LiteralPath $RarSample -PathType Leaf)) { throw 'RAR sample does not exist.' }
    cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar test --archive $RarSample | Select-String '"valid":true' | Out-Null
    $rarOutput = Join-Path $work 'rar-output'
    cargo run --quiet -p qzip-cli -- --sevenzip-dir $sidecar extract --archive $RarSample --output $rarOutput | Select-String 'completed' | Out-Null
    if ((Get-ChildItem -LiteralPath $rarOutput -Recurse -File | Measure-Object).Count -lt 1) { throw 'RAR extraction produced no files.' }
  }
  Write-Host '7-Zip integration test passed.'
}
finally { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
