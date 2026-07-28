[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$InstallPath,
  [Parameter(Mandatory)][string]$PackagePath
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $InstallPath -PathType Container)) { throw "QZip install path is missing: $InstallPath" }
if (-not (Test-Path -LiteralPath $PackagePath -PathType Leaf)) { throw "QZip shell package is missing: $PackagePath" }
Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction SilentlyContinue
Add-AppxPackage -Path $PackagePath -ExternalLocation $InstallPath -ErrorAction Stop
