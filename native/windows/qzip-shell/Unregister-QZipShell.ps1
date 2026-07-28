[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction Stop
