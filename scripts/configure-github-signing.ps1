[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateScript({ Test-Path -LiteralPath $_ -PathType Leaf })]
  [string]$PfxPath,
  [string]$Repository
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  $windowsPowerShellModulePath = [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')
  if (-not [string]::IsNullOrWhiteSpace($windowsPowerShellModulePath)) { $env:PSModulePath = $windowsPowerShellModulePath }
}
Import-Module Microsoft.PowerShell.Security -ErrorAction Stop
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'GitHub CLI (gh) is required.' }
$resolvedPfx = (Resolve-Path -LiteralPath $PfxPath).Path
$password = Read-Host 'PFX password' -AsSecureString
$certificate = Get-PfxCertificate -FilePath $resolvedPfx -Password $password
if (-not $certificate.HasPrivateKey) { throw 'The selected PFX does not contain a private key.' }
if ($certificate.NotAfter -le (Get-Date).AddDays(30)) { throw "The signing certificate expires too soon: $($certificate.NotAfter)" }
if (-not (@($certificate.EnhancedKeyUsageList | ForEach-Object { $_.ObjectId.Value }) -contains '1.3.6.1.5.5.7.3.3')) {
  throw 'The certificate does not include the Code Signing enhanced key usage.'
}
if ([string]::IsNullOrWhiteSpace($Repository)) {
  $Repository = (& gh repo view --json nameWithOwner --jq '.nameWithOwner').Trim()
  if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($Repository)) { throw 'Unable to determine the GitHub repository.' }
}

$plainPassword = [System.Net.NetworkCredential]::new('', $password).Password
try {
  $pfxBase64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($resolvedPfx))
  $pfxBase64 | & gh secret set WINDOWS_PFX_BASE64 --repo $Repository
  if ($LASTEXITCODE -ne 0) { throw 'Failed to configure WINDOWS_PFX_BASE64.' }
  $plainPassword | & gh secret set WINDOWS_PFX_PASSWORD --repo $Repository
  if ($LASTEXITCODE -ne 0) { throw 'Failed to configure WINDOWS_PFX_PASSWORD.' }
  & gh variable set QZIP_WINDOWS_PUBLISHER --body $certificate.Subject --repo $Repository
  if ($LASTEXITCODE -ne 0) { throw 'Failed to configure QZIP_WINDOWS_PUBLISHER.' }
}
finally {
  $plainPassword = $null
  $pfxBase64 = $null
}

Write-Host "Configured QZip signing metadata for $($certificate.Subject) in $Repository. The PFX and password were not written to the repository."
