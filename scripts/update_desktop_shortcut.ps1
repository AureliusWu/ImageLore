param(
  [Parameter(Mandatory=$true)][string]$ExecutablePath,
  [string]$ExpectedVersion = "",
  [string]$DesktopDirectory = [Environment]::GetFolderPath('Desktop'),
  [string]$StatePath = (Join-Path $env:LOCALAPPDATA 'app.imagelore.desktop\desktop-shortcut.json'),
  [switch]$ManagedOnly
)

$ErrorActionPreference = 'Stop'
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
$shell = $null
try {
  try { $ownsMutex = $mutex.WaitOne(30000) }
  catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
  if (-not $ownsMutex) { throw 'Another ImageLore shortcut update is still running' }
  $shell = New-Object -ComObject WScript.Shell
function Test-OwnedShortcut([string]$LinkPath, [string]$Target) {
  if (-not $LinkPath -or -not $Target) { return $false }
  $resolved = [IO.Path]::GetFullPath($LinkPath)
  if ([IO.Path]::GetDirectoryName($resolved) -ne $desktop) { return $false }
  if ([IO.Path]::GetFileName($resolved) -notmatch '^ImageLore v[0-9A-Za-z.-]+\.lnk$') { return $false }
  if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) { return $false }
  $link = $shell.CreateShortcut($resolved)
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
  $previous = Get-Content -LiteralPath $StatePath -Raw | ConvertFrom-Json
  if ($previous.owner -ne $owner) { throw 'Shortcut state is not owned by ImageLore' }
}
if ($ManagedOnly -and (-not $previous -or -not (Test-OwnedShortcut $previous.link $previous.executable))) {
  Write-Output 'No managed desktop shortcut to update'
  exit 0
}
if ($ManagedOnly -and (Compare-SemVer $version $previous.version) -lt 0) {
  Write-Output 'Retained newer managed desktop shortcut'
  exit 0
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
  exit 0
}

$temporary = Join-Path $desktop ('.imagelore-shortcut-' + [Guid]::NewGuid().ToString('N') + '.lnk')
$stateDirectory = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($StatePath))
New-Item -ItemType Directory -Path $stateDirectory -Force | Out-Null
$stateTemporary = Join-Path $stateDirectory ('shortcut-' + [Guid]::NewGuid().ToString('N') + '.tmp')
$oldStateBytes = if (Test-Path -LiteralPath $StatePath -PathType Leaf) { [IO.File]::ReadAllBytes($StatePath) } else { $null }
$oldDestinationBytes = if (Test-Path -LiteralPath $destination -PathType Leaf) { [IO.File]::ReadAllBytes($destination) } else { $null }
$oldLinkBytes = if ($previous -and (Test-OwnedShortcut $previous.link $previous.executable)) { [IO.File]::ReadAllBytes($previous.link) } else { $null }
try {
  $shortcut = $shell.CreateShortcut($temporary)
  $shortcut.TargetPath = $executable
  $shortcut.WorkingDirectory = [IO.Path]::GetDirectoryName($executable)
  $shortcut.IconLocation = "$executable,0"
  $shortcut.Description = "$owner; version=$version"
  $shortcut.Save()
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
  if ($null -ne $shell) { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) | Out-Null }
  if ($ownsMutex) { $mutex.ReleaseMutex() }
  $mutex.Dispose()
}
