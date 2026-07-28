[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$feature = Get-WindowsOptionalFeature -Online -FeatureName "Containers-DisposableClientVM"
if ($feature.State -ne "Enabled") {
  Enable-WindowsOptionalFeature -Online -FeatureName "Containers-DisposableClientVM" -All -NoRestart
  Write-Host "Windows Sandbox was enabled. Restart Windows, then open tests\\acceptance\\windows-sandbox\\QZip-RC1.wsb."
} else {
  Write-Host "Windows Sandbox is already enabled. Open tests\\acceptance\\windows-sandbox\\QZip-RC1.wsb."
}
