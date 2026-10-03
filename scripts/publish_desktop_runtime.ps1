param(
  [string]$SourceDirectory = '',
  [string]$DesktopDirectory = [Environment]::GetFolderPath('Desktop'),
  [string]$StatePath = (Join-Path $env:LOCALAPPDATA 'app.imagelore.desktop\desktop-shortcut.json')
)
$ErrorActionPreference = 'Stop'
$usesDefaultSource = -not $PSBoundParameters.ContainsKey('SourceDirectory')
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
$sourceManifestPath = Join-Path $source 'runtime.json'
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
if (Test-Path -LiteralPath $sourceManifestPath) {
  if (-not (Test-Path -LiteralPath $sourceManifestPath -PathType Leaf)) { throw 'Source runtime.json must be a file' }
  $sourceManifest = Get-Content -LiteralPath $sourceManifestPath -Raw | ConvertFrom-Json
  if ($sourceManifest -isnot [pscustomobject] -or
      $sourceManifest.builtFrom -isnot [string] -or $sourceManifest.builtFrom -cnotmatch '\A[0-9A-Fa-f]{40}\z' -or
      $sourceManifest.sha256 -isnot [string] -or $sourceManifest.sha256 -cnotmatch '\A[0-9A-Fa-f]{64}\z' -or
      $sourceManifest.version -isnot [string] -or $sourceManifest.version -cne $version -or
      $sourceManifest.executable -isnot [string] -or $sourceManifest.executable -cne 'ImageLore.exe') {
    throw 'Source runtime.json must contain a full builtFrom commit, SHA256, matching version, and ImageLore.exe executable'
  }
  if ($sourceManifest.sha256 -ine $binaryHash) { throw 'Source executable SHA256 does not match runtime.json' }
  $builtFrom = $sourceManifest.builtFrom
  $builtFromSource = 'source-runtime-manifest'
} else {
  if (-not $usesDefaultSource) { throw 'An explicit SourceDirectory requires runtime.json with artifact provenance' }
  # Legacy desktop:build compatibility: HEAD is only a clean-worktree assumption.
  $builtFrom = & git -C $project rev-parse HEAD
  if ($LASTEXITCODE -ne 0 -or $builtFrom -isnot [string] -or $builtFrom -cnotmatch '\A[0-9A-Fa-f]{40}\z') {
    throw 'Cannot resolve a full Git HEAD for the default build directory'
  }
  $trackedChanges = & git -C $project status --porcelain --untracked-files=no
  if ($LASTEXITCODE -ne 0 -or $trackedChanges) { throw 'Default build publication requires a clean Git tracked worktree' }
  $builtFromSource = 'legacy-clean-head'
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
$stagedHash = (Get-FileHash -LiteralPath (Join-Path $staged 'ImageLore.exe') -Algorithm SHA256).Hash
if ($stagedHash -ine $binaryHash) { throw 'Staged executable SHA256 changed during publication' }
Get-ChildItem -LiteralPath $source -Filter '*.dll' -File | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination $staged
}
$manifest = @{ version=$version; executable='ImageLore.exe'; builtFrom=$builtFrom; builtFromSource=$builtFromSource; sha256=$stagedHash; publishedAt=[DateTime]::UtcNow.ToString('o') }
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
