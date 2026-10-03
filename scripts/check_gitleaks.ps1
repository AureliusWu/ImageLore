param([string]$Repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')))
$ErrorActionPreference = 'Stop'
$scannerVersion = '8.30.1'
$scannerDigest = 'd29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e'
$scannerTempBase = [IO.Path]::GetTempPath()
if ($env:RUNNER_TEMP) { $scannerTempBase = $env:RUNNER_TEMP }
$scannerRoot = Join-Path $scannerTempBase ('imagelore-gitleaks-' + [Guid]::NewGuid().ToString('N'))
$scannerArchive = Join-Path $scannerRoot ('gitleaks_' + $scannerVersion + '_windows_x64.zip')
$scannerTool = Join-Path $scannerRoot 'tool'
$scannerReport = Join-Path $scannerRoot 'redacted-findings.json'
New-Item -ItemType Directory -Path $scannerRoot | Out-Null
$scannerUrl = 'https://github.com/gitleaks/gitleaks/releases/download/v' + $scannerVersion + '/gitleaks_' + $scannerVersion + '_windows_x64.zip'
Invoke-WebRequest -Uri $scannerUrl -OutFile $scannerArchive -TimeoutSec 120
if ((Get-FileHash -LiteralPath $scannerArchive -Algorithm SHA256).Hash -ine $scannerDigest) {
  throw 'Pinned Gitleaks archive digest did not match; execution refused'
}
Expand-Archive -LiteralPath $scannerArchive -DestinationPath $scannerTool
$scannerExecutable = Join-Path $scannerTool 'gitleaks.exe'
if (-not (Test-Path -LiteralPath $scannerExecutable -PathType Leaf)) { throw 'Verified scanner executable missing' }
& $scannerExecutable git $Repository --config (Join-Path $Repository '.gitleaks.toml') --log-opts='--all' --redact=100 --no-banner --no-color --log-level=warn --report-format=json --report-path $scannerReport
if ($LASTEXITCODE -ne 0) { throw 'Gitleaks found sensitive information or could not complete its scan' }
# Keep diagnostics outside the source tree; never print or upload the report.
