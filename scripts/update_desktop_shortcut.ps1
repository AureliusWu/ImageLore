param(
  [Parameter(Mandatory=$true)][string]$ExecutablePath,
  [string]$ExpectedVersion = "",
  [string]$DesktopDirectory = [Environment]::GetFolderPath('Desktop'),
  [string]$StatePath = (Join-Path $env:LOCALAPPDATA 'app.imagelore.desktop\desktop-shortcut.json'),
  [switch]$ManagedOnly
)

$ErrorActionPreference = 'Stop'
# WScript.Shell rejected the Chinese-path regression on English Windows. Use
# the Unicode Shell Link interface for both creation and ownership checks.
# https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishelllinkw
if (-not ('ImageLore.UnicodeShortcut' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Text;

namespace ImageLore {
    public sealed class ShortcutInfo {
        public string TargetPath { get; internal set; }
        public string Description { get; internal set; }
        public string WorkingDirectory { get; internal set; }
        public string IconPath { get; internal set; }
        public int IconIndex { get; internal set; }
    }

    public static class UnicodeShortcut {
        [ComImport, Guid("00021401-0000-0000-C000-000000000046")]
        private class ShellLink { }

        [ComImport, Guid("000214F9-0000-0000-C000-000000000046"),
         InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IShellLinkW {
            void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder path, int length, IntPtr findData, uint flags);
            void GetIDList(out IntPtr itemList);
            void SetIDList(IntPtr itemList);
            void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder description, int length);
            void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string description);
            void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder directory, int length);
            void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string directory);
            void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder arguments, int length);
            void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string arguments);
            void GetHotkey(out short hotkey);
            void SetHotkey(short hotkey);
            void GetShowCmd(out int command);
            void SetShowCmd(int command);
            void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder path, int length, out int index);
            void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string path, int index);
            void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string path, uint reserved);
            void Resolve(IntPtr window, uint flags);
            void SetPath([MarshalAs(UnmanagedType.LPWStr)] string path);
        }

        public static ShortcutInfo Read(string path) {
            object native = new ShellLink();
            try {
                ((IPersistFile)native).Load(path, 0);
                IShellLinkW link = (IShellLinkW)native;
                StringBuilder target = new StringBuilder(32768);
                StringBuilder description = new StringBuilder(32768);
                StringBuilder directory = new StringBuilder(32768);
                StringBuilder icon = new StringBuilder(32768);
                int iconIndex;
                link.GetPath(target, target.Capacity, IntPtr.Zero, 4); // SLGP_RAWPATH; never resolve or launch.
                link.GetDescription(description, description.Capacity);
                link.GetWorkingDirectory(directory, directory.Capacity);
                link.GetIconLocation(icon, icon.Capacity, out iconIndex);
                return new ShortcutInfo {
                    TargetPath = target.ToString(), Description = description.ToString(),
                    WorkingDirectory = directory.ToString(), IconPath = icon.ToString(), IconIndex = iconIndex
                };
            } finally {
                Marshal.FinalReleaseComObject(native);
            }
        }

        public static void Write(string path, string target, string directory, string icon, string description) {
            object native = new ShellLink();
            try {
                IShellLinkW link = (IShellLinkW)native;
                link.SetPath(target);
                link.SetWorkingDirectory(directory);
                link.SetIconLocation(icon, 0);
                link.SetDescription(description);
                link.SetShowCmd(1);
                ((IPersistFile)native).Save(path, true);
            } finally {
                Marshal.FinalReleaseComObject(native);
            }
        }
    }
}
'@
}
$owner = 'ImageLore managed desktop shortcut'
$executable = [IO.Path]::GetFullPath($ExecutablePath)
$desktop = [IO.Path]::GetFullPath($DesktopDirectory)
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw "Executable not found: $executable" }
$productVersion = (Get-Item -LiteralPath $executable).VersionInfo.ProductVersion
if ($productVersion -notmatch '(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)') { throw "Executable has no application version: $executable" }
$version = $Matches[1]
if ($ExpectedVersion -and $version -ne $ExpectedVersion) { throw "Executable version $version does not match expected $ExpectedVersion" }
if (-not (Test-Path -LiteralPath $desktop -PathType Container)) { throw "Desktop directory not found: $desktop" }

$hash = [Security.Cryptography.SHA256]::Create()
try { $mutexKey = [BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes([IO.Path]::GetFullPath($StatePath).ToLowerInvariant()))).Replace('-', '') }
finally { $hash.Dispose() }
$mutex = [Threading.Mutex]::new($false, ('Local\ImageLoreShortcut-' + $mutexKey))
$ownsMutex = $false
try {
  try { $ownsMutex = $mutex.WaitOne(30000) }
  catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
  if (-not $ownsMutex) { throw 'Another ImageLore shortcut update is still running' }
function Test-OwnedShortcut([string]$LinkPath, [string]$Target) {
  if (-not $LinkPath -or -not $Target) { return $false }
  $resolved = [IO.Path]::GetFullPath($LinkPath)
  if ([IO.Path]::GetDirectoryName($resolved) -ne $desktop) { return $false }
  if ([IO.Path]::GetFileName($resolved) -notmatch '^ImageLore v[0-9A-Za-z.-]+\.lnk$') { return $false }
  if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) { return $false }
  $link = [ImageLore.UnicodeShortcut]::Read($resolved)
  return $link.Description.StartsWith($owner) -and $link.TargetPath -and
    ([IO.Path]::GetFullPath($link.TargetPath) -eq [IO.Path]::GetFullPath($Target))
}

