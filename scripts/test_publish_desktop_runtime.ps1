param([string]$Repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')))
$ErrorActionPreference = 'Stop'
Import-Module -Name (Join-Path $PSHOME 'Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1') -ErrorAction Stop
$testTempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$testRoot = Join-Path $testTempBase ('imagelore-runtime-publish-test-' + [Guid]::NewGuid().ToString('N'))
$testProject = Join-Path $testRoot 'project'
$testDesktop = Join-Path $testRoot 'isolated-desktop'
$testState = Join-Path $testRoot 'isolated-state\desktop-shortcut.json'
$testScripts = Join-Path $testProject 'scripts'
$testSource = Join-Path $testProject 'synthetic-build'
$testSourceExe = Join-Path $testSource 'imagelore.exe'
$testSourceManifest = Join-Path $testSource 'runtime.json'
$testRuntime = Join-Path $testProject 'desktop-runtime'
$testCurrent = Join-Path $testRuntime 'current'
$testCurrentExe = Join-Path $testCurrent 'ImageLore.exe'
$testPublisher = Join-Path $testScripts 'publish_desktop_runtime.ps1'
$testCsc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$testShell = $null
$testLock = $null
$testCopyOverride = $false
$testOwnsRoot = $false
$testOwnerToken = [Guid]::NewGuid().ToString('N')
$testOwnerMarker = Join-Path $testRoot 'fixture-owner.txt'
$testAssertionCount = 0
$testScenariosPassed = 0
$testExpectedRejections = 0

function Assert-Test([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw "ASSERTION FAILED: $Message" }
  $script:testAssertionCount++
}
function Assert-OwnedPath([string]$Path, [switch]$AllowRoot) {
  $resolved = [IO.Path]::GetFullPath($Path)
  Assert-Test $testOwnsRoot 'this process atomically created the fixture root'
  Assert-Test (($AllowRoot -and $resolved -eq $testRoot) -or
    $resolved.StartsWith($testRoot + '\', [StringComparison]::OrdinalIgnoreCase)) 'fixture path stays within the owned root'
}
function Get-Digest([string]$Path) {
  Assert-OwnedPath $Path
  (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}
function Write-Text([string]$Path, [string]$Text) {
  Assert-OwnedPath $Path
  [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}
function Complete-Scenario([string]$Message) {
  $script:testScenariosPassed++
  Write-Output "PASS $Message"
}
function Commit-Fixture([string]$Message) {
  & git -C $testProject add -- VERSION tracked.txt
  Assert-Test ($LASTEXITCODE -eq 0) 'isolated git fixture add'
  & git -C $testProject -c user.name='ImageLore synthetic test' -c user.email='synthetic@example.invalid' commit --allow-empty -qm $Message
  Assert-Test ($LASTEXITCODE -eq 0) 'isolated git fixture commit'
}
function Get-FixtureHead { (& git -C $testProject rev-parse HEAD).Trim() }
function Compile-TestBinary([string]$Version) {
  $sourceCode = Join-Path $testRoot "sentinel-$Version.cs"
  $source = @"
using System.Reflection;
[assembly: AssemblyTitle("ImageLore synthetic publication fixture")]
[assembly: AssemblyProduct("ImageLore synthetic fixture")]
[assembly: AssemblyVersion("$Version.0")]
[assembly: AssemblyFileVersion("$Version.0")]
[assembly: AssemblyInformationalVersion("$Version")]
internal static class Program { private static void Main() { } }
"@
  Write-Text $sourceCode $source
  & $testCsc /nologo /target:exe "/out:$testSourceExe" $sourceCode
  Assert-Test ($LASTEXITCODE -eq 0) "C# compile $Version"
  Write-Text (Join-Path $testProject 'VERSION') $Version
  Commit-Fixture "Synthetic fixture $Version"
  Write-Text (Join-Path $testSource 'sentinel.dll') "synthetic-dependency-$Version"
  Assert-Test ((Get-Item -LiteralPath $testSourceExe).VersionInfo.ProductVersion -eq $Version) "fixture ProductVersion $Version"
  $metadata = @{ version=$Version; executable='ImageLore.exe'; builtFrom=(Get-FixtureHead); sha256=(Get-Digest $testSourceExe); candidate=$true }
  Write-Text $testSourceManifest ($metadata | ConvertTo-Json)
}
function Invoke-Publisher {
  & $testPublisher -SourceDirectory $testSource -DesktopDirectory $testDesktop -StatePath $testState
}
function Assert-Current([string]$Version, [string]$Commit, [string]$Provenance = 'source-runtime-manifest') {
  Assert-Test ((Get-Item -LiteralPath $testCurrentExe).VersionInfo.ProductVersion -eq $Version) "current version $Version"
  $manifest = Get-Content -LiteralPath (Join-Path $testCurrent 'runtime.json') -Raw | ConvertFrom-Json
  Assert-Test ($manifest.version -ceq $Version -and $manifest.executable -ceq 'ImageLore.exe' -and $manifest.builtFrom -ceq $Commit) "manifest version/executable/actual source commit $Version"
  Assert-Test ($manifest.sha256 -ceq (Get-Digest $testCurrentExe) -and $manifest.builtFromSource -ceq $Provenance) "manifest exact executable hash and provenance $Version"
  Assert-Test ((Get-Content -LiteralPath (Join-Path $testCurrent 'sentinel.dll') -Raw) -eq "synthetic-dependency-$Version") "dependency copied $Version"
  $state = Get-Content -LiteralPath $testState -Raw | ConvertFrom-Json
  $linkPath = Join-Path $testDesktop "ImageLore v$Version.lnk"
  Assert-Test ($state.version -eq $Version -and $state.executable -eq $testCurrentExe -and $state.link -eq $linkPath) "state $Version"
  $link = $testShell.CreateShortcut($linkPath)
  try { Assert-Test ($link.TargetPath -eq $testCurrentExe -and $link.Description.Contains("version=$Version")) "shortcut $Version" }
  finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) | Out-Null }
}
function Snapshot-Protected([string]$Version) {
  @{ binary=(Get-Digest $testCurrentExe); manifest=(Get-Digest (Join-Path $testCurrent 'runtime.json')); state=(Get-Digest $testState); link=(Get-Digest (Join-Path $testDesktop "ImageLore v$Version.lnk")) }
}
function Assert-Protected($Snapshot, [string]$Version) {
  $actual = Snapshot-Protected $Version
  foreach ($key in $Snapshot.Keys) { Assert-Test ($actual[$key] -ceq $Snapshot[$key]) "exact protected $key retained" }
}
function Runtime-Directories { (@(Get-ChildItem -LiteralPath $testRuntime -Directory | Select-Object -ExpandProperty Name | Sort-Object) -join ';') }
function Assert-Rejected([scriptblock]$Action, [string]$Cause) {
  $rejected = $false
  try { & $Action } catch {
    $rejected = $true
    Assert-Test ($_.Exception.Message.Contains($Cause)) "expected rejection cause: $Cause; actual: $($_.Exception.Message)"
  }
  Assert-Test $rejected "publication rejected: $Cause"
  $script:testExpectedRejections++
}

try {
  Assert-Test (Test-Path -LiteralPath $testCsc -PathType Leaf) 'available local C# compiler'
  Assert-Test ([IO.Path]::GetFullPath($testRoot).StartsWith($testTempBase + '\', [StringComparison]::OrdinalIgnoreCase)) 'new fixture root stays within the temp directory'
  Assert-Test (-not (Test-Path -LiteralPath $testRoot)) 'fixture root does not preexist'
  New-Item -ItemType Directory -Path $testRoot -ErrorAction Stop | Out-Null
  $testOwnsRoot = $true
  Write-Text $testOwnerMarker $testOwnerToken
  foreach ($path in @($testProject,$testDesktop,$testState,$testScripts,$testSource,$testSourceExe,$testSourceManifest,$testRuntime,$testCurrent,$testCurrentExe,$testPublisher)) {
    Assert-OwnedPath $path
  }
  foreach ($name in @('GIT_DIR','GIT_WORK_TREE','GIT_INDEX_FILE','GIT_COMMON_DIR','GIT_OBJECT_DIRECTORY','GIT_ALTERNATE_OBJECT_DIRECTORIES')) {
    Assert-Test ([string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($name))) 'inherited Git redirection is absent'
  }
  New-Item -ItemType Directory -Path $testScripts,$testDesktop,$testSource -Force | Out-Null
  foreach ($name in @('publish_desktop_runtime.ps1','update_desktop_shortcut.ps1')) {
    Copy-Item -LiteralPath (Join-Path $Repository "scripts\$name") -Destination (Join-Path $testScripts $name)
  }
  & git -c init.templateDir= -C $testProject init -q
  Assert-Test ($LASTEXITCODE -eq 0) 'isolated git fixture initialized'
  foreach ($setting in @(@('core.hooksPath',(Join-Path $testRoot 'disabled-hooks')),@('commit.gpgSign','false'),@('core.fsmonitor','false'))) {
    & git -C $testProject config --local $setting[0] $setting[1]
    Assert-Test ($LASTEXITCODE -eq 0) 'fixture hooks, signing and fsmonitor are isolated'
  }
  Write-Text (Join-Path $testProject 'tracked.txt') 'tracked baseline'
  $testShell = New-Object -ComObject WScript.Shell

  Compile-TestBinary '0.26.0'
  $firstCommit = Get-FixtureHead
  $firstBinaryDigest = Get-Digest $testSourceExe
  Write-Text (Join-Path $testProject 'docs-only.txt') 'Later documentation does not rebuild the candidate executable.'
  & git -C $testProject add -- docs-only.txt
  Assert-Test ($LASTEXITCODE -eq 0) 'docs-only fixture add'
  Commit-Fixture 'Docs-only HEAD ahead of candidate'
  Assert-Test ((Get-FixtureHead) -cne $firstCommit) 'candidate commit differs from current HEAD'
  Invoke-Publisher
  Assert-Current '0.26.0' $firstCommit
  Assert-Test ((Get-Digest $testCurrentExe) -eq $firstBinaryDigest) 'first promotion contains exact compiled binary'
  Assert-Test (@(Get-ChildItem -LiteralPath $testRuntime -Directory -Filter 'previous-*').Count -eq 0) 'first promotion has no fake previous'
  Complete-Scenario 'initial promotion preserves candidate commit/hash despite docs-only HEAD ahead'

  Compile-TestBinary '0.27.0'
  $currentCommit = Get-FixtureHead
  Invoke-Publisher
  Assert-Current '0.27.0' $currentCommit
  $previous = @(Get-ChildItem -LiteralPath $testRuntime -Directory -Filter 'previous-*')
  Assert-Test ($previous.Count -eq 1) 'upgrade preserves one previous runtime'
  Assert-Test ((Get-Digest (Join-Path $previous[0].FullName 'ImageLore.exe')) -eq $firstBinaryDigest) 'previous contains exact original binary'
  $previousManifest = Get-Content -LiteralPath (Join-Path $previous[0].FullName 'runtime.json') -Raw | ConvertFrom-Json
  Assert-Test ($previousManifest.builtFrom -ceq $firstCommit -and $previousManifest.sha256 -ceq $firstBinaryDigest) 'previous preserves original source metadata'
  Assert-Test (-not (Test-Path -LiteralPath (Join-Path $testDesktop 'ImageLore v0.26.0.lnk'))) 'old managed link removed after successful upgrade'
  Complete-Scenario 'upgrade preserves exact previous executable/source metadata and updates owned shortcut'

  $protected = Snapshot-Protected '0.27.0'
  Compile-TestBinary '0.28.0'
  $validManifestText = Get-Content -LiteralPath $testSourceManifest -Raw
  $directoriesBeforeReject = Runtime-Directories
  $invalidMetadata = @(
    @{ name='truncated commit'; field='builtFrom'; value='1234567'; cause='Source runtime.json' },
    @{ name='commit with trailing newline'; field='builtFrom'; value=((Get-FixtureHead) + "`n"); cause='Source runtime.json' },
    @{ name='truncated hash'; field='sha256'; value='1234'; cause='Source runtime.json' },
    @{ name='wrong hash'; field='sha256'; value=('0' * 64); cause='Source executable SHA256' },
    @{ name='wrong version'; field='version'; value='0.99.0'; cause='Source runtime.json' },
    @{ name='wrong executable'; field='executable'; value='Other.exe'; cause='Source runtime.json' }
  )
  try {
    foreach ($case in $invalidMetadata) {
      $metadata = $validManifestText | ConvertFrom-Json
      $metadata.($case.field) = $case.value
      Write-Text $testSourceManifest ($metadata | ConvertTo-Json)
      Assert-Rejected { Invoke-Publisher } $case.cause
      Assert-Protected $protected '0.27.0'
      Assert-Test ((Runtime-Directories) -ceq $directoriesBeforeReject) "metadata rejection precedes staging: $($case.name)"
    }
    Remove-Item -LiteralPath $testSourceManifest
    Assert-Rejected { Invoke-Publisher } 'An explicit SourceDirectory requires runtime.json'
    Assert-Protected $protected '0.27.0'
    Assert-Test ((Runtime-Directories) -ceq $directoriesBeforeReject) 'missing explicit sidecar rejected before staging'
  } finally { Write-Text $testSourceManifest $validManifestText }
  try {
    Write-Text (Join-Path $testProject 'VERSION') '0.29.0'
    Assert-Rejected { Invoke-Publisher } 'Application binary does not match VERSION'
    Assert-Protected $protected '0.27.0'
    Assert-Test ((Runtime-Directories) -ceq $directoriesBeforeReject) 'actual EXE version mismatch rejected before staging'
  } finally { Write-Text (Join-Path $testProject 'VERSION') '0.28.0' }
  Complete-Scenario 'invalid provenance/hash/version/executable and missing explicit sidecar preserve current before staging'

  # Fixture-only copy corruption exercises the production second-hash check.
  # The publisher has no test hook; this function shadows Copy-Item in this test process only.
  function Copy-Item {
    param([string]$LiteralPath, [string]$Destination)
    Microsoft.PowerShell.Management\Copy-Item -LiteralPath $LiteralPath -Destination $Destination
    if ($LiteralPath -ieq $testSourceExe) { [IO.File]::AppendAllText($Destination, 'fixture-stage-corruption') }
  }
  $testCopyOverride = $true
  try {
    Assert-Rejected { Invoke-Publisher } 'Staged executable SHA256 changed during publication'
    Assert-Protected $protected '0.27.0'
    Assert-Test ((Get-Digest $testSourceExe) -ceq (($validManifestText | ConvertFrom-Json).sha256)) 'copy corruption leaves source unchanged'
  } finally { Remove-Item -LiteralPath Function:\Copy-Item; $testCopyOverride = $false }
  Complete-Scenario 'staged-copy hash mismatch refuses promotion and preserves current/source'

  $collisionPath = Join-Path $testDesktop 'ImageLore v0.28.0.lnk'
  $unowned = $testShell.CreateShortcut($collisionPath)
  try {
    $unowned.TargetPath = $testSourceExe
    $unowned.Description = 'Unrelated user shortcut sentinel'
    $unowned.Save()
  } finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($unowned) | Out-Null }
  $collisionDigest = Get-Digest $collisionPath
  Assert-Rejected { Invoke-Publisher } 'unrelated shortcut'
  Assert-Current '0.27.0' $currentCommit
  Assert-Protected $protected '0.27.0'
  Assert-Test ((Get-Digest $collisionPath) -ceq $collisionDigest) 'collision preserves unowned shortcut exactly'
  $failed = @(Get-ChildItem -LiteralPath $testRuntime -Directory -Filter 'failed-*')
  Assert-Test ($failed.Count -eq 1 -and (Get-Item -LiteralPath (Join-Path $failed[0].FullName 'ImageLore.exe')).VersionInfo.ProductVersion -eq '0.28.0') 'rejected runtime retained for inspection'
  Complete-Scenario 'ownership collision rolls back exact current/source metadata/state/links and retains failed candidate'

  Remove-Item -LiteralPath $collisionPath
  $testLock = [IO.File]::Open($testCurrentExe, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
  $lockFailed = $false
  try { Invoke-Publisher } catch { $lockFailed = $true }
  $testLock.Dispose(); $testLock = $null
  Assert-Test $lockFailed 'exclusive open current binary prevents publication'
  $testExpectedRejections++
  Assert-Current '0.27.0' $currentCommit
  Assert-Protected $protected '0.27.0'
  Complete-Scenario 'exclusive current executable lock preserves current metadata/state/shortcut'

  $defaultSource = Join-Path $testProject 'src-tauri\target\release'
  New-Item -ItemType Directory -Path $defaultSource -Force | Out-Null
  Copy-Item -LiteralPath $testSourceExe -Destination (Join-Path $defaultSource 'imagelore.exe')
  Copy-Item -LiteralPath (Join-Path $testSource 'sentinel.dll') -Destination $defaultSource
  $directoriesBeforeReject = Runtime-Directories
  Assert-Rejected { & $testPublisher -SourceDirectory $defaultSource -DesktopDirectory $testDesktop -StatePath $testState } 'An explicit SourceDirectory requires runtime.json'
  Assert-Protected $protected '0.27.0'
  Assert-Test ((Runtime-Directories) -ceq $directoriesBeforeReject) 'explicit default path does not bypass sidecar requirement'
  Write-Text (Join-Path $testProject 'tracked.txt') 'uncommitted tracked change'
  $directoriesBeforeReject = Runtime-Directories
  Assert-Rejected { & $testPublisher -DesktopDirectory $testDesktop -StatePath $testState } 'Default build publication requires a clean Git tracked worktree'
  Assert-Protected $protected '0.27.0'
  Assert-Test ((Runtime-Directories) -ceq $directoriesBeforeReject) 'dirty legacy default refused before staging'
  Write-Text (Join-Path $testProject 'tracked.txt') 'tracked baseline'
  $legacyCommit = Get-FixtureHead
  & $testPublisher -DesktopDirectory $testDesktop -StatePath $testState
  Assert-Current '0.28.0' $legacyCommit 'legacy-clean-head'
  Complete-Scenario 'legacy default build permits tracked-clean HEAD and rejects tracked changes'
  Write-Output 'PASS all isolated runtime publication regressions; no fixture executable ran or actual Desktop/LOCALAPPDATA path was used'
} finally {
  if ($testCopyOverride) { Remove-Item -LiteralPath Function:\Copy-Item }
  if ($null -ne $testLock) { $testLock.Dispose() }
  if ($null -ne $testShell) { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($testShell) | Out-Null }
  if ($testOwnsRoot) {
    $resolved = [IO.Path]::GetFullPath($testRoot)
    Assert-OwnedPath $resolved -AllowRoot
    if (-not $resolved.StartsWith($testTempBase + '\', [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($resolved) -notmatch '^imagelore-runtime-publish-test-[0-9a-f]{32}$' -or
        -not (Test-Path -LiteralPath $testOwnerMarker -PathType Leaf) -or
        (Get-Content -LiteralPath $testOwnerMarker -Raw) -cne $testOwnerToken) {
      throw "Refusing cleanup of an unowned temp root: $resolved"
    }
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push($resolved)
    while ($pending.Count -gt 0) {
      $directory = $pending.Pop()
      Assert-OwnedPath $directory -AllowRoot
      foreach ($entry in @(Get-Item -LiteralPath $directory -Force) + @(Get-ChildItem -LiteralPath $directory -Force)) {
        Assert-OwnedPath $entry.FullName -AllowRoot
        if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
          throw 'Refusing recursive cleanup through a fixture reparse point'
        }
        if ($entry.PSIsContainer -and $entry.FullName -ne $directory) { $pending.Push($entry.FullName) }
      }
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
  }
}
Write-Output ('RESULT ' + (@{ scenariosPassed=$testScenariosPassed; assertionsPassed=$testAssertionCount; expectedRejectionsObserved=$testExpectedRejections; fixtureRoot=$testRoot; fixtureRemoved=(-not (Test-Path -LiteralPath $testRoot)); fixtureExecutableRan=$false; actualDesktopOrStateUsed=$false } | ConvertTo-Json -Compress))
