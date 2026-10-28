$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows_release_exceptions.ps1')
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Actual unsigned VERSIONINFO PE regression requires Windows' }
$caseCount = 0
function Confirm-Allowed([string]$Name, [scriptblock]$Action) {
  & $Action
  $script:caseCount++
}
function Confirm-Refused([string]$Name, [string]$Reason, [scriptblock]$Action) {
  $refused = $false
  try { & $Action } catch {
    if ($_.Exception.Message -notlike ('*' + $Reason + '*')) { throw "Unexpected refusal for $Name" }
    $refused = $true
  }
  if (-not $refused) { throw "Known-limitations release gate allowed $Name" }
  $script:caseCount++
}
function Get-FileDigest([string]$Path) {
  $held = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try { return Get-ReleaseStreamHash $held } finally { $held.Dispose() }
}
function Write-Utf8([string]$Path, [string]$Text) {
  [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}
$base = if ($env:IMAGELORE_RELEASE_EXCEPTION_TEST_ROOT) { $env:IMAGELORE_RELEASE_EXCEPTION_TEST_ROOT } elseif ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$root = [IO.Path]::GetFullPath((Join-Path $base ('imagelore-release-exception-' + [Guid]::NewGuid().ToString('N'))))
if (Test-Path -LiteralPath $root) { throw 'Release exception regression directory already exists' }
New-Item -ItemType Directory -Path (Join-Path $root 'src-tauri') | Out-Null
New-Item -ItemType Directory -Path (Join-Path $root 'docs/releases') | Out-Null
$notesPath = Join-Path $root 'docs/releases/v1.0.0.md'
$policyPath = Join-Path $root 'src-tauri/release-exceptions.json'
$validNotes = "# ImageLore v1.0.0`n`nStatus: RELEASED_WITH_WARNINGS`n`nSynthetic test notes; no actual release approval.`n"
Write-Utf8 $notesPath $validNotes
$validPolicy = [pscustomobject]@{
  schemaVersion = 1
  productName = 'ImageLore'
  version = '1.0.0'
  tag = 'refs/tags/v1.0.0'
  status = 'RELEASED_WITH_WARNINGS'
  releaseNotesPath = 'docs/releases/v1.0.0.md'
  releaseNotesSha256 = Get-FileDigest $notesPath
  limitations = @('unsigned-windows-binaries', 'manual-updates-only', 'desktop-and-performance-acceptance-deferred', 'third-party-inventory-not-link-map')
}
function Restore-Policy {
  Write-Utf8 $notesPath $validNotes
  Write-Utf8 $policyPath ($validPolicy | ConvertTo-Json -Depth 5)
}
Restore-Policy
Confirm-Allowed 'exact version and tag with byte-bound notes' {
  if ((Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0') -cne 'UNSIGNED_KNOWN_LIMITATIONS') { throw 'Exact release policy was not selected' }
}
foreach ($case in @(
  @{ version = '1.0.1'; ref = 'refs/tags/v1.0.1' },
  @{ version = '1.0.0-rc.1'; ref = 'refs/tags/v1.0.0-rc.1' },
  @{ version = '1.0.0+metadata'; ref = 'refs/tags/v1.0.0+metadata' },
  @{ version = '1.0.0'; ref = 'refs/heads/main' },
  @{ version = '1.0.0'; ref = 'refs/tags/v1.0.1' },
  @{ version = '1.0.0'; ref = 'refs/tags/V1.0.0' },
  @{ version = '0.26.0'; ref = 'refs/tags/v0.26.0' }
)) {
  Confirm-Allowed 'all other versions and refs retain strict signing' {
    if ((Get-ReleasePublicationMode $root $case.version $case.ref) -cne 'SIGNED_REQUIRED') { throw 'An unauthorized version or ref received the release exception' }
  }
}
foreach ($case in @(
  @{ field = 'schemaVersion'; value = 2; reason = 'schema' },
  @{ field = 'schemaVersion'; value = '1'; reason = 'schema' },
  @{ field = 'productName'; value = 'Other'; reason = 'authorized v1.0.0' },
  @{ field = 'version'; value = '1.0.1'; reason = 'authorized v1.0.0' },
  @{ field = 'tag'; value = 'v1.0.0'; reason = 'authorized v1.0.0' },
  @{ field = 'status'; value = 'RELEASED'; reason = 'authorized v1.0.0' },
  @{ field = 'releaseNotesPath'; value = '../README.md'; reason = 'fixed location' },
  @{ field = 'releaseNotesSha256'; value = ('a' * 64); reason = 'approved SHA-256' },
  @{ field = 'releaseNotesSha256'; value = 'invalid'; reason = 'SHA-256 is invalid' },
  @{ field = 'limitations'; value = @('unsigned-windows-binaries'); reason = 'four items' },
  @{ field = 'limitations'; value = @('unsigned-windows-binaries', 'manual-updates-only', 'desktop-and-performance-acceptance-deferred', 'unexpected'); reason = 'four items' },
  @{ field = 'limitations'; value = @('unsigned-windows-binaries', 'manual-updates-only', 'desktop-and-performance-acceptance-deferred', 'manual-updates-only'); reason = 'four items' }
)) {
  $changed = $validPolicy.PSObject.Copy()
  $changed.($case.field) = $case.value
  Write-Utf8 $policyPath ($changed | ConvertTo-Json -Depth 5)
  Confirm-Refused ('invalid policy ' + $case.field) $case.reason { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
  Restore-Policy
}
$extra = $validPolicy.PSObject.Copy()
$extra | Add-Member -NotePropertyName allowUnsigned -NotePropertyValue $true
Write-Utf8 $policyPath ($extra | ConvertTo-Json -Depth 5)
Confirm-Refused 'generic allow flag refused' 'unexpected fields' { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
Restore-Policy
$missing = $validPolicy.PSObject.Copy()
$missing.PSObject.Properties.Remove('limitations')
Write-Utf8 $policyPath ($missing | ConvertTo-Json -Depth 5)
Confirm-Refused 'missing limitation disclosure field' 'unexpected fields' { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
Restore-Policy
Write-Utf8 $notesPath ($validNotes + "tampered`n")
Confirm-Refused 'notes changed after policy approval' 'approved SHA-256' { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
Restore-Policy
Write-Utf8 $notesPath ($validNotes.Replace("`n", "`r`n"))
$crlf = $validPolicy.PSObject.Copy()
$crlf.releaseNotesSha256 = Get-FileDigest $notesPath
Write-Utf8 $policyPath ($crlf | ConvertTo-Json -Depth 5)
Confirm-Refused 'CRLF rejected even with matching notes hash' 'LF line endings' { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
Restore-Policy
Write-Utf8 $notesPath $validNotes.Replace('RELEASED_WITH_WARNINGS', 'RELEASED')
$undisclosed = $validPolicy.PSObject.Copy()
$undisclosed.releaseNotesSha256 = Get-FileDigest $notesPath
Write-Utf8 $policyPath ($undisclosed | ConvertTo-Json -Depth 5)
Confirm-Refused 'notes must disclose the warning status' 'do not disclose' { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
Restore-Policy
Write-Utf8 $policyPath '{broken'
Confirm-Refused 'malformed exact-release policy' 'valid JSON' { $null = Get-ReleasePublicationMode $root '1.0.0' 'refs/tags/v1.0.0' }
Confirm-Allowed 'future release does not consume malformed exception' {
  if ((Get-ReleasePublicationMode $root '1.0.1' 'refs/tags/v1.0.1') -cne 'SIGNED_REQUIRED') { throw 'Malformed exception escaped its fixed version' }
}
Restore-Policy
$nominal = [pscustomobject]@{
  productName = 'ImageLore'; productVersion = '1.0.0'; fileVersion = '1.0.0.0'
  signatureStatus = 'NotSigned'; signatureType = 'None'; signerCertificateSha256 = $null; hasTimestamp = $false; sha256 = ('a' * 64)
}
Confirm-Allowed 'unsigned evidence contract' { Assert-KnownLimitationsReleaseEvidence $nominal '1.0.0' }
foreach ($case in @(
  @{ field = 'productName'; value = 'Other'; reason = 'product name or product version' },
  @{ field = 'productVersion'; value = '1.0.0-rc.1'; reason = 'product name or product version' },
  @{ field = 'fileVersion'; value = '1.0.0.1'; reason = 'file version' },
  @{ field = 'signatureStatus'; value = 'HashMismatch'; reason = 'genuinely NotSigned' },
  @{ field = 'signatureStatus'; value = 'Valid'; reason = 'genuinely NotSigned' },
  @{ field = 'signatureStatus'; value = 'UnknownError'; reason = 'genuinely NotSigned' },
  @{ field = 'signatureType'; value = 'Catalog'; reason = 'genuinely NotSigned' },
  @{ field = 'signerCertificateSha256'; value = ('b' * 64); reason = 'genuinely NotSigned' },
  @{ field = 'hasTimestamp'; value = $true; reason = 'genuinely NotSigned' },
  @{ field = 'hasTimestamp'; value = 'False'; reason = 'genuinely NotSigned' },
  @{ field = 'sha256'; value = 'invalid'; reason = 'full SHA-256' }
)) {
  $changed = $nominal.PSObject.Copy()
  $changed.($case.field) = $case.value
  Confirm-Refused ('invalid artifact ' + $case.field) $case.reason { Assert-KnownLimitationsReleaseEvidence $changed '1.0.0' }
}
Confirm-Refused 'evidence cannot approve future versions' 'only accepts version 1.0.0' { Assert-KnownLimitationsReleaseEvidence $nominal '1.0.1' }

# Framework csc emits native VERSIONINFO from these assembly attributes;
# Add-Type alone did not provide this evidence in earlier managed controls.
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) { throw 'Windows Framework x64 csc is required for the VERSIONINFO control' }
Assert-ReleaseNoReparsePoint $compiler
$source = @'
using System.Reflection;
[assembly: AssemblyProduct("ImageLore")]
[assembly: AssemblyInformationalVersion("1.0.0")]
[assembly: AssemblyFileVersion("1.0.0.0")]
[assembly: AssemblyVersion("1.0.0.0")]
public sealed class UnsignedVersionInfoControl {}
'@
$sourcePath = Join-Path $root 'UnsignedVersionInfoControl.cs'
$app = Join-Path $root 'ImageLore-control.exe'
$installer = Join-Path $root 'ImageLore-setup-control.exe'
Write-Utf8 $sourcePath $source
$compileOutput = & $compiler /nologo /target:library ("/out:" + $app) $sourcePath 2>&1
$compileCode = $LASTEXITCODE
Write-Utf8 (Join-Path $root 'compiler.log') ($compileOutput | Out-String)
if ($compileCode -ne 0) { throw 'Could not compile the real unsigned VERSIONINFO PE control' }
Copy-Item -LiteralPath $app -Destination $installer
$originalHash = Get-FileDigest $app
$held = [IO.File]::Open($app, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try { $real = Get-ReleaseFileEvidence $app $held } finally { $held.Dispose() }
Confirm-Allowed 'actual PE has exact native VERSIONINFO and real NotSigned evidence' { Assert-KnownLimitationsReleaseEvidence $real '1.0.0' }
$actualResult = $null
Confirm-Allowed 'actual unsigned VERSIONINFO PE pair under exact release policy' {
  $script:actualResult = Invoke-KnownLimitationsReleaseGate $root $app $installer '1.0.0' 'refs/tags/v1.0.0'
  if ($script:actualResult.releaseStatus -cne 'RELEASED_WITH_WARNINGS' -or $script:actualResult.applicationSha256 -cne $originalHash -or $script:actualResult.installerSha256 -cne $originalHash) { throw 'The real PE pair receipt is incomplete' }
}
Confirm-Refused 'same actual artifact path' 'must be distinct' { $null = Invoke-KnownLimitationsReleaseGate $root $app $app '1.0.0' 'refs/tags/v1.0.0' }
Confirm-Refused 'future tag cannot call unsigned gate' 'exact authorized v1.0.0 tag' { $null = Invoke-KnownLimitationsReleaseGate $root $app $installer '1.0.1' 'refs/tags/v1.0.1' }
Confirm-Refused 'branch cannot call unsigned gate' 'exact authorized v1.0.0 tag' { $null = Invoke-KnownLimitationsReleaseGate $root $app $installer '1.0.0' 'refs/heads/main' }
Confirm-Allowed 'artifact streams reject writes while held for verification' {
  $held = [IO.File]::Open($app, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  $writeRefused = $false
  try {
    try {
      $attempt = [IO.File]::Open($app, [IO.FileMode]::Open, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
      $attempt.Dispose()
    } catch [IO.IOException] { $writeRefused = $true }
  } finally { $held.Dispose() }
  if (-not $writeRefused -or (Get-FileDigest $app) -cne $originalHash) { throw 'Read-held artifact did not preserve its original bytes' }
}
$strictPolicy = [pscustomobject]@{ schemaVersion = 1; productName = 'ImageLore'; approvedSignerCertificateSha256 = @() }
Confirm-Refused 'strict signer policy remains unchanged' 'No approved Windows signing certificate' { $null = Invoke-WindowsReleaseSignatureGate $app $installer '1.0.0' $strictPolicy }
$shell = Join-Path $PSHOME 'pwsh.exe'
if (-not (Test-Path -LiteralPath $shell -PathType Leaf)) { $shell = Join-Path $PSHOME 'powershell.exe' }
$cli = Join-Path $PSScriptRoot 'verify_windows_release_exceptions.ps1'
$positiveOutput = & $shell -NoProfile -NonInteractive -File $cli -RepositoryRoot $root -ApplicationPath $app -InstallerPath $installer -ExpectedVersion '1.0.0' -ReleaseRef 'refs/tags/v1.0.0' 2>&1
$positiveCode = $LASTEXITCODE
Write-Utf8 (Join-Path $root 'cli-positive.log') ($positiveOutput | Out-String)
Confirm-Allowed 'real exact-release CLI success' {
  if ($positiveCode -ne 0 -or ($positiveOutput | Out-String) -notlike '*PASS_KNOWN_LIMITATIONS_RELEASE*') { throw 'Exact-release unsigned CLI did not return its explicit warning receipt' }
}
# Windows PowerShell surfaces native stderr as ErrorRecords. Capture this one
# expected negative invocation, then require its exact exit class and reason.
$previousErrorPreference = $ErrorActionPreference
try {
  $ErrorActionPreference = 'Continue'
  $negativeOutput = & $shell -NoProfile -NonInteractive -File $cli -RepositoryRoot $root -ApplicationPath $app -InstallerPath $installer -ExpectedVersion '1.0.1' -ReleaseRef 'refs/tags/v1.0.1' 2>&1
  $negativeCode = $LASTEXITCODE
} finally { $ErrorActionPreference = $previousErrorPreference }
Write-Utf8 (Join-Path $root 'cli-negative.log') ($negativeOutput | Out-String)
Confirm-Allowed 'real future-release CLI refusal' {
  if ($negativeCode -eq 0 -or ($negativeOutput | Out-String) -notlike '*exact authorized v1.0.0 tag*') { throw 'Future-release CLI did not refuse the exception' }
}
# The negative native exit is consumed only after its status and reason match.
$global:LASTEXITCODE = 0
if ((Get-FileDigest $app) -cne $originalHash -or (Get-FileDigest $installer) -cne $originalHash) { throw 'Regression controls changed their original PE bytes' }
$receipt = [ordered]@{
  state = 'PASS_FIXED_V1_RELEASE_EXCEPTION_CONTRACTS_AND_REAL_VERSIONINFO_PE'
  cases = $caseCount
  actualUnsignedPeSha256 = $originalHash
  actualProductName = $real.productName
  actualProductVersion = $real.productVersion
  actualFileVersion = $real.fileVersion
  actualSignatureStatus = $real.signatureStatus
  actualSignatureType = $real.signatureType
  cliPositiveExitCode = $positiveCode
  cliNegativeExitCode = $negativeCode
  strictSignaturePolicyUnchanged = $true
  scope = 'Policy and disclosure refusals plus compiler-produced native VERSIONINFO in a real unsigned managed PE. The two PE files are synthetic library copies and were never executed; no actual ImageLore/NSIS publication, signature approval, app launch, private key or trust-store write.'
}
Write-Utf8 (Join-Path $root 'test-terminal.json') ($receipt | ConvertTo-Json -Depth 5)
Write-Host ('Windows fixed-v1 release exception regression: PASS ' + ($receipt | ConvertTo-Json -Compress -Depth 5))
