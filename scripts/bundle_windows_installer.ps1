param(
  [ValidateSet('', 'x86_64-pc-windows-msvc')][string]$Target = '',
  [switch]$DebugBuild,
  [switch]$NoSign
)
$ErrorActionPreference = 'Stop'
function Get-StreamSha256([IO.Stream]$Stream) {
  $algorithm = [Security.Cryptography.SHA256]::Create()
  try {
    $Stream.Position = 0
    return [BitConverter]::ToString($algorithm.ComputeHash($Stream)).Replace('-', '').ToLowerInvariant()
  } finally { $algorithm.Dispose() }
}
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Windows installer bundling requires Windows' }
$manifest = Get-Content -LiteralPath (Join-Path $project 'src-tauri/windows-runtime.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or $manifest.target -ne 'x86_64-pc-windows-msvc' -or $manifest.directML.fileName -cne 'DirectML.dll') {
  throw 'Unsupported Windows runtime manifest'
}
if ($manifest.directML.sha256 -cnotmatch '^[0-9a-f]{64}$' -or $manifest.directML.bytes -le 0) { throw 'Invalid DirectML identity' }
$config = Get-Content -LiteralPath (Join-Path $project 'src-tauri/tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($config.bundle.windows.bundleVCRuntime -ne $true) { throw 'Windows installer must include the Visual C++ runtime' }
if ($config.bundle.resources -is [array] -and $config.bundle.resources.Count -gt 0) {
  throw 'Convert bundle.resources to a source/target map before adding runtime resources, to preserve existing assets'
}
if (-not $Target -and $env:CARGO_BUILD_TARGET) { $Target = $env:CARGO_BUILD_TARGET }
if ($Target -and $Target -ne $manifest.target) { throw 'Only the declared Windows x64 target is supported' }
if (-not $Target) {
  $hostInfo = & rustc -vV
  if ($LASTEXITCODE -ne 0 -or @($hostInfo | Where-Object { $_ -ceq ('host: ' + $manifest.target) }).Count -ne 1) {
    throw 'Specify the Windows x64 target when building on another host architecture'
  }
}
Push-Location $project
try {
  $metadataText = & cargo metadata --locked --offline --no-deps --format-version 1 --manifest-path src-tauri/Cargo.toml
  if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the Cargo output directory' }
  $metadata = $metadataText | ConvertFrom-Json
  $targetDirectory = [IO.Path]::GetFullPath($metadata.target_directory)
  if ($Target) { $targetDirectory = Join-Path $targetDirectory $Target }
  $buildProfile = if ($DebugBuild) { 'debug' } else { 'release' }
  $outputDirectory = Join-Path $targetDirectory $buildProfile
  $binary = Join-Path $outputDirectory 'imagelore.exe'
  $directML = Join-Path $outputDirectory $manifest.directML.fileName
  if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'Build the application with tauri build --no-bundle before bundling' }
  if (-not (Test-Path -LiteralPath $directML -PathType Leaf)) { throw 'The application output is missing DirectML.dll' }
  $lockfile = Get-Content -LiteralPath (Join-Path $project 'src-tauri/Cargo.lock') -Raw -Encoding UTF8
  $ortPattern = '(?m)^name = "ort-sys"\r?\nversion = "' + [regex]::Escape($manifest.ortSysVersion) + '"\r?$'
  if ([regex]::Matches($lockfile, $ortPattern).Count -ne 1) { throw 'Runtime manifest does not match the locked ort-sys version' }
  $held = [IO.File]::Open($directML, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try {
    $directMLHash = Get-StreamSha256 $held
    if ($held.Length -ne $manifest.directML.bytes -or $directMLHash -cne $manifest.directML.sha256) {
      throw 'DirectML.dll does not match the reviewed Windows runtime manifest'
    }
    $inputDirectory = Join-Path $outputDirectory ('bundle-inputs/' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $inputDirectory | Out-Null
    $stagedDLL = Join-Path $inputDirectory 'DirectML.dll'
    $writer = [IO.File]::Open($stagedDLL, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
      $held.Position = 0
      $held.CopyTo($writer)
      $writer.Flush($true)
    } finally { $writer.Dispose() }
    $staged = [IO.File]::Open($stagedDLL, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
      if ((Get-StreamSha256 $staged) -cne $directMLHash) { throw 'Staged DirectML.dll hash does not match the reviewed input' }
      $overlay = Join-Path $inputDirectory 'runtime-resources.json'
      $resources = [ordered]@{}
      $resources[$stagedDLL] = 'DirectML.dll'
      $value = [ordered]@{ bundle = [ordered]@{ resources = $resources } }
      [IO.File]::WriteAllText($overlay, ($value | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
      $node = (Get-Command node -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
      $cli = Join-Path $project 'node_modules/@tauri-apps/cli/tauri.js'
      $arguments = @($cli, 'bundle', '--bundles', 'nsis', '--ci', '--config', $overlay)
      if ($Target) { $arguments += @('--target', $Target) }
      if ($DebugBuild) { $arguments += '--debug' }
      if ($NoSign) { $arguments += '--no-sign' }
      & $node @arguments
      if ($LASTEXITCODE -ne 0) { throw 'Windows installer bundling failed' }
      $held.Position = 0
      if ((Get-StreamSha256 $held) -cne $directMLHash) {
        throw 'DirectML.dll changed during bundling'
      }
      if ((Get-StreamSha256 $staged) -cne $directMLHash) { throw 'Staged DirectML.dll changed during bundling' }
      Write-Output ('Bundled reviewed DirectML.dll: SHA256=' + $directMLHash)
    } finally { $staged.Dispose() }
  } finally { $held.Dispose() }
} finally { Pop-Location }
