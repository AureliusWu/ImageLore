param(
  [Parameter(Mandatory=$true)][string]$ApplicationPath,
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][string]$ExpectedVersion,
  [string]$PolicyPath = (Join-Path $PSScriptRoot '../src-tauri/windows-signing-policy.json')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows_release_signatures.ps1')
$policy = Get-Content -LiteralPath $PolicyPath -Raw -Encoding UTF8 | ConvertFrom-Json
$result = Invoke-WindowsReleaseSignatureGate $ApplicationPath $InstallerPath $ExpectedVersion $policy
Write-Host ('Windows release Authenticode gate: PASS ' + ($result | ConvertTo-Json -Compress))
