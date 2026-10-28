# The strict signature verifier remains unchanged. This exception is scoped to
# the explicitly authorized v1.0.0 release and never selects future releases.
. (Join-Path $PSScriptRoot 'windows_release_signatures.ps1')

function Get-KnownLimitationsReleasePolicy([string]$Root) {
  $fullRoot = [IO.Path]::GetFullPath($Root)
  if (-not (Test-Path -LiteralPath $fullRoot -PathType Container)) { throw 'Release repository root is missing' }
  Assert-ReleaseNoReparsePoint $fullRoot
  $policyPath = Join-Path $fullRoot 'src-tauri/release-exceptions.json'
  $notesPath = Join-Path $fullRoot 'docs/releases/v1.0.0.md'
  foreach ($path in @($policyPath, $notesPath)) {
    Assert-ReleaseNoReparsePoint $path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw 'Required fixed release policy or notes file is missing' }
  }
  $policyStream = $null
  $notesStream = $null
  try {
    $policyStream = [IO.File]::Open($policyPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $notesStream = [IO.File]::Open($notesPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    if ($policyStream.Length -gt 65536 -or $notesStream.Length -gt 1048576) { throw 'Release policy or notes exceeds its bounded text size' }
    $utf8 = [Text.UTF8Encoding]::new($false, $true)
    $policyBuffer = [IO.MemoryStream]::new()
    try {
      $policyStream.CopyTo($policyBuffer)
      try { $policyText = $utf8.GetString($policyBuffer.ToArray()) } catch { throw 'Release policy is not valid UTF-8' }
    } finally { $policyBuffer.Dispose() }
    try { $policy = $policyText | ConvertFrom-Json } catch { throw 'Release exception policy is not valid JSON' }
    $expectedFields = @('schemaVersion', 'productName', 'version', 'tag', 'status', 'releaseNotesPath', 'releaseNotesSha256', 'limitations')
    $fields = @($policy.PSObject.Properties.Name)
    if ($fields.Count -ne $expectedFields.Count) { throw 'Release exception policy has missing or unexpected fields' }
    foreach ($field in $expectedFields) {
      if ($fields -cnotcontains $field) { throw 'Release exception policy has missing or unexpected fields' }
    }
    if (($policy.schemaVersion -isnot [int] -and $policy.schemaVersion -isnot [long]) -or $policy.schemaVersion -ne 1) { throw 'Unsupported release exception policy schema' }
    foreach ($field in @('productName', 'version', 'tag', 'status', 'releaseNotesPath', 'releaseNotesSha256')) {
      if ($policy.$field -isnot [string]) { throw 'Release exception policy contains an invalid text field' }
    }
    if ($policy.productName -cne 'ImageLore' -or $policy.version -cne '1.0.0' -or $policy.tag -cne 'refs/tags/v1.0.0' -or $policy.status -cne 'RELEASED_WITH_WARNINGS') { throw 'Release exception policy is not the authorized v1.0.0 limitation release' }
    if ($policy.releaseNotesPath -cne 'docs/releases/v1.0.0.md') { throw 'Release exception notes path differs from its fixed location' }
    if ($policy.releaseNotesSha256 -cnotmatch '^[a-f0-9]{64}$') { throw 'Release exception notes SHA-256 is invalid' }
    $expectedLimitations = @('unsigned-windows-binaries', 'manual-updates-only', 'desktop-and-performance-acceptance-deferred', 'third-party-inventory-not-link-map')
    $limitations = @($policy.limitations)
    if ($limitations.Count -ne 4) { throw 'Release exception limitation set differs from the authorized four items' }
    foreach ($limitation in $limitations) {
      if ($limitation -isnot [string] -or $expectedLimitations -cnotcontains $limitation) { throw 'Release exception limitation set differs from the authorized four items' }
    }
    foreach ($limitation in $expectedLimitations) {
      if ($limitations -cnotcontains $limitation) { throw 'Release exception limitation set differs from the authorized four items' }
    }
    $notesBuffer = [IO.MemoryStream]::new()
    try {
      $notesStream.CopyTo($notesBuffer)
      $notesBytes = $notesBuffer.ToArray()
      try { $notesText = $utf8.GetString($notesBytes) } catch { throw 'Release notes are not valid UTF-8' }
    } finally { $notesBuffer.Dispose() }
    if (($notesBytes.Length -ge 3 -and $notesBytes[0] -eq 239 -and $notesBytes[1] -eq 187 -and $notesBytes[2] -eq 191) -or $notesText.Contains("`r")) { throw 'Release notes must use UTF-8 without BOM and LF line endings' }
    if (-not $notesText.Contains('RELEASED_WITH_WARNINGS')) { throw 'Release notes do not disclose RELEASED_WITH_WARNINGS' }
    $notesHash = Get-ReleaseStreamHash $notesStream
    if ($notesHash -cne $policy.releaseNotesSha256) { throw 'Release notes bytes differ from their approved SHA-256' }
    return $policy
  } finally {
    if ($notesStream) { $notesStream.Dispose() }
    if ($policyStream) { $policyStream.Dispose() }
  }
}

function Get-ReleasePublicationMode([string]$Root, [string]$ExpectedVersion, [string]$Ref) {
  if ($ExpectedVersion -cne '1.0.0' -or $Ref -cne 'refs/tags/v1.0.0') { return 'SIGNED_REQUIRED' }
  $null = Get-KnownLimitationsReleasePolicy $Root
  return 'UNSIGNED_KNOWN_LIMITATIONS'
}

function Assert-KnownLimitationsReleaseEvidence($Evidence, [string]$ExpectedVersion) {
  if ($ExpectedVersion -cne '1.0.0') { throw 'Known-limitations artifact verification only accepts version 1.0.0' }
  if ($Evidence.productName -cne 'ImageLore' -or $Evidence.productVersion -cne '1.0.0') { throw 'Known-limitations artifact product name or product version does not match the release' }
  if ((Get-ReleaseNumericVersion $Evidence.fileVersion) -ne [Version]::new(1, 0, 0, 0)) { throw 'Known-limitations artifact file version does not match the release' }
  if ($Evidence.signatureStatus -cne 'NotSigned' -or $Evidence.signatureType -cne 'None' -or $null -ne $Evidence.signerCertificateSha256 -or $Evidence.hasTimestamp -isnot [bool] -or $Evidence.hasTimestamp -ne $false) { throw 'Known-limitations artifact must be genuinely NotSigned without a signer, catalog signature or timestamp' }
  if ($Evidence.sha256 -isnot [string] -or $Evidence.sha256 -cnotmatch '^[a-f0-9]{64}$') { throw 'Known-limitations artifact has no valid full SHA-256' }
}

function Invoke-KnownLimitationsReleaseGate([string]$Root, [string]$ApplicationPath, [string]$InstallerPath, [string]$ExpectedVersion, [string]$Ref) {
  if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Known-limitations release verification requires Windows' }
  if ((Get-ReleasePublicationMode $Root $ExpectedVersion $Ref) -cne 'UNSIGNED_KNOWN_LIMITATIONS') { throw 'Known-limitations publication requires the exact authorized v1.0.0 tag; a signed release is required otherwise' }
  $policy = Get-KnownLimitationsReleasePolicy $Root
  $paths = @([IO.Path]::GetFullPath($ApplicationPath), [IO.Path]::GetFullPath($InstallerPath))
  if ([string]::Equals($paths[0], $paths[1], [StringComparison]::OrdinalIgnoreCase)) { throw 'Application and installer must be distinct release files' }
  $held = [Collections.Generic.List[IO.FileStream]]::new()
  $evidence = @()
  try {
    foreach ($path in $paths) {
      Assert-ReleaseNoReparsePoint $path
      $stream = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
      $held.Add($stream)
      $item = Get-ReleaseFileEvidence $path $stream
      Assert-KnownLimitationsReleaseEvidence $item $ExpectedVersion
      $evidence += $item
    }
    for ($index = 0; $index -lt $held.Count; $index++) {
      if ((Get-ReleaseStreamHash $held[$index]) -cne $evidence[$index].sha256) { throw 'Known-limitations artifact bytes changed during verification' }
    }
    $null = Get-KnownLimitationsReleasePolicy $Root
    return [pscustomobject]@{
      state = 'PASS_KNOWN_LIMITATIONS_RELEASE'
      releaseStatus = 'RELEASED_WITH_WARNINGS'
      publicationMode = 'UNSIGNED_KNOWN_LIMITATIONS'
      version = '1.0.0'
      tag = 'refs/tags/v1.0.0'
      releaseNotesPath = 'docs/releases/v1.0.0.md'
      releaseNotesSha256 = $policy.releaseNotesSha256
      limitations = @($policy.limitations)
      applicationSha256 = $evidence[0].sha256
      installerSha256 = $evidence[1].sha256
      applicationSignatureStatus = $evidence[0].signatureStatus
      installerSignatureStatus = $evidence[1].signatureStatus
    }
  } finally { foreach ($stream in $held) { $stream.Dispose() } }
}
