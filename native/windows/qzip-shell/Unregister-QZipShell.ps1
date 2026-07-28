[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$logRoot = Join-Path $env:LOCALAPPDATA 'QZip\Logs'
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
$log = Join-Path $logRoot 'shell-registration.log'
try {
  Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction Stop
  "$(Get-Date -Format o) unregistered QZip Shell package" | Add-Content -LiteralPath $log -Encoding utf8
}
catch {
  "$(Get-Date -Format o) uninstall failed: $($_.Exception.Message)" | Add-Content -LiteralPath $log -Encoding utf8
  throw
}
