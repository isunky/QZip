[CmdletBinding(DefaultParameterSetName = 'Increment')]
param(
  [Parameter(Mandatory, ParameterSetName = 'Increment')]
  [ValidateSet('patch', 'minor', 'major')]
  [string]$Increment,

  [Parameter(Mandatory, ParameterSetName = 'Check')]
  [ValidatePattern('^\d+\.\d+\.\d+$')]
  [string]$ExpectedVersion,

  [Parameter(ParameterSetName = 'Check')]
  [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

function Format-Version([version]$Value) {
  return '{0}.{1}.{2}' -f $Value.Major, $Value.Minor, $Value.Build
}

function Get-IncrementedVersion([version]$Value, [string]$Kind) {
  switch ($Kind) {
    'patch' { return [version]::new($Value.Major, $Value.Minor, $Value.Build + 1) }
    'minor' { return [version]::new($Value.Major, $Value.Minor + 1, 0) }
    'major' { return [version]::new($Value.Major + 1, 0, 0) }
  }
  throw "Unsupported version increment: $Kind"
}

function Get-JsonVersion([string]$Path) {
  return [string]((Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json).version)
}

function Set-JsonVersion([string]$Path, [string]$OldVersion, [string]$NewVersion) {
  $content = [System.IO.File]::ReadAllText($Path)
  $pattern = '(?m)(^\s*"version"\s*:\s*")' + [regex]::Escape($OldVersion) + '("\s*,?\s*$)'
  $regex = [regex]::new($pattern)
  if ($regex.Matches($content).Count -ne 1) {
    throw "Expected exactly one version field set to $OldVersion in $Path."
  }
  $updated = $regex.Replace(
    $content,
    { param($match) $match.Groups[1].Value + $NewVersion + $match.Groups[2].Value },
    1
  )
  [System.IO.File]::WriteAllText($Path, $updated, $utf8NoBom)
}

function Set-CargoWorkspaceVersion([string]$Path, [string]$OldVersion, [string]$NewVersion) {
  $content = [System.IO.File]::ReadAllText($Path)
  $pattern = '(?ms)(\[workspace\.package\].*?^version\s*=\s*")' + [regex]::Escape($OldVersion) + '("\s*$)'
  $regex = [regex]::new($pattern)
  if ($regex.Matches($content).Count -ne 1) {
    throw "Expected the workspace package version $OldVersion in $Path."
  }
  $updated = $regex.Replace(
    $content,
    { param($match) $match.Groups[1].Value + $NewVersion + $match.Groups[2].Value },
    1
  )
  [System.IO.File]::WriteAllText($Path, $updated, $utf8NoBom)
}

function Assert-ProjectVersion([string]$Version) {
  $jsonManifests = @(
    (Join-Path $repoRoot 'package.json'),
    (Join-Path $repoRoot 'apps\desktop\package.json'),
    (Join-Path $repoRoot 'packages\ui\package.json'),
    (Join-Path $repoRoot 'apps\desktop\src-tauri\tauri.conf.json')
  )
  foreach ($manifest in $jsonManifests) {
    $actual = Get-JsonVersion $manifest
    if ($actual -ne $Version) {
      throw "Version mismatch in $manifest`: expected $Version, got $actual."
    }
  }

  $cargoManifest = Join-Path $repoRoot 'Cargo.toml'
  $cargoContent = [System.IO.File]::ReadAllText($cargoManifest)
  if ($cargoContent -notmatch ('(?ms)\[workspace\.package\].*?^version\s*=\s*"' + [regex]::Escape($Version) + '"\s*$')) {
    throw "Cargo workspace version does not match $Version."
  }

  $rootPackageContent = [System.IO.File]::ReadAllText((Join-Path $repoRoot 'package.json'))
  if ([regex]::Matches($rootPackageContent, [regex]::Escape("-Version v$Version")).Count -ne 2) {
    throw "Root release commands are not synchronized with v$Version."
  }

  Push-Location $repoRoot
  try {
    $metadataOutput = @(& cargo metadata --no-deps --format-version 1 --locked)
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata --locked failed while checking workspace versions.' }
  }
  finally {
    Pop-Location
  }
  $metadata = ($metadataOutput -join "`n") | ConvertFrom-Json
  $workspaceMembers = @($metadata.workspace_members)
  $workspacePackages = @($metadata.packages | Where-Object { $workspaceMembers -contains $_.id })
  $mismatched = @($workspacePackages | Where-Object { [string]$_.version -ne $Version })
  if ($mismatched.Count -ne 0) {
    throw "Cargo workspace package versions are not synchronized with $Version`: $($mismatched.name -join ', ')."
  }

  $cargoLockContent = [System.IO.File]::ReadAllText((Join-Path $repoRoot 'Cargo.lock'))
  foreach ($package in $workspacePackages) {
    $lockPattern = '(?ms)^\[\[package\]\]\s*\r?\nname\s*=\s*"' + [regex]::Escape([string]$package.name) + '"\s*\r?\nversion\s*=\s*"' + [regex]::Escape($Version) + '"\s*$'
    if ([regex]::Matches($cargoLockContent, $lockPattern).Count -ne 1) {
      throw "Cargo.lock does not contain exactly one $($package.name) workspace package at version $Version."
    }
  }
}

$rootPackagePath = Join-Path $repoRoot 'package.json'
$currentVersionText = Get-JsonVersion $rootPackagePath
if ($currentVersionText -notmatch '^\d+\.\d+\.\d+$') {
  throw "Project version must be stable SemVer: $currentVersionText"
}
$currentVersion = [version]$currentVersionText

if ($PSCmdlet.ParameterSetName -eq 'Check') {
  Assert-ProjectVersion $ExpectedVersion
  Write-Host "Project version is synchronized at $ExpectedVersion."
  exit 0
}

$tagLines = @(& git -C $repoRoot tag --list 'v*')
if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository tags.' }
$stableVersions = @(
  foreach ($tag in $tagLines) {
    if ($tag -match '^v(\d+\.\d+\.\d+)$') { [version]$Matches[1] }
  }
)
$latestVersion = $stableVersions | Sort-Object -Descending | Select-Object -First 1

if ($null -eq $latestVersion) {
  $targetVersion = Get-IncrementedVersion $currentVersion $Increment
}
else {
  $comparison = $currentVersion.CompareTo($latestVersion)
  if ($comparison -lt 0) {
    throw "Project version $currentVersionText is behind the latest stable tag v$(Format-Version $latestVersion)."
  }
  if ($comparison -eq 0) {
    $targetVersion = Get-IncrementedVersion $currentVersion $Increment
  }
  else {
    $pendingVersion = Get-IncrementedVersion $latestVersion $Increment
    if ($currentVersion.CompareTo($pendingVersion) -ne 0) {
      throw "Project version $currentVersionText is ahead of the latest stable tag, but is not its $Increment increment."
    }
    $targetVersion = $currentVersion
  }
}

$targetVersionText = Format-Version $targetVersion
$changed = $targetVersionText -ne $currentVersionText
if ($changed) {
  $jsonManifests = @(
    $rootPackagePath,
    (Join-Path $repoRoot 'apps\desktop\package.json'),
    (Join-Path $repoRoot 'packages\ui\package.json'),
    (Join-Path $repoRoot 'apps\desktop\src-tauri\tauri.conf.json')
  )
  foreach ($manifest in $jsonManifests) {
    Set-JsonVersion $manifest $currentVersionText $targetVersionText
  }

  Set-CargoWorkspaceVersion (Join-Path $repoRoot 'Cargo.toml') $currentVersionText $targetVersionText

  $rootPackageContent = [System.IO.File]::ReadAllText($rootPackagePath)
  $oldReleaseArgument = "-Version v$currentVersionText"
  if ([regex]::Matches($rootPackageContent, [regex]::Escape($oldReleaseArgument)).Count -ne 2) {
    throw "Expected two release commands using $oldReleaseArgument."
  }
  $rootPackageContent = $rootPackageContent.Replace($oldReleaseArgument, "-Version v$targetVersionText")
  [System.IO.File]::WriteAllText($rootPackagePath, $rootPackageContent, $utf8NoBom)

  Push-Location $repoRoot
  try {
    & cargo update -w
    if ($LASTEXITCODE -ne 0) { throw 'cargo update -w failed while refreshing Cargo.lock.' }
  }
  finally {
    Pop-Location
  }
}

Assert-ProjectVersion $targetVersionText

if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_OUTPUT)) {
  "version=$targetVersionText" | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding UTF8
  "tag=v$targetVersionText" | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding UTF8
  "changed=$($changed.ToString().ToLowerInvariant())" | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding UTF8
}

Write-Host "Prepared QZip v$targetVersionText ($Increment)."
