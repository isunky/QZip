[CmdletBinding()]
param(
  [string]$CertificatePath = "artifacts\windows-shell\QZip.Development.cer"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$certificate = Join-Path $root $CertificatePath
if (-not (Test-Path -LiteralPath $certificate -PathType Leaf)) { throw "Development certificate was not found: $certificate" }
$principal = [Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw "Run this script from an elevated PowerShell window to trust the development MSIX certificate for this machine." }
Import-Certificate -FilePath $certificate -CertStoreLocation "Cert:\LocalMachine\Root" | Out-Null
Write-Host "QZip development certificate was installed in LocalMachine Root."
