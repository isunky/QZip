[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$InstallPath,
  [Parameter(Mandatory)][string]$PackagePath
)

$ErrorActionPreference = 'Stop'
$logRoot = Join-Path $env:LOCALAPPDATA 'QZip\Logs'
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
$log = Join-Path $logRoot 'shell-registration.log'
try {
  if (-not (Test-Path -LiteralPath $InstallPath -PathType Container)) { throw "QZip install path is missing: $InstallPath" }
  if (-not (Test-Path -LiteralPath $PackagePath -PathType Leaf)) { throw "QZip shell package is missing: $PackagePath" }
  Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction SilentlyContinue
  Add-AppxPackage -Path $PackagePath -ExternalLocation $InstallPath -ErrorAction Stop
  "$(Get-Date -Format o) registered $PackagePath" | Add-Content -LiteralPath $log -Encoding utf8
}
catch {
  "$(Get-Date -Format o) failed: $($_.Exception.Message)" | Add-Content -LiteralPath $log -Encoding utf8
  throw
}
