param([switch]$LegacyDacl)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$stage = 'PREPARE'
$failureType = 'none'
$success = $false
$ownsRoot = $false
$child = $null
$stderrTask = $null
$childStderr = ''
$childExited = $false
$childExitCode = 'not-observed'
$readyObserved = $false
$restoreRequested = $false
$restoreObserved = $false
$deniedObserved = $false
$restoredWriteObserved = $false
$legacyPrepared = $false
$parentSddlEqual = $false
$markerText = "ImageLore raw ACL test v1`n"
$tempBase = ''
$rootName = 'imagelore-raw-acl-preservation-' + [Guid]::NewGuid().ToString('N')
$root = ''
$backups = ''
$marker = ''
$control = ''

function Assert-Condition([bool]$Condition) {
  if (-not $Condition) { throw [InvalidOperationException]::new() }
}

function Assert-OrdinaryPath([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force
  Assert-Condition (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0)
}

function Write-Control {
  $stream = [IO.File]::Open($control, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  try { $stream.WriteByte(1); $stream.Flush() } finally { $stream.Dispose() }
  [IO.File]::Delete($control)
}

function Read-ProtocolLine([IO.StreamReader]$Reader) {
  $pending = $Reader.ReadLineAsync()
  if (-not $pending.Wait(30000)) { throw [TimeoutException]::new() }
  return $pending.GetAwaiter().GetResult()
}

function Assert-OwnedCleanup {
  $resolved = [IO.Path]::GetFullPath($root)
  Assert-Condition ($resolved -ceq $root)
  Assert-Condition ([IO.Path]::GetDirectoryName($resolved).Equals($tempBase, [StringComparison]::OrdinalIgnoreCase))
  Assert-Condition ([IO.Path]::GetFileName($resolved) -ceq $rootName)
  Assert-Condition ($rootName -cmatch '^imagelore-raw-acl-preservation-[0-9a-f]{32}$')
  Assert-Condition $ownsRoot
  Assert-OrdinaryPath $resolved
  Assert-OrdinaryPath $backups
  Assert-OrdinaryPath $marker
  Assert-Condition ([IO.File]::ReadAllText($marker) -ceq $markerText)
  $pending = [Collections.Generic.Stack[string]]::new()
  $pending.Push($resolved)
  while ($pending.Count -gt 0) {
    $directory = $pending.Pop()
    foreach ($entry in Get-ChildItem -LiteralPath $directory -Force) {
      $absolute = [IO.Path]::GetFullPath($entry.FullName)
      Assert-Condition ($absolute.StartsWith($resolved + '\', [StringComparison]::OrdinalIgnoreCase))
      Assert-OrdinaryPath $absolute
      if ($entry.PSIsContainer) { $pending.Push($absolute) }
    }
  }
}

function Get-SafeChildDiagnostics([string]$Output) {
  $safe = [Collections.Generic.List[string]]::new()
  $dropped = 0
  $numericKeys = @('originalFlags','restoredFlags','flagsXor','originalAceCount','restoredAceCount','originalAclRevision','restoredAclRevision')
  $booleanKeys = @('daclBytesEqual','aceOrderEqual','aceMultisetEqual','originalCanonical','restoredCanonical','originalDaclNull','restoredDaclNull')
  foreach ($line in ($Output -split '\r?\n')) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $parts = $line.Trim() -split ' '
    $valid = $parts[0] -cin @('OWNED_ACL_FAILURE','OWNED_ACL_DIAGNOSTIC') -and $parts.Count -gt 1
    foreach ($part in $parts | Select-Object -Skip 1) {
      if ($part -cnotmatch '^([A-Za-z][A-Za-z-]*)=([A-Za-z0-9_-]+)$') { $valid = $false; break }
      $key = $Matches[1]; $value = $Matches[2]
      $allowed = ($key -cin $numericKeys -and $value -cmatch '^-?\d+$') -or
        ($key -cin $booleanKeys -and $value -cmatch '^(?:[Tt]rue|[Ff]alse)$') -or
        ($key -cin @('stage','cleanup-stage','diagnostic-stage') -and $value -cmatch '^[A-Z][A-Z_]*$') -or
        ($key -ceq 'exception' -and $value -cmatch '^[A-Za-z]+Exception$')
      if (-not $allowed) { $valid = $false; break }
    }
    if ($valid) { $safe.Add(($parts -join ' ')) } else { $dropped++ }
  }
  return @{ text=($safe -join "`n"); dropped=$dropped }
}

try {
  if ($LegacyDacl -and -not ('ImageLore.AclFixtureBaseline' -as [type])) {
    $stage = 'COMPILE_BASELINE_HELPER'
    Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
namespace ImageLore {
    public static class AclFixtureBaseline {
        [DllImport("advapi32.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetFileSecurityW(string path, uint securityInformation, byte[] descriptor);
    }
}
'@
  }
  $tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
  $root = Join-Path $tempBase $rootName
  $backups = Join-Path $root 'backups'
  $marker = Join-Path $root '.imagelore-raw-acl-test'
  $control = Join-Path $backups 'permission-control.raw'
  $stage = 'VALIDATE_TEMP_ANCESTORS'
  $ancestor = $tempBase
  while (-not [string]::IsNullOrEmpty($ancestor)) {
    Assert-OrdinaryPath $ancestor
    $ancestor = [IO.Path]::GetDirectoryName($ancestor)
  }
  $stage = 'CREATE_OWNED_ROOT'
  Assert-Condition (-not (Test-Path -LiteralPath $root))
  [void][IO.Directory]::CreateDirectory($root)
  Assert-OrdinaryPath $root
  $markerStream = [IO.File]::Open($marker, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  try {
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($markerText)
    $markerStream.Write($bytes, 0, $bytes.Length)
  } finally { $markerStream.Dispose() }
  $ownsRoot = $true
  [void][IO.Directory]::CreateDirectory($backups)
  Assert-OrdinaryPath $backups
  $stage = 'VERIFY_INITIAL_WRITE'
  Write-Control

  $access = [Security.AccessControl.AccessControlSections]::Access
  if ($LegacyDacl) {
    $stage = 'PREPARE_LEGACY_DACL'
    $original = Get-Acl -LiteralPath $backups
    $descriptor = [Security.AccessControl.RawSecurityDescriptor]::new($original.GetSecurityDescriptorBinaryForm(), 0)
    Assert-Condition ($null -ne $descriptor.DiscretionaryAcl)
    $daclBefore = [byte[]]::new($descriptor.DiscretionaryAcl.BinaryLength)
    $descriptor.DiscretionaryAcl.GetBinaryForm($daclBefore, 0)
    $expectedFlags = ([int]$descriptor.ControlFlags -band 0x150C) -band (-bnot 0x400)
    $descriptor.SetFlags([Security.AccessControl.ControlFlags]([int]$descriptor.ControlFlags -band (-bnot 0x400)))
    $encoded = [byte[]]::new($descriptor.BinaryLength)
    $descriptor.GetBinaryForm($encoded, 0)
    # This native call prepares only the owned test baseline. Never select an
    # external path or request Owner, Group, SACL or protection changes.
    Assert-OwnedCleanup
    Assert-Condition ([IO.Path]::GetFullPath($backups) -ceq (Join-Path $root 'backups'))
    if (-not [ImageLore.AclFixtureBaseline]::SetFileSecurityW($backups, 4, $encoded)) {
      throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
    $stage = 'VERIFY_LEGACY_DACL'
    $prepared = Get-Acl -LiteralPath $backups
    $readback = [Security.AccessControl.RawSecurityDescriptor]::new($prepared.GetSecurityDescriptorBinaryForm(), 0)
    Assert-Condition ($null -ne $readback.DiscretionaryAcl)
    $daclAfter = [byte[]]::new($readback.DiscretionaryAcl.BinaryLength)
    $readback.DiscretionaryAcl.GetBinaryForm($daclAfter, 0)
    Assert-Condition ([Convert]::ToBase64String($daclBefore) -ceq [Convert]::ToBase64String($daclAfter))
    Assert-Condition (([int]$readback.ControlFlags -band 0x150C) -eq $expectedFlags)
    $legacyPrepared = ([int]$readback.ControlFlags -band 0x400) -eq 0
    Assert-Condition $legacyPrepared
  }
  $stage = 'CAPTURE_PARENT_ORACLE'
  $parentAccess = (Get-Acl -LiteralPath $backups).GetSecurityDescriptorSddlForm($access)

  $stage = 'START_FIXTURE'
  $start = [Diagnostics.ProcessStartInfo]::new()
  $start.FileName = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
  $start.UseShellExecute = $false
  $start.CreateNoWindow = $true
  $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
  $start.RedirectStandardInput = $true
  $start.RedirectStandardOutput = $true
  $start.RedirectStandardError = $true
  $fixture = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\src-tauri\tests\fixtures\backup\hold-createfiles-deny.ps1'))
  foreach ($argument in @('-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',$fixture,'-OwnedRoot',$root)) {
    $start.ArgumentList.Add($argument)
  }
  $child = [Diagnostics.Process]::new()
  $child.StartInfo = $start
  Assert-Condition ($child.Start())
  $childHandle = $child.Handle
  Assert-Condition ($childHandle -ne [IntPtr]::Zero)
  $stderrTask = $child.StandardError.ReadToEndAsync()
  $stage = 'WAIT_READY'
  $readyObserved = (Read-ProtocolLine $child.StandardOutput) -ceq 'ACL_DENY_READY'
  Assert-Condition $readyObserved

  $stage = 'VERIFY_PERMISSION_DENIED'
  try { Write-Control } catch {
    $cause = $_.Exception
    while ($null -ne $cause.InnerException) { $cause = $cause.InnerException }
    if ($cause -isnot [UnauthorizedAccessException]) { throw }
    $deniedObserved = $true
  }
  Assert-Condition $deniedObserved

  $stage = 'REQUEST_RESTORE'
  $child.StandardInput.WriteLine('RESTORE')
  $child.StandardInput.Flush()
  $restoreRequested = $true
  $child.StandardInput.Close()
  $stage = 'WAIT_RESTORED'
  $restoreObserved = (Read-ProtocolLine $child.StandardOutput) -ceq 'ACL_ACCESS_RESTORED'
  Assert-Condition $restoreObserved
  $stage = 'WAIT_CHILD_EXIT'
  $childExited = $child.WaitForExit(30000)
  Assert-Condition $childExited
  $childExitCode = [string]$child.ExitCode
  Assert-Condition ($child.ExitCode -eq 0)
  $stage = 'VERIFY_PARENT_ORACLE'
  $parentSddlEqual = (Get-Acl -LiteralPath $backups).GetSecurityDescriptorSddlForm($access) -ceq $parentAccess
  Assert-Condition $parentSddlEqual
  $stage = 'VERIFY_RESTORED_WRITE'
  Write-Control
  $restoredWriteObserved = $true
  $success = $true
} catch {
  $failureType = $_.Exception.GetType().Name
} finally {
  if ($null -ne $child) {
    # Closing stdin also requests the fixture's original-ACL restoration on EOF.
    try { $child.StandardInput.Close() } catch { }
    try {
      if (-not $childExited) { $childExited = $child.WaitForExit(30000) }
      if ($childExited) { $childExitCode = [string]$child.ExitCode }
    } catch { $success = $false; $failureType = $_.Exception.GetType().Name }
    if ($null -ne $stderrTask) {
      try {
        if (-not $stderrTask.Wait(5000)) { throw [TimeoutException]::new() }
        $childStderr = $stderrTask.GetAwaiter().GetResult()
      } catch { $success = $false; $failureType = $_.Exception.GetType().Name }
    }
    try { $child.Dispose() } catch { $success = $false; $failureType = $_.Exception.GetType().Name }
  }
}

if ($success) {
  try {
    $stage = 'CLEANUP_OWNED_ROOT'
    Assert-OwnedCleanup
    Remove-Item -LiteralPath $root -Recurse -Force
  } catch { $success = $false; $failureType = $_.Exception.GetType().Name }
}

if (-not $success) {
  $details = Get-SafeChildDiagnostics $childStderr
  $message = 'ImageLore ACL fixture: FAIL stage={0} exception={1} ready={2} denied={3} restoreRequested={4} restoreObserved={5} childExited={6} childExit={7} restoredWrite={8} droppedChildLines={9} legacy={10} legacyPrepared={11} parentSddlEqual={12}' -f $stage,$failureType,$readyObserved,$deniedObserved,$restoreRequested,$restoreObserved,$childExited,$childExitCode,$restoredWriteObserved,$details.dropped,$LegacyDacl.IsPresent,$legacyPrepared,$parentSddlEqual
  if ($details.text) { $message += "`n" + $details.text }
  # All accepted text is ASCII; reserve two bytes for WriteLine's CRLF.
  if ($message.Length -gt 4094) { $message = $message.Substring(0,4094) }
  [Console]::Error.WriteLine($message)
  exit 1
}
Write-Output ('ImageLore ACL fixture: PASS legacy={0} (initial write, real denial, strict restore ACK, exit 0, independent parent ACL oracle, restored write, owned cleanup)' -f $LegacyDacl.IsPresent)
exit 0
