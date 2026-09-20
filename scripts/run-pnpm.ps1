[CmdletBinding()]
param(
  [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
  [string[]]$CommandArguments
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$packageManifest = Get-Content -LiteralPath (Join-Path $root 'package.json') -Raw | ConvertFrom-Json
$packageManager = [string]$packageManifest.packageManager
if ($packageManager -notmatch '^pnpm@\d+\.\d+\.\d+(?:-.+)?$') {
  throw "package.json must declare a pinned pnpm packageManager, got '$packageManager'."
}

$corepack = Get-Command corepack -ErrorAction SilentlyContinue
$pnpm = Get-Command pnpm -ErrorAction SilentlyContinue
$npx = Get-Command npx -ErrorAction SilentlyContinue
if ($corepack) {
  & $corepack.Source $packageManager @CommandArguments
} elseif ($pnpm) {
  Write-Warning "Corepack is unavailable; using $packageManager via pnpm dlx. The pinned package-manager version remains enforced."
  & $pnpm.Source dlx $packageManager @CommandArguments
} elseif ($npx) {
  Write-Warning "Corepack and pnpm are unavailable; using $packageManager via npx. The pinned package-manager version remains enforced."
  & $npx.Source --yes $packageManager @CommandArguments
} else {
  throw "Unable to run the pinned workspace package manager ($packageManager). Install Node.js with Corepack, or install pnpm/npx."
}
if ($LASTEXITCODE -ne 0) {
  throw "$packageManager failed with exit code $LASTEXITCODE."
}
