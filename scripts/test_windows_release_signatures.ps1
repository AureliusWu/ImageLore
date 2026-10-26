$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows_release_signatures.ps1')
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
  if (-not $refused) { throw "Release gate allowed $Name" }
  $script:caseCount++
}
$pin = 'a' * 64
$policy = [pscustomobject]@{ schemaVersion = 1; productName = 'ImageLore'; approvedSignerCertificateSha256 = @($pin) }
$nominal = [pscustomobject]@{
  productName = 'ImageLore'; productVersion = '0.26.0'; fileVersion = '0.26.0.0'
  signatureStatus = 'Valid'; signatureType = 'Authenticode'; signerCertificateSha256 = $pin; hasTimestamp = $true
}
# Positive cases exercise policy decisions only, not a real signing service.
Confirm-Allowed 'approved evidence contract' { Assert-ReleaseSignatureEvidence $nominal $policy '0.26.0' }
$rc = $nominal.PSObject.Copy()
$rc.productVersion = '1.0.0-rc.1'
$rc.fileVersion = '1.0.0.0'
Confirm-Allowed 'RC evidence contract' { Assert-ReleaseSignatureEvidence $rc $policy '1.0.0-rc.1' }
foreach ($case in @(
  @{ name = 'wrong product'; field = 'productName'; value = 'Other'; reason = 'product name or product version' },
  @{ name = 'wrong product version'; field = 'productVersion'; value = '0.26.0-other'; reason = 'product name or product version' },
  @{ name = 'wrong file version'; field = 'fileVersion'; value = '0.26.1'; reason = 'file version does not match' },
  @{ name = 'wrong revision'; field = 'fileVersion'; value = '0.26.0.1'; reason = 'file version does not match' },
  @{ name = 'unsigned evidence'; field = 'signatureStatus'; value = 'NotSigned'; reason = 'not a valid embedded' },
  @{ name = 'changed signature'; field = 'signatureStatus'; value = 'HashMismatch'; reason = 'not a valid embedded' },
  @{ name = 'catalog only'; field = 'signatureType'; value = 'Catalog'; reason = 'not a valid embedded' },
  @{ name = 'unknown signer'; field = 'signerCertificateSha256'; value = ('b' * 64); reason = 'not approved' },
  @{ name = 'missing signer'; field = 'signerCertificateSha256'; value = $null; reason = 'not approved' },
  @{ name = 'missing timestamp'; field = 'hasTimestamp'; value = $false; reason = 'no trusted timestamp' }
)) {
  $item = $nominal.PSObject.Copy()
  $item.($case.field) = $case.value
  Confirm-Refused $case.name $case.reason { Assert-ReleaseSignatureEvidence $item $policy '0.26.0' }
}
Confirm-Refused 'invalid requested version' 'Invalid expected release version' { Assert-ReleaseSignatureEvidence $nominal $policy 'invalid' }
$emptyPolicy = [pscustomobject]@{ schemaVersion = 1; productName = 'ImageLore'; approvedSignerCertificateSha256 = @() }
Confirm-Refused 'empty signer policy' 'No approved Windows signing certificate' { Assert-ReleaseSignatureEvidence $nominal $emptyPolicy '0.26.0' }
$badPolicy = [pscustomobject]@{ schemaVersion = 1; productName = 'ImageLore'; approvedSignerCertificateSha256 = @('invalid') }
Confirm-Refused 'malformed signer digest' 'Invalid approved signing certificate' { Assert-ReleaseSignatureEvidence $nominal $badPolicy '0.26.0' }

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Actual unsigned PE regression requires Windows' }
Confirm-Allowed 'installed Microsoft SDK verifier' { $null = Get-ReleaseSdkSignTool }
$base = if ($env:IMAGELORE_SIGNATURE_TEST_ROOT) { $env:IMAGELORE_SIGNATURE_TEST_ROOT } elseif ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
$root = [IO.Path]::GetFullPath((Join-Path $base ('imagelore-signature-test-' + [Guid]::NewGuid().ToString('N'))))
if (Test-Path -LiteralPath $root) { throw 'Signature test directory already exists' }
New-Item -ItemType Directory -Path $root | Out-Null
$app = Join-Path $root 'ImageLore.exe'
$installer = Join-Path $root 'ImageLore-test-setup.exe'
$source = @'
using System.Reflection;
[assembly: AssemblyProduct("ImageLore")]
[assembly: AssemblyInformationalVersion("0.26.0")]
[assembly: AssemblyFileVersion("0.26.0")]
[assembly: AssemblyVersion("0.26.0.0")]
public class UnsignedImageLoreReleaseFixture {}
'@
# Compile a tiny unsigned managed PE. Never execute it or install certificates.
Add-Type -TypeDefinition $source -OutputAssembly $app -OutputType Library
Copy-Item -LiteralPath $app -Destination $installer
$held = [IO.File]::Open($app, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try { $actual = Get-ReleaseFileEvidence $app $held } finally { $held.Dispose() }
if ($actual.signatureStatus -cne 'NotSigned' -or -not [Reflection.AssemblyName]::GetAssemblyName($app)) {
  throw 'The unsigned control is not a valid managed PE with the intended signature state'
}
Confirm-Refused 'actual unsigned PE pair' 'not a valid embedded' { Invoke-WindowsReleaseSignatureGate $app $installer '0.26.0' $policy }
Confirm-Refused 'reused app as installer' 'must be distinct' { Invoke-WindowsReleaseSignatureGate $app $app '0.26.0' $policy }

$policyPath = Join-Path $root 'synthetic-policy.json'
[IO.File]::WriteAllText($policyPath, ($policy | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
$shell = Join-Path $PSHOME 'pwsh.exe'
if (-not (Test-Path -LiteralPath $shell -PathType Leaf)) { $shell = Join-Path $PSHOME 'powershell.exe' }
$cli = Join-Path $PSScriptRoot 'verify_windows_release_signatures.ps1'
$commandOutput = & $shell -NoProfile -NonInteractive -File $cli -ApplicationPath $app -InstallerPath $installer -ExpectedVersion '0.26.0' -PolicyPath $policyPath 2>&1
$commandCode = $LASTEXITCODE
[IO.File]::WriteAllText((Join-Path $root 'cli-negative.log'), ($commandOutput | Out-String), [Text.UTF8Encoding]::new($false))
if ($commandCode -eq 0 -or ($commandOutput | Out-String) -notlike '*not a valid embedded*') { throw 'Unsigned PE CLI did not refuse publication' }
$caseCount++
$projectPolicy = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../src-tauri/windows-signing-policy.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if (@($projectPolicy.approvedSignerCertificateSha256).Count -eq 0) {
  Confirm-Refused 'unconfigured production signer' 'No approved Windows signing certificate' { Invoke-WindowsReleaseSignatureGate $app $installer '0.26.0' $projectPolicy }
}
$receipt = [ordered]@{
  state = 'PASS_RELEASE_SIGNATURE_DECISIONS_AND_REAL_UNSIGNED_PE_REFUSAL'
  cases = $caseCount
  unsignedPeSha256 = $actual.sha256
  cliExitCode = $commandCode
  scope = 'Positive policy contracts and real unsigned PE negatives only; no real approved signing service, app launch, private key or trust-store write.'
}
[IO.File]::WriteAllText((Join-Path $root 'test-terminal.json'), ($receipt | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
Write-Host ('Windows release signature regression: PASS ' + ($receipt | ConvertTo-Json -Compress))
