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
  $store = [System.Security.Cryptography.X509Certificates.X509Store]::new('TrustedPeople', 'CurrentUser'); $store.Open('ReadWrite'); $store.Add($cert); $store.Close()
  Write-Host "Installed a QZip development certificate for the current user: $($cert.Thumbprint)"
}

if ($Release) {
  $signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $signTool) { throw 'Windows SDK signtool.exe was not found.' }
  & $signTool.FullName sign /fd SHA256 /f $env:QZIP_WINDOWS_PFX_PATH /p $env:QZIP_WINDOWS_PFX_PASSWORD $dll
  if ($LASTEXITCODE -ne 0) { throw 'Shell DLL release signing failed.' }
} elseif ($InstallDevCertificate) {
  $certificate = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq 'CN=QZip Development' } | Select-Object -First 1
  Set-AuthenticodeSignature -FilePath $dll -Certificate $certificate | Out-Null
}

Write-Host "Shell DLL built: $dll"
Write-Host 'Sparse MSIX signing and registration are explicit deployment steps; this script does not change package registration by default.'
