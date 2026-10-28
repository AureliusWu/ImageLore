param(
  [Parameter(Mandatory=$true)][string]$RepositoryRoot,
  [Parameter(Mandatory=$true)][string]$ApplicationPath,
  [Parameter(Mandatory=$true)][string]$InstallerPath,
  [Parameter(Mandatory=$true)][string]$ExpectedVersion,
  [Parameter(Mandatory=$true)][string]$ReleaseRef
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows_release_exceptions.ps1')
$result = Invoke-KnownLimitationsReleaseGate $RepositoryRoot $ApplicationPath $InstallerPath $ExpectedVersion $ReleaseRef
Write-Host ('Windows known-limitations release gate: PASS ' + ($result | ConvertTo-Json -Compress -Depth 5))
