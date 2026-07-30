[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Executable,
  [ValidateRange(1, 20)][int]$Iterations = 5,
  [ValidateRange(1, 60)][int]$IdleSeconds = 10,
  [ValidateRange(1000, 200000)][int]$LargeEntryCount = 100000,
  [string]$OutputPath = 'artifacts/performance/rc1-baseline.json',
  [switch]$KeepWorkload
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$resolvedExecutable = (Resolve-Path -LiteralPath $Executable -ErrorAction Stop).Path
$sevenZip = Join-Path $repoRoot 'third_party\7zip\bin\win-x64\7z.exe'
if (-not (Test-Path -LiteralPath $sevenZip -PathType Leaf)) { throw "7-Zip sidecar was not found: $sevenZip" }

function Get-OutputPath([string]$Value) {
  if ([IO.Path]::IsPathRooted($Value)) { return $Value }
  return Join-Path $repoRoot $Value
}
function Get-RunningQZipProcess {
  $name = [IO.Path]::GetFileNameWithoutExtension($resolvedExecutable)
  @(Get-Process -Name $name -ErrorAction SilentlyContinue | Where-Object {
    try { $_.Path -eq $resolvedExecutable } catch { $false }
  })
}
function Stop-TestProcess([Diagnostics.Process]$Process) {
  if ($null -eq $Process) { return }
  $Process.Refresh()
  if ($Process.HasExited) { return }
  $null = $Process.CloseMainWindow()
  if (-not $Process.WaitForExit(5000)) { Stop-Process -Id $Process.Id -Force }
}
function Wait-ForMarker([string]$Path, [string]$Name, [int]$TimeoutSeconds = 45) {
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    if (Test-Path -LiteralPath $Path) {
      foreach ($line in Get-Content -LiteralPath $Path) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try {
          $marker = $line | ConvertFrom-Json
          if ($marker.name -eq $Name) { return $marker }
        } catch { throw "Invalid performance marker JSON: $line" }
      }
    }
    Start-Sleep -Milliseconds 100
  } while ((Get-Date) -lt $deadline)
  throw "Timed out waiting for performance marker '$Name'."
}
function Wait-ForWindowResponsive([Diagnostics.Process]$Process, [long]$StartedAt, [int]$TimeoutSeconds = 45) {
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    $Process.Refresh()
    if ($Process.HasExited) { throw "QZip exited before becoming responsive (exit code $($Process.ExitCode))." }
    if ($Process.MainWindowHandle -ne 0 -and $Process.Responding) {
      return [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() - $StartedAt
    }
    Start-Sleep -Milliseconds 100
  } while ((Get-Date) -lt $deadline)
  throw 'Timed out waiting for the QZip main window.'
}
function Start-MeasuredQZip([string[]]$Arguments) {
  $markerPath = Join-Path ([IO.Path]::GetTempPath()) ("qzip-performance-markers-{0}.jsonl" -f [Guid]::NewGuid())
  $previous = $env:QZIP_PERF_MARKER_PATH
  try {
    $env:QZIP_PERF_MARKER_PATH = $markerPath
    $startedAt = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    if ($Arguments.Count -gt 0) {
      $process = Start-Process -FilePath $resolvedExecutable -ArgumentList $Arguments -PassThru
    } else {
      $process = Start-Process -FilePath $resolvedExecutable -PassThru
    }
  } finally {
    if ($null -eq $previous) { Remove-Item Env:QZIP_PERF_MARKER_PATH -ErrorAction SilentlyContinue } else { $env:QZIP_PERF_MARKER_PATH = $previous }
  }
  [pscustomobject]@{ process = $process; markerPath = $markerPath; startedAt = $startedAt }
}
function Get-Median([object[]]$Values) {
  $sorted = @($Values | Sort-Object)
  return $sorted[[math]::Floor(($sorted.Count - 1) / 2)]
}
function Get-Percentile95([object[]]$Values) {
  $sorted = @($Values | Sort-Object)
  return $sorted[[math]::Ceiling(($sorted.Count - 1) * 0.95)]
}
function New-LargeArchive([string]$Root, [int]$EntryCount) {
  $source = Join-Path $Root 'large-list-source'
  $archive = Join-Path $Root 'large-list.7z'
  New-Item -ItemType Directory -Path $source -Force | Out-Null
  $payload = [byte[]](0x51, 0x5A, 0x49, 0x50, 0x0A)
  for ($index = 1; $index -le $EntryCount; $index++) {
    [IO.File]::WriteAllBytes((Join-Path $source ("entry-{0:D6}.txt" -f $index)), $payload)
  }
  & $sevenZip a -t7z -mx=1 $archive (Join-Path $source '*') | Out-Null
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $archive -PathType Leaf)) { throw 'Unable to create the synthetic large-list archive.' }
  return $archive
}

$existing = Get-RunningQZipProcess
if ($existing.Count -gt 0) { throw 'Close the existing QZip instance before measuring performance.' }
$workRoot = Join-Path ([IO.Path]::GetTempPath()) ("qzip-performance-{0}" -f [Guid]::NewGuid())
$target = Get-OutputPath $OutputPath
$runtimeReportPath = Join-Path $workRoot 'runtime-probe.json'
New-Item -ItemType Directory -Path $workRoot -Force | Out-Null

