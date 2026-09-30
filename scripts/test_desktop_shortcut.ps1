$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('imagelore-shortcut-test-' + [Guid]::NewGuid().ToString('N'))
$desktop = Join-Path $root 'desktop'
$state = Join-Path $root 'state\desktop-shortcut.json'
$update = Join-Path $PSScriptRoot 'update_desktop_shortcut.ps1'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$powershell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
function Assert-True([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
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
function Update-Fixture([string]$Binary, [switch]$ManagedOnly, [switch]$ExpectFailure) {
  $arguments = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',$update,
    '-ExecutablePath',$Binary,'-DesktopDirectory',$desktop,'-StatePath',$state)
  if ($ManagedOnly) { $arguments += '-ManagedOnly' }
  $previousPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = 'Continue'
    & $powershell @arguments 2>&1 | Out-Null
  } finally {
    $ErrorActionPreference = $previousPreference
  }
  if ($ExpectFailure) { Assert-True ($LASTEXITCODE -ne 0) 'Expected safe refusal' }
  else { Assert-True ($LASTEXITCODE -eq 0) 'Shortcut script failed' }
}
try {
  New-Item -ItemType Directory -Path $desktop -Force | Out-Null
  $first = Build-Fixture '0.1.0'
  $second = Build-Fixture '0.2.0'
  $third = Build-Fixture '0.3.0'
  Update-Fixture $first -ManagedOnly
  Assert-True (-not (Test-Path -LiteralPath $state)) 'ManagedOnly created a shortcut without opt-in'
  Update-Fixture $first
  $firstLink = Join-Path $desktop 'ImageLore v0.1.0.lnk'
  Assert-True (Test-Path -LiteralPath $firstLink) 'Initial versioned shortcut missing'
  $before = (Get-Item -LiteralPath $firstLink).LastWriteTimeUtc
  Update-Fixture $first
  Assert-True ((Get-Item -LiteralPath $firstLink).LastWriteTimeUtc -eq $before) 'Idempotent update changed shortcut'
  $shell = New-Object -ComObject WScript.Shell
  $unrelated = Join-Path $desktop 'User pinned image tool.lnk'
  $userLink = $shell.CreateShortcut($unrelated)
  $userLink.TargetPath = $first
  $userLink.Description = 'user-created'
  $userLink.Save()
  Update-Fixture $second -ManagedOnly
  $secondLink = Join-Path $desktop 'ImageLore v0.2.0.lnk'
  Assert-True (Test-Path -LiteralPath $secondLink) 'New versioned shortcut missing'
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Old owned version shortcut retained'
  Assert-True (Test-Path -LiteralPath $unrelated) 'Unrelated shortcut was deleted'
  $link = $shell.CreateShortcut($secondLink)
  Assert-True ($link.TargetPath -eq $second) 'Shortcut did not follow current executable'
  Assert-True ($link.Description -like '*version=0.2.0') 'Shortcut description has stale version'
  Update-Fixture $first -ManagedOnly
  Assert-True (Test-Path -LiteralPath $secondLink) 'Starting an older binary downgraded the shortcut'
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Older binary recreated an obsolete shortcut'
  $beforeState = Get-Content -LiteralPath $state -Raw
  $lockedLink = [IO.File]::Open($secondLink, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try { Update-Fixture $third -ExpectFailure }
  finally { $lockedLink.Dispose() }
  Assert-True ((Get-Content -LiteralPath $state -Raw) -eq $beforeState) 'Failed old-link cleanup did not roll back state'
  Assert-True (Test-Path -LiteralPath $secondLink) 'Failure lost the previous shortcut'
  Assert-True (-not (Test-Path -LiteralPath (Join-Path $desktop 'ImageLore v0.3.0.lnk'))) 'Failure retained a partially committed shortcut'
  Update-Fixture $first
  $commonArgs = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',('"' + $update + '"'),
    '-DesktopDirectory',('"' + $desktop + '"'),'-StatePath',('"' + $state + '"'),'-ManagedOnly','-ExecutablePath')
  $older = Start-Process -FilePath $powershell -ArgumentList ($commonArgs + ('"' + $first + '"')) -WindowStyle Hidden -PassThru
  $newer = Start-Process -FilePath $powershell -ArgumentList ($commonArgs + ('"' + $second + '"')) -WindowStyle Hidden -PassThru
  $older.WaitForExit(); $newer.WaitForExit()
  Assert-True ($older.ExitCode -eq 0 -and $newer.ExitCode -eq 0) 'Concurrent shortcut update failed'
  Assert-True (Test-Path -LiteralPath $secondLink) 'Concurrent updates lost the newer shortcut'
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Concurrent updates left an obsolete shortcut'
  Remove-Item -LiteralPath $secondLink
  Update-Fixture $first -ManagedOnly
  Assert-True (-not (Test-Path -LiteralPath $firstLink)) 'Deleted shortcut unexpectedly reappeared'
  $collision = $shell.CreateShortcut($firstLink)
  $collision.TargetPath = $second
  $collision.Description = 'user-created'
  $collision.Save()
  Update-Fixture $first -ExpectFailure
  Assert-True ($shell.CreateShortcut($firstLink).Description -eq 'user-created') 'Collision overwrote user shortcut'
  [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) | Out-Null
  Write-Output 'ImageLore desktop shortcut regression: PASS (version change, downgrade guard, target, idempotence, opt-out, ownership, locked-file rollback, concurrent updates)'
} finally {
  $resolved = [IO.Path]::GetFullPath($root)
  $tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
  if (-not $resolved.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -or
    [IO.Path]::GetFileName($resolved) -notlike 'imagelore-shortcut-test-*') { throw 'Unsafe test cleanup path' }
  if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
