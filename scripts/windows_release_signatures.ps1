# Read-only release verification. No private key, signing request or trust-store write.
function Get-ReleaseStreamHash([IO.Stream]$Stream) {
  $algorithm = [Security.Cryptography.SHA256]::Create()
  try {
    $Stream.Position = 0
    return [BitConverter]::ToString($algorithm.ComputeHash($Stream)).Replace('-', '').ToLowerInvariant()
  } finally { $algorithm.Dispose() }
}

function Assert-ReleaseNoReparsePoint([string]$Path) {
  $cursor = [IO.Path]::GetFullPath($Path)
  while ($cursor) {
    if (Test-Path -LiteralPath $cursor) {
      if ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'Release input has a reparse-point ancestor'
      }
    }
    $cursor = [IO.Path]::GetDirectoryName($cursor)
  }
}

function Get-ApprovedReleaseCertificateHashes($Policy) {
  if ($Policy.schemaVersion -ne 1 -or $Policy.productName -cne 'ImageLore') { throw 'Unsupported Windows signing policy' }
  $pins = @($Policy.approvedSignerCertificateSha256)
  if ($pins.Count -eq 0) { throw 'No approved Windows signing certificate is configured; release upload refused' }
  foreach ($pin in $pins) {
    if ($pin -isnot [string] -or $pin -notmatch '^[a-fA-F0-9]{64}$') { throw 'Invalid approved signing certificate SHA-256' }
    $pin.ToLowerInvariant()
  }
}

function Get-ReleaseNumericVersion([string]$Text) {
  $parsed = $null
  if (-not [Version]::TryParse(($Text -split '[-+]')[0], [ref]$parsed) -or $parsed.Build -lt 0) {
    throw 'Release artifact has an invalid numeric file version'
  }
  return [Version]::new($parsed.Major, $parsed.Minor, $parsed.Build, [Math]::Max($parsed.Revision, 0))
}

function Assert-ReleaseSignatureEvidence($Evidence, $Policy, [string]$ExpectedVersion) {
  $pins = @(Get-ApprovedReleaseCertificateHashes $Policy)
  if ($ExpectedVersion -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$') { throw 'Invalid expected release version' }
  if ($Evidence.signatureStatus -cne 'Valid' -or $Evidence.signatureType -cne 'Authenticode') {
    throw 'Release artifact is not a valid embedded Authenticode signature'
  }
  if ($Evidence.productName -cne $Policy.productName -or $Evidence.productVersion -cne $ExpectedVersion) {
    throw 'Release artifact product name or product version does not match the release'
  }
  if ((Get-ReleaseNumericVersion $Evidence.fileVersion) -ne (Get-ReleaseNumericVersion $ExpectedVersion)) {
    throw 'Release artifact file version does not match the release'
  }
  if ($pins -notcontains $Evidence.signerCertificateSha256) { throw 'Release artifact signer certificate is not approved' }
  if ($Evidence.hasTimestamp -ne $true) { throw 'Release artifact has no trusted timestamp certificate' }
}

function Get-ReleaseFileEvidence([string]$Path, [IO.Stream]$Held) {
  $version = (Get-Item -LiteralPath $Path).VersionInfo
  $signature = Get-AuthenticodeSignature -LiteralPath $Path
  $certificateHash = $null
  if ($signature.SignerCertificate) {
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { $certificateHash = [BitConverter]::ToString($algorithm.ComputeHash($signature.SignerCertificate.RawData)).Replace('-', '').ToLowerInvariant() }
    finally { $algorithm.Dispose() }
  }
  return [pscustomobject]@{
    productName = $version.ProductName
    productVersion = $version.ProductVersion
    fileVersion = $version.FileVersion
    signatureStatus = [string]$signature.Status
    signatureType = [string]$signature.SignatureType
    signerCertificateSha256 = $certificateHash
    hasTimestamp = ($null -ne $signature.TimeStamperCertificate)
    sha256 = Get-ReleaseStreamHash $Held
  }
}

function Get-ReleaseSdkSignTool {
  $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
  if (-not (Test-Path -LiteralPath $sdkRoot -PathType Container)) { throw 'Windows SDK SignTool is required for release verification' }
  $tools = @(Get-ChildItem -LiteralPath $sdkRoot -Directory | ForEach-Object {
    $version = $null
    if ([Version]::TryParse($_.Name, [ref]$version)) {
      $path = Join-Path $_.FullName 'x64/signtool.exe'
      if (Test-Path -LiteralPath $path -PathType Leaf) { [pscustomobject]@{ version = $version; path = $path } }
    }
  } | Sort-Object version -Descending)
  if ($tools.Count -eq 0) { throw 'Windows SDK x64 SignTool is missing' }
  $tool = $tools[0].path
  Assert-ReleaseNoReparsePoint $tool
  $identity = Get-AuthenticodeSignature -LiteralPath $tool
  if ($identity.Status -ne 'Valid' -or -not $identity.SignerCertificate -or $identity.SignerCertificate.Subject -notmatch 'CN=Microsoft (Corporation|Windows)') {
    throw 'Windows SDK SignTool does not have a valid Microsoft signature'
  }
  return $tool
}

function Invoke-WindowsReleaseSignatureGate([string]$ApplicationPath, [string]$InstallerPath, [string]$ExpectedVersion, $Policy) {
  if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Windows release verification requires Windows' }
  $null = @(Get-ApprovedReleaseCertificateHashes $Policy)
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
      Assert-ReleaseSignatureEvidence $item $Policy $ExpectedVersion
      $evidence += $item
    }
    $tool = Get-ReleaseSdkSignTool
    for ($index = 0; $index -lt $paths.Count; $index++) {
      # No /a catalog search: verify every embedded signature and require timestamps.
      $toolOutput = & $tool verify /pa /all /v /tw $paths[$index] 2>&1
      $toolCode = $LASTEXITCODE
      $toolOutput | ForEach-Object { Write-Host $_ }
      if ($toolCode -ne 0) { throw 'SignTool refused a release artifact or reported a verification warning' }
      if ((Get-ReleaseStreamHash $held[$index]) -cne $evidence[$index].sha256) { throw 'Release artifact bytes changed during verification' }
    }
    return [pscustomobject]@{ applicationSha256 = $evidence[0].sha256; installerSha256 = $evidence[1].sha256; version = $ExpectedVersion }
  } finally {
    foreach ($stream in $held) { $stream.Dispose() }
  }
}
