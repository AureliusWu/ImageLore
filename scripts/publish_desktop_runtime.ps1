param(
  [string]$SourceDirectory = '',
  [string]$DesktopDirectory = [Environment]::GetFolderPath('Desktop'),
  [string]$StatePath = (Join-Path $env:LOCALAPPDATA 'app.imagelore.desktop\desktop-shortcut.json')
)
$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$hash = [Security.Cryptography.SHA256]::Create()
try { $mutexKey = [BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($project.ToLowerInvariant()))).Replace('-', '') }
finally { $hash.Dispose() }
$mutex = [Threading.Mutex]::new($false, ('Local\ImageLoreRuntime-' + $mutexKey))
$ownsMutex = $false
try {
  try { $ownsMutex = $mutex.WaitOne(30000) }
  catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
  if (-not $ownsMutex) { throw 'Another ImageLore runtime publication is still running' }
if (-not $SourceDirectory) { $SourceDirectory = Join-Path $project 'src-tauri\target\release' }
$source = [IO.Path]::GetFullPath($SourceDirectory)
$binary = Join-Path $source 'imagelore.exe'
$version = (Get-Content -LiteralPath (Join-Path $project 'VERSION') -Raw).Trim()
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Build the native application first: $binary" }
$actual = (Get-Item -LiteralPath $binary).VersionInfo.ProductVersion
if ($actual -notmatch '(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)' -or $Matches[1] -ne $version) {
  throw "Application binary does not match VERSION=$version ($actual)"
}
$runtimeRoot = Join-Path $project 'desktop-runtime'
$current = Join-Path $runtimeRoot 'current'
$token = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss') + '-' + [Guid]::NewGuid().ToString('N')
$staged = Join-Path $runtimeRoot "next-$token"
$previous = Join-Path $runtimeRoot "previous-$token"
$failed = Join-Path $runtimeRoot "failed-$token"
foreach ($target in @($current,$staged,$previous,$failed)) {
  if (-not ([IO.Path]::GetFullPath($target).StartsWith($runtimeRoot + '\', [StringComparison]::OrdinalIgnoreCase))) {
    throw 'Runtime target escaped project desktop-runtime'
  }
}
New-Item -ItemType Directory -Path $staged -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $staged 'ImageLore.exe')
Get-ChildItem -LiteralPath $source -Filter '*.dll' -File | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination $staged
}
$manifest = @{ version=$version; executable='ImageLore.exe'; builtFrom=(git -C $project rev-parse HEAD); publishedAt=[DateTime]::UtcNow.ToString('o') }
[IO.File]::WriteAllText((Join-Path $staged 'runtime.json'), ($manifest | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
$hadPrevious = Test-Path -LiteralPath $current
if ($hadPrevious) { Move-Item -LiteralPath $current -Destination $previous }
try {
  Move-Item -LiteralPath $staged -Destination $current
  & (Join-Path $PSScriptRoot 'update_desktop_shortcut.ps1') -ExecutablePath (Join-Path $current 'ImageLore.exe') -ExpectedVersion $version -DesktopDirectory $DesktopDirectory -StatePath $StatePath
} catch {
  if (Test-Path -LiteralPath $current) { Move-Item -LiteralPath $current -Destination $failed }
  if ($hadPrevious) {
    Move-Item -LiteralPath $previous -Destination $current
  }
  throw
}
Write-Output "ImageLore $version desktop runtime: $current"
} finally {
  if ($ownsMutex) { $mutex.ReleaseMutex() }
  $mutex.Dispose()
}
