[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidatePattern('^v\d+\.\d+\.\d+$')]
  [string]$Version,

  [Parameter(Mandatory)]
  [ValidatePattern('^[0-9a-fA-F]{40}$')]
  [string]$ReleaseSha,

  [Parameter(Mandatory)]
  [ValidateSet('trusted', 'unsigned-degraded')]
  [string]$SigningMode,

  [Parameter(Mandatory)]
  [ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')]
  [string]$Repository,

  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$OutputPath,

  [string]$Publisher
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

function Invoke-Git([string[]]$Arguments) {
  $output = @(& git -C $repoRoot @Arguments)
  if ($LASTEXITCODE -ne 0) {
    throw "Git command failed: git $($Arguments -join ' ')"
  }
  return $output
}

function Escape-Markdown([string]$Value) {
  return $Value.Replace('\', '\\').Replace('[', '\[').Replace(']', '\]')
}

if ($SigningMode -eq 'trusted' -and [string]::IsNullOrWhiteSpace($Publisher)) {
  throw 'Publisher is required when generating notes for a trusted release.'
}

Invoke-Git @('cat-file', '-e', "$ReleaseSha`^{commit}") | Out-Null
$stableTags = @(Invoke-Git @('tag', '--merged', $ReleaseSha, '--sort=-version:refname') | Where-Object {
  $_ -match '^v\d+\.\d+\.\d+$' -and $_ -ne $Version
})
$previousTag = if ($stableTags.Count -gt 0) { $stableTags[0] } else { $null }
$revision = if ($previousTag) { "$previousTag..$ReleaseSha" } else { $ReleaseSha }

$commitLines = @(Invoke-Git @('log', '--first-parent', '--format=%H%x1f%s', $revision))
$changes = [System.Collections.Generic.List[object]]::new()
foreach ($line in $commitLines) {
  $parts = $line -split ([char]0x1f), 2
  if ($parts.Count -ne 2) { continue }
  $sha = $parts[0].Trim()
  $subject = $parts[1].Trim()
  if ([string]::IsNullOrWhiteSpace($subject) -or $subject -match '^chore\(release\):\s+v\d+\.\d+\.\d+$') {
    continue
  }
  $changes.Add([pscustomobject]@{ Sha = $sha; Subject = $subject })
}

$notes = [System.Collections.Generic.List[string]]::new()
$notes.Add('## 更新记录 / What''s Changed')
$notes.Add('')
if ($changes.Count -eq 0) {
  $notes.Add('- 版本维护、依赖更新与发布构建改进。')
} else {
  foreach ($change in $changes) {
    $shortSha = $change.Sha.Substring(0, 7)
    $subject = Escape-Markdown $change.Subject
    $notes.Add("- $subject ([``$shortSha``](https://github.com/$Repository/commit/$($change.Sha)))")
  }
}

$notes.Add('')
$notes.Add('## Windows 发布信息 / Windows Release')
$notes.Add('')
if ($SigningMode -eq 'trusted') {
  $notes.Add('- 签名状态：受信任的 Authenticode 签名。')
  $notes.Add("- 发布者：``$Publisher``。")
  $notes.Add('- Windows 11 一级现代右键菜单、应用界面和文件关联均可用。')
} else {
  $notes.Add('> [!WARNING]')
  $notes.Add('> 此版本未使用受信任的 Authenticode 证书签名，Windows 可能显示“未知发布者”或 SmartScreen 提示。')
  $notes.Add('> Windows 11 一级现代右键菜单不可用；应用界面、“打开方式/QZip”和文件关联仍可正常使用。')
  $notes.Add('> This build is unsigned and feature-degraded. Core archive features and file associations remain available.')
}

$notes.Add('')
$notes.Add('## 下载与校验 / Download & Verification')
$notes.Add('')
$notes.Add("- Windows 安装包：``QZip-$Version-windows-x64-setup.exe``")
$notes.Add('- SHA-256：`checksums-sha256.txt`')
$notes.Add('- 发布清单：`release-manifest.json`')
$notes.Add('- 构建来源证明由 GitHub Artifact Attestations 提供。')
$notes.Add('')
if ($previousTag) {
  $notes.Add("**完整变更 / Full Changelog**: [$previousTag...$Version](https://github.com/$Repository/compare/$previousTag...$Version)")
} else {
  $notes.Add("**源代码 / Source**: [``$($ReleaseSha.Substring(0, 7))``](https://github.com/$Repository/commit/$ReleaseSha)")
}
$notes.Add('')
$notes.Add("[项目变更记录 / Project changelog](https://github.com/$Repository/blob/$Version/CHANGELOG.md)")

$parent = Split-Path -Parent $OutputPath
if (-not [string]::IsNullOrWhiteSpace($parent)) {
  [IO.Directory]::CreateDirectory($parent) | Out-Null
}
[IO.File]::WriteAllLines($OutputPath, [string[]]$notes, $utf8NoBom)

Write-Host "Generated release notes for $Version from $($changes.Count) update records."
if ($previousTag) { Write-Host "Previous stable tag: $previousTag" }
