[CmdletBinding()]
param(
  [switch]$InstallDevCertificate,
  [switch]$CiDevelopmentSigning,
  [switch]$Release,
  [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  $windowsPowerShellModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
  if (-not [string]::IsNullOrWhiteSpace($windowsPowerShellModulePath)) { $env:PSModulePath = $windowsPowerShellModulePath }
}
Import-Module Microsoft.PowerShell.Security -ErrorAction Stop
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path $root 'artifacts\windows-shell' }
$source = Join-Path $root 'native\windows\qzip-shell'
$build = Join-Path $OutputDirectory 'build'

if ($Release -and (-not $env:QZIP_WINDOWS_PFX_PATH -or -not $env:QZIP_WINDOWS_PFX_PASSWORD -or -not $env:QZIP_WINDOWS_PUBLISHER)) {
  throw 'Release builds require QZIP_WINDOWS_PFX_PATH, QZIP_WINDOWS_PFX_PASSWORD, and QZIP_WINDOWS_PUBLISHER. No self-signed fallback is allowed.'
}
if ($Release -and ($InstallDevCertificate -or $CiDevelopmentSigning)) {
  throw 'Release, InstallDevCertificate, and CiDevelopmentSigning cannot be used together.'
}
if ($InstallDevCertificate -and $CiDevelopmentSigning) {
  throw 'InstallDevCertificate and CiDevelopmentSigning cannot be used together.'
}
if ((-not $Release) -and (-not $InstallDevCertificate) -and (-not $CiDevelopmentSigning)) {
  throw 'Non-release sparse MSIX builds require -InstallDevCertificate or -CiDevelopmentSigning.'
}
if ($InstallDevCertificate) {
  # The Certificate provider is not guaranteed to be auto-loaded in a
  # non-interactive PowerShell session (including pnpm-launched builds).
  # Node and pnpm can pass a PowerShell 7 PSModulePath to Windows PowerShell.
  # The inbox Certificate provider was loaded above after normalizing that path.
  Import-Module PKI -ErrorAction Stop
  if (-not (Get-PSProvider -PSProvider Certificate -ErrorAction SilentlyContinue)) {
    throw 'The Windows Certificate provider is unavailable. Run the build from Windows PowerShell with the PKI module installed.'
  }
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

$certificate = $null
$ciCertificate = $null
$ciKey = $null
$ciPfxPath = $null

try {
  if ($InstallDevCertificate) {
    Write-Host 'Preparing trusted local development certificate...'
    $certificate = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq 'CN=QZip Development' -and $_.NotAfter -gt (Get-Date).AddDays(30) } | Select-Object -First 1
    if (-not $certificate) { $certificate = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=QZip Development' -CertStoreLocation 'Cert:\CurrentUser\My' -KeyExportPolicy Exportable -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 }
    foreach ($storeName in @('TrustedPeople', 'Root')) {
      $store = [System.Security.Cryptography.X509Certificates.X509Store]::new($storeName, 'CurrentUser'); $store.Open('ReadWrite'); $store.Add($certificate); $store.Close()
    }
    Export-Certificate -Cert $certificate -FilePath (Join-Path $OutputDirectory 'QZip.Development.cer') -Force | Out-Null
    Write-Host "Installed the QZip development certificate for the current user and exported $OutputDirectory\QZip.Development.cer."
  }
  elseif ($CiDevelopmentSigning) {
    Write-Host 'Creating ephemeral CI development certificate (no certificate-store writes)...'
    $ciKey = [System.Security.Cryptography.RSA]::Create(3072)
    $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=QZip Development', $ciKey, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $false))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new([System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature, $false))
    $usageOids = [System.Security.Cryptography.OidCollection]::new()
    [void]$usageOids.Add([System.Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3', 'Code Signing'))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usageOids, $false))
    $ciCertificate = $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddMinutes(-5), [DateTimeOffset]::UtcNow.AddDays(30))
    $certificate = $ciCertificate
    $passwordBytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($passwordBytes)
    $ciPfxPassword = [Convert]::ToBase64String($passwordBytes)
    $ciPfxPath = Join-Path $env:TEMP ("qzip-ci-development-{0}.pfx" -f [Guid]::NewGuid().ToString('N'))
    [System.IO.File]::WriteAllBytes($ciPfxPath, $ciCertificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $ciPfxPassword))
    [System.IO.File]::WriteAllBytes((Join-Path $OutputDirectory 'QZip.Development.cer'), $ciCertificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Cert))
    Write-Host 'Ephemeral CI development certificate created.'
  }

  Write-Host 'Locating Windows signing and MSIX tools...'
  $signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $signTool) { throw 'Windows SDK x64 signtool.exe was not found.' }
  $makeAppx = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue | Where-Object { $_.FullName -match '\\x64\\makeappx\.exe$' } | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $makeAppx) { throw 'Windows SDK x64 makeappx.exe was not found.' }

  $signingArguments = if ($Release) {
    @('/tr', 'http://timestamp.digicert.com', '/td', 'SHA256', '/f', $env:QZIP_WINDOWS_PFX_PATH, '/p', $env:QZIP_WINDOWS_PFX_PASSWORD)
  }
  elseif ($CiDevelopmentSigning) {
    @('/f', $ciPfxPath, '/p', $ciPfxPassword)
  }
  else {
    @('/sha1', $certificate.Thumbprint)
  }

  Write-Host 'Signing QZip Explorer command DLL...'
  & $signTool.FullName @('sign', '/fd', 'SHA256') @signingArguments $dll
  if ($LASTEXITCODE -ne 0) { throw 'Shell DLL signing failed.' }

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
  $msix = Join-Path $shellOutput 'QZip.Shell.msix'
  Write-Host 'Packing QZip sparse MSIX shell integration...'
  & $makeAppx.FullName pack /o /d $packageRoot /nv /p $msix
  if ($LASTEXITCODE -ne 0) { throw 'Sparse MSIX packaging failed.' }
  Write-Host 'Signing QZip sparse MSIX shell integration...'
  & $signTool.FullName @('sign', '/fd', 'SHA256') @signingArguments $msix
  if ($LASTEXITCODE -ne 0) { throw 'Sparse MSIX signing failed.' }

  foreach ($target in @($dll, $msix)) {
    $signature = Get-AuthenticodeSignature -LiteralPath $target
    if (-not $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint) {
      throw "Signer validation failed for $target."
    }
    if ($Release -and ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike "*$env:QZIP_WINDOWS_PUBLISHER*")) {
      throw "Trusted publisher signature validation failed for $target."
    }
  }
  Write-Host "Shell DLL and sparse MSIX built: $shellOutput"
}
finally {
  if ($ciPfxPath -and (Test-Path -LiteralPath $ciPfxPath)) { Remove-Item -LiteralPath $ciPfxPath -Force }
  if ($ciCertificate) { $ciCertificate.Dispose() }
  if ($ciKey) { $ciKey.Dispose() }
}
