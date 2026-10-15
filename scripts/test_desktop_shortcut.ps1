$ErrorActionPreference = 'Stop'
# Exercise UTF-8 state paths even on an ASCII-only Windows user profile.
$root = Join-Path ([IO.Path]::GetTempPath()) ('imagelore-shortcut-test-' + [char]0x4e2d + [char]0x6587 + '-' + [Guid]::NewGuid().ToString('N'))
$desktop = Join-Path $root 'desktop'
$state = Join-Path $root 'state\desktop-shortcut.json'
$update = Join-Path $PSScriptRoot 'update_desktop_shortcut.ps1'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$powershell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
$script:shortcutCall = 0
function Assert-True([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Get-DiagnosticTail($Output) {
  $details = ($Output | Select-Object -Last 12 | Out-String).Trim()
  if ($details.Length -gt 4096) { $details = $details.Substring($details.Length - 4096) }
  return $details
}
function Build-Fixture([string]$Version) {
  $directory = Join-Path $root $Version
  New-Item -ItemType Directory -Path $directory -Force | Out-Null
  $code = @"
using System.Reflection;
[assembly: AssemblyProduct("ImageLore")]
[assembly: AssemblyFileVersion("$Version.0")]
[assembly: AssemblyInformationalVersion("$Version")]
class Entry { static void Main() {} }
"@
  $source = Join-Path $directory 'fixture.cs'
  $binary = Join-Path $directory 'ImageLore.exe'
  [IO.File]::WriteAllText($source, $code)
  & $compiler /nologo /target:winexe "/out:$binary" $source
  Assert-True ($LASTEXITCODE -eq 0) 'Fixture compilation failed'
  return $binary
}
function Update-Fixture([string]$Binary, [string]$Scenario, [switch]$ManagedOnly, [switch]$ExpectFailure) {
  $script:shortcutCall += 1
  $callsite = $MyInvocation.ScriptLineNumber
  $arguments = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',$update,
    '-ExecutablePath',$Binary,'-DesktopDirectory',$desktop,'-StatePath',$state)
  if ($ManagedOnly) { $arguments += '-ManagedOnly' }
  $previousPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = 'Continue'
    $output = @(& $powershell @arguments 2>&1)
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousPreference
  }
  $context = "call=$script:shortcutCall; line=$callsite; scenario=$Scenario; version=" +
    [IO.Path]::GetFileName([IO.Path]::GetDirectoryName($Binary)) +
    '; managedOnly=' + $ManagedOnly.IsPresent + '; expectFailure=' + $ExpectFailure.IsPresent
  $details = Get-DiagnosticTail $output
  if ($ExpectFailure) { Assert-True ($exitCode -ne 0) ("Expected safe refusal; $context; exit=$exitCode`n$details") }
  else { Assert-True ($exitCode -eq 0) ("Shortcut script failed; $context; exit=$exitCode`n$details") }
}
try {
  New-Item -ItemType Directory -Path $desktop -Force | Out-Null
  $first = Build-Fixture '0.1.0'
  $second = Build-Fixture '0.2.0'
  $third = Build-Fixture '0.3.0'
  # Load the same self-contained Unicode API in this host, twice to exercise its
  # type guard. Explicit owned paths and ManagedOnly prevent any desktop opt-in.
  1..2 | ForEach-Object {
    & { . $update -ExecutablePath $first -DesktopDirectory $desktop -StatePath $state -ManagedOnly } | Out-Null
    Assert-True (-not (Test-Path -LiteralPath $state)) 'Loading shortcut types created managed state'
  }
  Update-Fixture $first -Scenario 'managed-only-before-opt-in' -ManagedOnly
  Assert-True (-not (Test-Path -LiteralPath $state)) 'ManagedOnly created a shortcut without opt-in'
  Update-Fixture $first -Scenario 'initial-opt-in'
  $firstLink = Join-Path $desktop 'ImageLore v0.1.0.lnk'
  Assert-True (Test-Path -LiteralPath $firstLink) 'Initial versioned shortcut missing'
  $initialLink = [ImageLore.UnicodeShortcut]::Read($firstLink)
  Assert-True ($initialLink.TargetPath -eq $first) 'Initial Unicode target did not round-trip'
  Assert-True ($initialLink.WorkingDirectory -eq [IO.Path]::GetDirectoryName($first)) 'Unicode working directory did not round-trip'
  Assert-True ($initialLink.IconPath -eq $first -and $initialLink.IconIndex -eq 0) 'Unicode icon did not round-trip'
  $before = (Get-Item -LiteralPath $firstLink).LastWriteTimeUtc
  Update-Fixture $first -Scenario 'idempotent-same-version'
  Assert-True ((Get-Item -LiteralPath $firstLink).LastWriteTimeUtc -eq $before) 'Idempotent update changed shortcut'
  $unrelated = Join-Path $desktop 'User pinned image tool.lnk'
  [ImageLore.UnicodeShortcut]::Write($unrelated, $first, [IO.Path]::GetDirectoryName($first), $first, 'user-created')
  Update-Fixture $second -Scenario 'upgrade-managed-version' -ManagedOnly
  $secondLink = Join-Path $desktop 'ImageLore v0.2.0.lnk'
  Assert-True (Test-Path -LiteralPath $secondLink) 'New versioned shortcut missing'
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Old owned version shortcut retained'
  Assert-True (Test-Path -LiteralPath $unrelated) 'Unrelated shortcut was deleted'
  $link = [ImageLore.UnicodeShortcut]::Read($secondLink)
  Assert-True ($link.TargetPath -eq $second) 'Shortcut did not follow current executable'
  Assert-True ($link.Description -like '*version=0.2.0') 'Shortcut description has stale version'
  Update-Fixture $first -Scenario 'managed-downgrade-refusal' -ManagedOnly
  Assert-True (Test-Path -LiteralPath $secondLink) 'Starting an older binary downgraded the shortcut'
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Older binary recreated an obsolete shortcut'
  $beforeState = Get-Content -LiteralPath $state -Raw -Encoding UTF8
  $lockedLink = [IO.File]::Open($secondLink, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try { Update-Fixture $third -Scenario 'locked-old-link-rollback' -ExpectFailure }
  finally { $lockedLink.Dispose() }
  Assert-True ((Get-Content -LiteralPath $state -Raw -Encoding UTF8) -eq $beforeState) 'Failed old-link cleanup did not roll back state'
  Assert-True (Test-Path -LiteralPath $secondLink) 'Failure lost the previous shortcut'
  Assert-True (-not (Test-Path -LiteralPath (Join-Path $desktop 'ImageLore v0.3.0.lnk'))) 'Failure retained a partially committed shortcut'
  Update-Fixture $first -Scenario 'explicit-older-opt-in'
  $commonArgs = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',('"' + $update + '"'),
    '-DesktopDirectory',('"' + $desktop + '"'),'-StatePath',('"' + $state + '"'),'-ManagedOnly','-ExecutablePath')
  $olderOut = Join-Path $root 'concurrent-older.stdout.log'
  $olderErr = Join-Path $root 'concurrent-older.stderr.log'
  $newerOut = Join-Path $root 'concurrent-newer.stdout.log'
  $newerErr = Join-Path $root 'concurrent-newer.stderr.log'
  $older = Start-Process -FilePath $powershell -ArgumentList ($commonArgs + ('"' + $first + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput $olderOut -RedirectStandardError $olderErr
  $olderHandle = $older.Handle
  $newer = Start-Process -FilePath $powershell -ArgumentList ($commonArgs + ('"' + $second + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput $newerOut -RedirectStandardError $newerErr
  $newerHandle = $newer.Handle
  Assert-True ($olderHandle -ne [IntPtr]::Zero -and $newerHandle -ne [IntPtr]::Zero) 'Concurrent child handles were not retained'
  $older.WaitForExit(); $newer.WaitForExit()
  $concurrentDetails = Get-DiagnosticTail (Get-Content -LiteralPath $olderOut,$olderErr,$newerOut,$newerErr)
  Assert-True ($older.ExitCode -eq 0 -and $newer.ExitCode -eq 0) ("Concurrent shortcut update failed; older=$($older.ExitCode); newer=$($newer.ExitCode)`n$concurrentDetails")
  Assert-True (Test-Path -LiteralPath $secondLink) 'Concurrent updates lost the newer shortcut'
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Concurrent updates left an obsolete shortcut'
  Remove-Item -LiteralPath $secondLink
  Update-Fixture $first -Scenario 'deleted-link-opt-out' -ManagedOnly
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Deleted shortcut unexpectedly reappeared'
  [ImageLore.UnicodeShortcut]::Write($firstLink, $second, [IO.Path]::GetDirectoryName($second), $second, 'user-created')
  Update-Fixture $first -Scenario 'unrelated-name-collision' -ExpectFailure
  Assert-True ([ImageLore.UnicodeShortcut]::Read($firstLink).Description -eq 'user-created') 'Collision overwrote user shortcut'
  Write-Output 'ImageLore desktop shortcut regression: PASS (version change, downgrade guard, target, idempotence, opt-out, ownership, locked-file rollback, concurrent updates)'
} finally {
  $resolved = [IO.Path]::GetFullPath($root)
  $tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
  if (-not $resolved.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -or
    [IO.Path]::GetFileName($resolved) -notlike 'imagelore-shortcut-test-*') { throw 'Unsafe test cleanup path' }
  if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
