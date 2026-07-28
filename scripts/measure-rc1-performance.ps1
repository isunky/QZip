[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Executable,
  [int]$Iterations = 5,
  [string]$OutputPath = 'artifacts/performance/rc1-baseline.json'
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw "Executable was not found: $Executable" }
$results = @()
for ($i = 1; $i -le $Iterations; $i++) {
  $watch = [System.Diagnostics.Stopwatch]::StartNew()
  $process = Start-Process -FilePath $Executable -PassThru
  try {
    do { Start-Sleep -Milliseconds 100; $process.Refresh() } while ($watch.Elapsed.TotalSeconds -lt 20 -and ($process.MainWindowHandle -eq 0 -or -not $process.Responding))
    if ($process.MainWindowHandle -eq 0 -or -not $process.Responding) { throw 'Window did not become responsive within 20 seconds.' }
    Start-Sleep -Seconds 3; $process.Refresh()
    $results += [pscustomobject]@{ launchMilliseconds = [math]::Round($watch.Elapsed.TotalMilliseconds); workingSetBytes = $process.WorkingSet64 }
  } finally { if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force } }
}
$machine = Get-CimInstance Win32_ComputerSystem
$report = [ordered]@{
  schemaVersion = 1; recordedAt = (Get-Date).ToUniversalTime().ToString('o'); executable = (Resolve-Path $Executable).Path
  machine = @{ name = $env:COMPUTERNAME; os = (Get-CimInstance Win32_OperatingSystem).Caption; memoryBytes = $machine.TotalPhysicalMemory }
  iterations = $results; medianLaunchMilliseconds = ($results.launchMilliseconds | Sort-Object)[[math]::Floor(($results.Count - 1) / 2)]
  medianWorkingSetBytes = ($results.workingSetBytes | Sort-Object)[[math]::Floor(($results.Count - 1) / 2)]
}
$target = Join-Path (Split-Path -Parent $PSScriptRoot) $OutputPath
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
$report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $target -Encoding utf8
Write-Host "RC1 performance baseline written: $target"