try {
  $startupRuns = @()
  for ($index = 1; $index -le $Iterations; $index++) {
    $run = Start-MeasuredQZip @()
    try {
      $windowResponsiveMilliseconds = Wait-ForWindowResponsive $run.process $run.startedAt
      $homeMarker = Wait-ForMarker $run.markerPath 'home-interactive'
      Start-Sleep -Seconds $IdleSeconds
      $run.process.Refresh()
      if ($run.process.HasExited) { throw 'QZip exited before the idle-memory sample.' }
      $startupRuns += [pscustomobject]@{
        iteration = $index
        windowResponsiveMilliseconds = $windowResponsiveMilliseconds
        homeInteractiveMilliseconds = [long]$homeMarker.timestampUnixMilliseconds - $run.startedAt
        workingSetBytes = $run.process.WorkingSet64
        privateBytes = $run.process.PrivateMemorySize64
      }
    } finally {
      Stop-TestProcess $run.process
      Remove-Item -LiteralPath $run.markerPath -Force -ErrorAction SilentlyContinue
    }
  }

  $largeArchive = New-LargeArchive $workRoot $LargeEntryCount
  $largeRun = Start-MeasuredQZip @($largeArchive)
  try {
    $windowResponsiveMilliseconds = Wait-ForWindowResponsive $largeRun.process $largeRun.startedAt 90
    $firstPage = Wait-ForMarker $largeRun.markerPath 'archive-list-first-page' 90
    $largeRun.process.Refresh()
    $largeList = [pscustomobject]@{
      entryCount = $LargeEntryCount
      archiveBytes = (Get-Item -LiteralPath $largeArchive).Length
      archiveSha256 = (Get-FileHash -LiteralPath $largeArchive -Algorithm SHA256).Hash.ToLowerInvariant()
      windowResponsiveMilliseconds = $windowResponsiveMilliseconds
      firstPageReadyMilliseconds = [long]$firstPage.timestampUnixMilliseconds - $largeRun.startedAt
      renderedPageLimit = 500
      workingSetBytes = $largeRun.process.WorkingSet64
      privateBytes = $largeRun.process.PrivateMemorySize64
    }
  } finally {
    Stop-TestProcess $largeRun.process
    Remove-Item -LiteralPath $largeRun.markerPath -Force -ErrorAction SilentlyContinue
  }

  $corruptArchive = Join-Path $workRoot 'corrupt.zip'
  [IO.File]::WriteAllBytes($corruptArchive, [byte[]](0x51, 0x5A, 0x49, 0x50))
  $errorRun = Start-MeasuredQZip @($corruptArchive)
  try {
    $errorMarker = Wait-ForMarker $errorRun.markerPath 'archive-error-presented'
    $errorRun.process.Refresh()
    $uiRecovery = [pscustomobject]@{
      corruptArchiveErrorPresentedMilliseconds = [long]$errorMarker.timestampUnixMilliseconds - $errorRun.startedAt
      mainWindowStillRunning = -not $errorRun.process.HasExited
    }
  } finally {
    Stop-TestProcess $errorRun.process
    Remove-Item -LiteralPath $errorRun.markerPath -Force -ErrorAction SilentlyContinue
  }

  & cargo run --quiet -p task-runtime --example performance_probe --release -- $runtimeReportPath
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $runtimeReportPath -PathType Leaf)) { throw 'The task-runtime performance probe failed.' }
  $runtime = Get-Content -LiteralPath $runtimeReportPath -Raw | ConvertFrom-Json
  if (-not $runtime.exceptionRecovery.subsequentTaskCompleted) { throw 'A task could not recover after a synthetic backend failure.' }

  $computer = Get-CimInstance Win32_ComputerSystem
  $os = Get-CimInstance Win32_OperatingSystem
  $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
  $sourceStatus = @(& git -C $repoRoot status --porcelain)
  $homeValues = @($startupRuns | ForEach-Object { $_.homeInteractiveMilliseconds })
  $memoryValues = @($startupRuns | ForEach-Object { $_.workingSetBytes })
  $warnings = @()
  if ((Get-Median $homeValues) -ge 1000) { $warnings += 'Median homepage-interactive time is above the PRD internal 1 second goal.' }
  if ((Get-Median $memoryValues) -ge 104857600) { $warnings += 'Median idle working set is above the PRD internal 100 MB goal.' }
  if ($runtime.progress.dropped -gt 0) { $warnings += 'The runtime probe dropped progress events under a burst; event throttling remains a V1.0 follow-up.' }
  $report = [ordered]@{
    schemaVersion = 2
    recordedAt = (Get-Date).ToUniversalTime().ToString('o')
    release = @{ executable = $resolvedExecutable; executableSha256 = (Get-FileHash -LiteralPath $resolvedExecutable -Algorithm SHA256).Hash.ToLowerInvariant(); sourceCommit = (& git -C $repoRoot rev-parse HEAD).Trim(); sourceTreeDirty = [bool]($sourceStatus.Count -gt 0) }
    machine = @{ name = $env:COMPUTERNAME; os = $os.Caption; osVersion = $os.Version; processor = $cpu.Name; memoryBytes = $computer.TotalPhysicalMemory }
    configuration = @{ iterations = $Iterations; idleSeconds = $IdleSeconds; largeEntryCount = $LargeEntryCount; generatedWorkloadRetained = [bool]$KeepWorkload }
    thresholds = @{ homeInteractiveMilliseconds = 1000; idleWorkingSetBytes = 104857600 }
    startup = @{ runs = $startupRuns; medianHomeInteractiveMilliseconds = Get-Median $homeValues; p95HomeInteractiveMilliseconds = Get-Percentile95 $homeValues; medianWorkingSetBytes = Get-Median $memoryValues; p95WorkingSetBytes = Get-Percentile95 $memoryValues }
    largeArchiveList = $largeList
    uiExceptionRecovery = $uiRecovery
    runtimeProbe = $runtime
    warnings = $warnings
  }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
  $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $target -Encoding utf8
  Write-Host "RC1 performance baseline written: $target"
} finally {
  if (-not $KeepWorkload -and (Test-Path -LiteralPath $workRoot)) { Remove-Item -LiteralPath $workRoot -Force -Recurse }
}
