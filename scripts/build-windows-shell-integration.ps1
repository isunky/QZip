[CmdletBinding()]
param(
  [switch]$InstallDevCertificate,
  [switch]$Release,
  [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $root 'artifacts\windows-shell' }
$source = Join-Path $root 'native\windows\qzip-shell'
$build = Join-Path $OutputDirectory 'build'

if ($Release -and (-not $env:QZIP_WINDOWS_PFX_PATH -or -not $env:QZIP_WINDOWS_PFX_PASSWORD -or -not $env:QZIP_WINDOWS_PUBLISHER)) {
  throw 'Release builds require QZIP_WINDOWS_PFX_PATH, QZIP_WINDOWS_PFX_PASSWORD, and QZIP_WINDOWS_PUBLISHER. No self-signed fallback is allowed.'
}
if ((-not $Release) -and (-not $InstallDevCertificate)) {
  throw 'Local sparse MSIX builds require -InstallDevCertificate. This explicitly creates/trusts the development certificate for the current user.'
}

$cmake = (Get-Command cmake -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if (-not $cmake) {
  $cmake = (Get-ChildItem 'C:\Program Files (x86)\Microsoft Visual Studio' -Recurse -Filter cmake.exe -ErrorAction SilentlyContinue | Select-Object -First 1).FullName
}
if (-not $cmake) { throw 'CMake was not found. Install Visual Studio CMake tools or add cmake.exe to PATH.' }
& $cmake -S $source -B $build -G 'Visual Studio 17 2022' -A x64
if ($LASTEXITCODE -ne 0) { throw 'CMake project generation failed.' }
& $cmake --build $build --config Release
if ($LASTEXITCODE -ne 0) { throw 'Shell DLL compilation failed.' }

$dll = Join-Path $build 'Release\qzip-shell.dll'
if (-not (Test-Path -LiteralPath $dll)) { throw "Shell DLL was not produced: $dll" }

if ($InstallDevCertificate) {
  $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq 'CN=QZip Development' -and $_.NotAfter -gt (Get-Date).AddDays(30) } | Select-Object -First 1
  if (-not $cert) { $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=QZip Development' -CertStoreLocation 'Cert:\CurrentUser\My' -KeyExportPolicy Exportable -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 }
  foreach ($storeName in @('TrustedPeople', 'Root')) {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new($storeName, 'CurrentUser'); $store.Open('ReadWrite'); $store.Add($cert); $store.Close()
  }
  Export-Certificate -Cert $cert -FilePath (Join-Path $OutputDirectory 'QZip.Development.cer') -Force | Out-Null
  Write-Host "Installed the QZip development certificate for the current user and exported $OutputDirectory\QZip.Development.cer. Machine-wide MSIX testing also requires scripts\install-qzip-development-certificate.ps1 from an elevated PowerShell window."
}

 $signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signTool) { throw 'Windows SDK x64 signtool.exe was not found.' }
if ($Release) {
  & $signTool.FullName sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $env:QZIP_WINDOWS_PFX_PATH /p $env:QZIP_WINDOWS_PFX_PASSWORD $dll
  if ($LASTEXITCODE -ne 0) { throw 'Shell DLL release signing failed.' }
} else {
  $certificate = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq 'CN=QZip Development' } | Select-Object -First 1
  & $signTool.FullName sign /fd SHA256 /sha1 $certificate.Thumbprint $dll
  if ($LASTEXITCODE -ne 0) { throw 'Shell DLL development signing failed.' }
}

$shellOutput = Join-Path $OutputDirectory 'qzip-shell'
$packageRoot = Join-Path $build 'sparse-package'
New-Item -ItemType Directory -Force -Path $shellOutput, (Join-Path $packageRoot 'Assets'), (Join-Path $packageRoot 'qzip-shell') | Out-Null
Copy-Item -LiteralPath $dll -Destination (Join-Path $shellOutput 'qzip-shell.dll') -Force
# The sparse package supplies the application identity, but Explorer activates
# the IExplorerCommand COM server from the package payload. Keep a copy beside
# the installed application for diagnostics and place the signed DLL in the
# MSIX at the manifest-declared path for actual shell activation.
Copy-Item -LiteralPath $dll -Destination (Join-Path $packageRoot 'qzip-shell\qzip-shell.dll') -Force
$publisher = if ($Release) { $env:QZIP_WINDOWS_PUBLISHER } else { 'CN=QZip Development' }
$manifest = (Get-Content -Raw (Join-Path $source 'AppxManifest.xml.in')).Replace('@PUBLISHER@', $publisher)
Set-Content -LiteralPath (Join-Path $packageRoot 'AppxManifest.xml') -Value $manifest -Encoding utf8
$iconRoot = Join-Path $root 'apps\desktop\src-tauri\icons'
$packageLogo = Join-Path $iconRoot 'Square150x150Logo.png'
$contextMenuIcon = Join-Path $iconRoot 'Square44x44Logo.png'
if (-not (Test-Path -LiteralPath $packageLogo -PathType Leaf)) { throw "Package logo was not found: $packageLogo" }
if (-not (Test-Path -LiteralPath $contextMenuIcon -PathType Leaf)) { throw "Context-menu icon was not found: $contextMenuIcon" }
Copy-Item -LiteralPath $packageLogo -Destination (Join-Path $packageRoot 'Assets\Logo.png') -Force
Copy-Item -LiteralPath $contextMenuIcon -Destination (Join-Path $packageRoot 'Assets\ContextMenuIcon.png') -Force
$makeAppx = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match '\\x64\\makeappx\.exe$' } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeAppx) { throw 'Windows SDK x64 makeappx.exe was not found.' }
$msix = Join-Path $shellOutput 'QZip.Shell.msix'
& $makeAppx.FullName pack /o /d $packageRoot /nv /p $msix
if ($LASTEXITCODE -ne 0) { throw 'Sparse MSIX packaging failed.' }
if ($Release) { & $signTool.FullName sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $env:QZIP_WINDOWS_PFX_PATH /p $env:QZIP_WINDOWS_PFX_PASSWORD $msix }
else { & $signTool.FullName sign /fd SHA256 /sha1 $certificate.Thumbprint $msix }
if ($LASTEXITCODE -ne 0) { throw 'Sparse MSIX signing failed.' }
if ($Release) {
  foreach ($target in @($dll, $msix)) {
    $signature = Get-AuthenticodeSignature -LiteralPath $target
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike "*$env:QZIP_WINDOWS_PUBLISHER*") {
      throw "Trusted publisher signature validation failed for $target."
    }
  }
}
Write-Host "Shell DLL and sparse MSIX built: $shellOutput"