$previous = $null
function Compare-SemVer([string]$Left, [string]$Right) {
  $a = $Left.Split('-', 2); $b = $Right.Split('-', 2)
  $core = ([Version]$a[0]).CompareTo([Version]$b[0])
  if ($core -ne 0) { return $core }
  if ($a.Length -eq 1 -and $b.Length -eq 1) { return 0 }
  if ($a.Length -eq 1) { return 1 }
  if ($b.Length -eq 1) { return -1 }
  $partsA = $a[1].Split('.'); $partsB = $b[1].Split('.')
  for ($index = 0; $index -lt [Math]::Min($partsA.Length, $partsB.Length); $index++) {
    $x = $partsA[$index]; $y = $partsB[$index]
    if ($x -eq $y) { continue }
    $numberX = $x -match '^\d+$'; $numberY = $y -match '^\d+$'
    if ($numberX -and $numberY) { return ([long]$x).CompareTo([long]$y) }
    if ($numberX) { return -1 }; if ($numberY) { return 1 }
    return [string]::CompareOrdinal($x, $y)
  }
  return $partsA.Length.CompareTo($partsB.Length)
}
if (Test-Path -LiteralPath $StatePath -PathType Leaf) {
  $previous = Get-Content -LiteralPath $StatePath -Raw -Encoding UTF8 | ConvertFrom-Json
  if ($previous.owner -ne $owner) { throw 'Shortcut state is not owned by ImageLore' }
}
if ($ManagedOnly -and (-not $previous -or -not (Test-OwnedShortcut $previous.link $previous.executable))) {
  Write-Output 'No managed desktop shortcut to update'
  return
}
if ($ManagedOnly -and (Compare-SemVer $version $previous.version) -lt 0) {
  Write-Output 'Retained newer managed desktop shortcut'
  return
}

$destination = Join-Path $desktop "ImageLore v$version.lnk"
if ((Test-Path -LiteralPath $destination) -and
  -not (Test-OwnedShortcut $destination $executable) -and
  -not ($previous -and (Test-OwnedShortcut $destination $previous.executable))) {
  throw "An unrelated shortcut already exists: $destination"
}
if ($previous -and $previous.link -eq $destination -and $previous.executable -eq $executable -and
  (Test-OwnedShortcut $destination $executable)) {
  Write-Output $destination
  return
}

$temporary = Join-Path $desktop ('.imagelore-shortcut-' + [Guid]::NewGuid().ToString('N') + '.lnk')
$stateDirectory = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($StatePath))
New-Item -ItemType Directory -Path $stateDirectory -Force | Out-Null
$stateTemporary = Join-Path $stateDirectory ('shortcut-' + [Guid]::NewGuid().ToString('N') + '.tmp')
$oldStateBytes = if (Test-Path -LiteralPath $StatePath -PathType Leaf) { [IO.File]::ReadAllBytes($StatePath) } else { $null }
$oldDestinationBytes = if (Test-Path -LiteralPath $destination -PathType Leaf) { [IO.File]::ReadAllBytes($destination) } else { $null }
$oldLinkBytes = if ($previous -and (Test-OwnedShortcut $previous.link $previous.executable)) { [IO.File]::ReadAllBytes($previous.link) } else { $null }
try {
  [ImageLore.UnicodeShortcut]::Write($temporary, $executable,
    [IO.Path]::GetDirectoryName($executable), $executable, "$owner; version=$version")
  $state = @{ owner=$owner; version=$version; executable=$executable; link=$destination }
  [IO.File]::WriteAllText($stateTemporary, ($state | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
  Move-Item -LiteralPath $temporary -Destination $destination -Force
  Move-Item -LiteralPath $stateTemporary -Destination $StatePath -Force
  if ($previous -and $previous.link -ne $destination -and
    (Test-OwnedShortcut $previous.link $previous.executable)) {
    Remove-Item -LiteralPath $previous.link
  }
  Write-Output $destination
} catch {
  if ($null -ne $oldDestinationBytes) { [IO.File]::WriteAllBytes($destination, $oldDestinationBytes) }
  elseif (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination }
  if ($null -ne $oldStateBytes) { [IO.File]::WriteAllBytes($StatePath, $oldStateBytes) }
  elseif (Test-Path -LiteralPath $StatePath) { Remove-Item -LiteralPath $StatePath }
  if ($null -ne $oldLinkBytes) { [IO.File]::WriteAllBytes($previous.link, $oldLinkBytes) }
  throw
} finally {
  if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
  if (Test-Path -LiteralPath $stateTemporary) { Remove-Item -LiteralPath $stateTemporary }
}
} finally {
  if ($ownsMutex) { $mutex.ReleaseMutex() }
  $mutex.Dispose()
}
