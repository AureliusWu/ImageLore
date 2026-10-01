param(
  [Parameter(Mandatory=$true)][string]$PreviousInstaller,
  [Parameter(Mandatory=$true)][string]$CurrentInstaller,
  [Parameter(Mandatory=$true)][string]$PreviousSchema,
  [string]$PreviousVersion = "0.19.0",
  [Parameter(Mandatory=$true)][string]$CurrentVersion
)

$ErrorActionPreference = "Stop"

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

# This test uninstalls applications and removes both ImageLore data directories.
# Refuse a workstation or self-hosted runner before inspecting any user paths.
Assert-True ($env:GITHUB_ACTIONS -eq "true" -and $env:RUNNER_ENVIRONMENT -eq "github-hosted") `
  "Upgrade smoke requires a disposable GitHub-hosted runner (GITHUB_ACTIONS=true, RUNNER_ENVIRONMENT=github-hosted)."
Assert-True (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) "LOCALAPPDATA is missing"
Assert-True (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) "APPDATA is missing"
Assert-True (-not [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) "RUNNER_TEMP is missing"

function Unquote([string]$Value) {
  if ($null -eq $Value) { return "" }
  return $Value.Trim().Trim('"')
}

function Assert-NoReparsePoint([string]$Path) {
  $cursor = [IO.Path]::GetFullPath($Path)
  while ($cursor) {
    if (Test-Path -LiteralPath $cursor) {
      $item = Get-Item -LiteralPath $cursor -Force
      Assert-True (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) "Reparse point refused: $cursor"
    }
    $cursor = [IO.Path]::GetDirectoryName($cursor)
  }
}

function Get-SafeDataRoot([string]$Path) {
  $full = [IO.Path]::GetFullPath($Path).TrimEnd([IO.Path]::DirectorySeparatorChar)
  $allowed = @($expectedDataRoots | Where-Object { [string]::Equals($_, $full, [StringComparison]::OrdinalIgnoreCase) })
  Assert-True ($allowed.Count -eq 1) "Cleanup path is not an expected ImageLore data directory: $full"
  Assert-NoReparsePoint $full
  if (Test-Path -LiteralPath $full) {
    Assert-True (Test-Path -LiteralPath $full -PathType Container) "Data path is not a directory: $full"
    # Inspect one level at a time: never follow an unchecked junction or symlink.
    $directories = [Collections.Generic.Stack[string]]::new()
    $directories.Push($full)
    while ($directories.Count -gt 0) {
      foreach ($item in Get-ChildItem -LiteralPath $directories.Pop() -Force) {
        Assert-True (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) "Reparse point refused: $($item.FullName)"
        if ($item.PSIsContainer) { $directories.Push($item.FullName) }
      }
    }
  }
  return $full
}

function Remove-ExpectedDataRoot([string]$Path) {
  $full = Get-SafeDataRoot $Path
  if (Test-Path -LiteralPath $full) {
    Remove-Item -LiteralPath $full -Recurse -Force
  }
}

function Invoke-Installer([string]$Path) {
  Assert-True (Test-Path -LiteralPath $Path -PathType Leaf) "Installer not found: $Path"
  $process = Start-Process -FilePath $Path -ArgumentList "/S" -WindowStyle Hidden -Wait -PassThru
  Assert-True ($process.ExitCode -eq 0) "Installer failed with exit code $($process.ExitCode): $Path"
}

function Invoke-RegisteredUninstaller {
  $registration = Get-ItemProperty -LiteralPath $uninstallKey
  $registeredDirectory = [IO.Path]::GetFullPath((Unquote $registration.InstallLocation))
  Assert-True ([string]::Equals($registeredDirectory, $legacyRoot, [StringComparison]::OrdinalIgnoreCase)) "Unexpected registered install directory: $registeredDirectory"
  $uninstaller = [IO.Path]::GetFullPath((Unquote $registration.UninstallString))
  Assert-True ([string]::Equals($uninstaller, (Join-Path $legacyRoot "uninstall.exe"), [StringComparison]::OrdinalIgnoreCase)) "Unexpected registered uninstaller: $uninstaller"
  Get-SafeDataRoot $legacyRoot | Out-Null
  Invoke-Installer $uninstaller
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class ImageLoreUpgradeWindow {
  private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);
  [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
  [DllImport("user32.dll")] private static extern IntPtr GetWindow(IntPtr window, uint command);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
  [DllImport("user32.dll", SetLastError=true)] private static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
  public static IntPtr Find(uint processId) {
    IntPtr result = IntPtr.Zero;
    EnumWindows((window, parameter) => {
      uint owner;
      GetWindowThreadProcessId(window, out owner);
      var title = new StringBuilder(512);
      GetWindowText(window, title, title.Capacity);
      if (owner == processId && GetWindow(window, 4) == IntPtr.Zero && title.ToString().StartsWith("ImageLore", StringComparison.Ordinal)) {
        result = window;
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return result;
  }
  public static bool Close(IntPtr window) {
    return PostMessage(window, 0x0010, IntPtr.Zero, IntPtr.Zero);
  }
}
'@

function Invoke-AppStartupAndClose([string]$Executable, [string]$LogPath, [string]$Label) {
  $previousLog = if (Test-Path -LiteralPath $LogPath) { [IO.File]::ReadAllText($LogPath) } else { "" }
  $startedAt = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
  $app = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
  $deadline = (Get-Date).AddSeconds(90)
  $ready = $false
  $window = [IntPtr]::Zero
  while ((Get-Date) -lt $deadline) {
    $app.Refresh()
    Assert-True (-not $app.HasExited) "$Label exited before startup was ready"
    if (Test-Path -LiteralPath $LogPath) {
      $currentLog = [IO.File]::ReadAllText($LogPath)
      Assert-True ($currentLog.StartsWith($previousLog, [StringComparison]::Ordinal)) "$Label diagnostics changed unexpectedly; refusing stale startup evidence"
      $freshLog = $currentLog.Substring($previousLog.Length)
      # diagnostics.rs emits Unix seconds, a level, and this exact message.
      foreach ($match in [regex]::Matches($freshLog, '(?m)^(\d+) \[INFO\] startup: library ready\r?$')) {
        if ([long]$match.Groups[1].Value -ge $startedAt) { $ready = $true }
      }
    }
    if ($ready) {
      $window = [ImageLoreUpgradeWindow]::Find([uint32]$app.Id)
      if ($window -ne [IntPtr]::Zero) { break }
    }
    Start-Sleep -Milliseconds 500
  }
  Assert-True ($ready -and $window -ne [IntPtr]::Zero) "$Label did not report current startup library readiness and create its window within 90 seconds"
  Assert-True ([ImageLoreUpgradeWindow]::Close($window)) "$Label WM_CLOSE could not be posted"
  Assert-True ($app.WaitForExit(30000)) "$Label did not exit normally within 30 seconds after WM_CLOSE; no forced termination was attempted"
  Assert-True ($app.ExitCode -eq 0) "$Label exited with code $($app.ExitCode)"
}

$uninstallRoot = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall"
$uninstallKey = Join-Path $uninstallRoot "ImageLore"
$localDataRoot = [IO.Path]::GetFullPath($env:LOCALAPPDATA)
$legacyRoot = [IO.Path]::GetFullPath((Join-Path $localDataRoot "ImageLore"))
$dataRoot = [IO.Path]::GetFullPath((Join-Path $localDataRoot "app.imagelore.desktop"))
$expectedDataRoots = @($legacyRoot, $dataRoot)
$startMenuLink = [IO.Path]::GetFullPath((Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\ImageLore\ImageLore.lnk"))
Get-SafeDataRoot $legacyRoot | Out-Null
Get-SafeDataRoot $dataRoot | Out-Null
Assert-NoReparsePoint $startMenuLink

if (Test-Path -LiteralPath $uninstallKey) { Invoke-RegisteredUninstaller }
Remove-ExpectedDataRoot $legacyRoot
Remove-ExpectedDataRoot $dataRoot
if (Test-Path -LiteralPath $startMenuLink) { Remove-Item -LiteralPath $startMenuLink -Force }

Write-Host "Installing upgrade baseline $PreviousVersion..."
Invoke-Installer $PreviousInstaller

Assert-True (Test-Path -LiteralPath $uninstallKey) "Baseline installer did not register ImageLore"
$oldReg = Get-ItemProperty -LiteralPath $uninstallKey
Assert-True ($oldReg.DisplayVersion -eq $PreviousVersion) "Expected baseline DisplayVersion $PreviousVersion, got $($oldReg.DisplayVersion)"
$oldInstallDir = Unquote $oldReg.InstallLocation
Assert-True ([string]::Equals([IO.Path]::GetFullPath($oldInstallDir), $legacyRoot, [StringComparison]::OrdinalIgnoreCase)) "Baseline install location changed: $oldInstallDir"
Assert-True (Test-Path -LiteralPath (Join-Path $oldInstallDir "ImageLore.exe")) "Baseline ImageLore.exe missing"
Assert-True (Test-Path -LiteralPath (Join-Path $oldInstallDir "uninstall.exe")) "Baseline uninstaller missing"
Get-SafeDataRoot $legacyRoot | Out-Null

# Use only fields present in the real v0.19 schema (version 7). Keep every
# seeded business row, not just an asset name, for exact readback comparisons.
$fixtureId = [Guid]::NewGuid().ToString("N")
$sentinelImage = Join-Path $env:RUNNER_TEMP "imagelore-upgrade-$fixtureId.png"
$expectedSnapshot = Join-Path $env:RUNNER_TEMP "imagelore-upgrade-$fixtureId.json"
$legacyDb = Join-Path $legacyRoot "library.sqlite3"
$seedDb = @'
import hashlib, json, pathlib, sqlite3, struct, sys, zlib
schema = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
db, image, expected = map(pathlib.Path, sys.argv[2:5])
if db.exists() or image.exists() or expected.exists():
    raise SystemExit("refusing to overwrite an existing upgrade fixture")
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
pixels = b"\x00" + b"\x40\x80\xc0" * 2
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels * 2)) + chunk(b"IEND", b"")
image.write_bytes(png)
con = sqlite3.connect(db)
con.executescript(schema)
version = con.execute("SELECT value FROM app_meta WHERE key='schema_version'").fetchone()
if version != ("7",):
    raise SystemExit(f"expected real baseline schema 7, got {version!r}")
con.execute("INSERT INTO assets(id,path,name,favorite,width,height,file_size,format,mime_type,metadata_type,generation_json,fingerprint,portable_id,file_mtime,missing,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", (1, str(image.resolve()), "Upgrade Sentinel", 1, 2, 2, len(png), "png", "image/png", "a1111", '{"seed":"12345","steps":20}', hashlib.sha256(png).hexdigest(), "il-upgrade-smoke", int(image.stat().st_mtime), 0, 1760000000, 1760000001))
con.execute("INSERT INTO prompt_state VALUES(?,?,?,?,?)", (1, "Upgrade prompt: keep colors and details", "Upgrade negative: blur", "upgrade-model-v1", 1760000002))
con.execute("INSERT INTO tags VALUES(?,?,?)", (1, "Upgrade Tag", 1760000003))
con.execute("INSERT INTO asset_tags VALUES(?,?,?)", (1, 1, 1760000004))
con.execute("INSERT INTO collections VALUES(?,?,?,?,?)", (1, "Upgrade Collection", "Preserve collection description", 1760000005, 1760000006))
con.execute("INSERT INTO collection_assets VALUES(?,?,?)", (1, 1, 1760000007))
con.execute("INSERT INTO prompt_revisions VALUES(?,?,?,?,?,?,?,?)", (1, 1, "Earlier upgrade prompt", "Earlier negative", "earlier-model", '["Upgrade Tag"]', "Preserve revision note", 1760000008))
con.commit()
orders = {"assets": "id", "prompt_state": "asset_id", "tags": "id", "asset_tags": "asset_id,tag_id", "collections": "id", "collection_assets": "collection_id,asset_id", "prompt_revisions": "id"}
snapshot = {table: con.execute(f"SELECT * FROM {table} ORDER BY {order}").fetchall() for table, order in orders.items()}
expected.write_text(json.dumps(snapshot), encoding="utf-8")
con.close()
'@
$seedDb | python - $PreviousSchema $legacyDb $sentinelImage $expectedSnapshot
Assert-True ($LASTEXITCODE -eq 0) "Failed to seed baseline business sentinels"
Assert-True (Test-Path -LiteralPath $legacyDb) "Failed to seed legacy database"

New-Item -ItemType Directory -Path (Join-Path $legacyRoot "backups") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $legacyRoot "models") -Force | Out-Null
$legacyBackup = Join-Path $legacyRoot "backups\upgrade-sentinel.sqlite3"
$legacyModel = Join-Path $legacyRoot "models\upgrade-sentinel.bin"
Copy-Item -LiteralPath $legacyDb -Destination $legacyBackup
[IO.File]::WriteAllBytes($legacyModel, [Text.Encoding]::UTF8.GetBytes("keep-model"))
$backupHash = (Get-FileHash -LiteralPath $legacyBackup -Algorithm SHA256).Hash
$modelHash = (Get-FileHash -LiteralPath $legacyModel -Algorithm SHA256).Hash

Write-Host "Installing current version $CurrentVersion over $PreviousVersion..."
Invoke-Installer $CurrentInstaller

Assert-True (Test-Path -LiteralPath $uninstallKey) "Current installer did not preserve ImageLore registration"
$newReg = Get-ItemProperty -LiteralPath $uninstallKey
Assert-True ($newReg.DisplayVersion -eq $CurrentVersion) "Expected DisplayVersion $CurrentVersion, got $($newReg.DisplayVersion)"
$newInstallDir = Unquote $newReg.InstallLocation
Assert-True ([string]::Equals([IO.Path]::GetFullPath($newInstallDir), [IO.Path]::GetFullPath($oldInstallDir), [StringComparison]::OrdinalIgnoreCase)) "Upgrade installed side-by-side instead of replacing old location"
$currentExe = Join-Path $newInstallDir "ImageLore.exe"
Assert-True (Test-Path -LiteralPath $currentExe) "Upgraded ImageLore.exe missing"
Assert-True (Test-Path -LiteralPath (Join-Path $newInstallDir "uninstall.exe")) "Upgraded uninstaller missing"

$entries = @(
  Get-ChildItem -LiteralPath $uninstallRoot | ForEach-Object {
    try { Get-ItemProperty -LiteralPath $_.PSPath } catch { $null }
  } | Where-Object { $_ -and $_.DisplayName -eq "ImageLore" }
)
Assert-True ($entries.Count -eq 1) "Expected exactly one ImageLore installed-app entry, found $($entries.Count)"
$exeVersion = (Get-Item -LiteralPath $currentExe).VersionInfo.ProductVersion
Assert-True ($exeVersion -like "$CurrentVersion*") "Installed executable version is not ${CurrentVersion}: $exeVersion"
Assert-True (Test-Path -LiteralPath $startMenuLink) "Start menu shortcut missing after upgrade"
Assert-NoReparsePoint $startMenuLink
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($startMenuLink)
Assert-True ([string]::Equals([IO.Path]::GetFullPath($shortcut.TargetPath), [IO.Path]::GetFullPath($currentExe), [StringComparison]::OrdinalIgnoreCase)) "Start menu shortcut points to the wrong executable"

$newDb = Join-Path $dataRoot "library.sqlite3"
$logPath = Join-Path $dataRoot "logs\imagelore.log"
$verifyDb = @'
import json, pathlib, sqlite3, sys
expected = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
# mode=ro keeps verification from silently creating or repairing a missing DB;
# do not use immutable=1, which would ignore committed WAL contents.
con = sqlite3.connect(pathlib.Path(sys.argv[1]).resolve().as_uri() + "?mode=ro", uri=True)
version = con.execute("SELECT value FROM app_meta WHERE key='schema_version'").fetchone()
if version != ("11",):
    raise SystemExit(f"expected migrated schema 11, got {version!r}")
integrity = con.execute("PRAGMA integrity_check").fetchall()
if integrity != [("ok",)]:
    raise SystemExit(f"integrity check failed: {integrity!r}")
foreign_keys = con.execute("PRAGMA foreign_key_check").fetchall()
if foreign_keys:
    raise SystemExit(f"foreign key check failed: {foreign_keys!r}")
orders = {"assets": "id", "prompt_state": "asset_id", "tags": "id", "asset_tags": "asset_id,tag_id", "collections": "id", "collection_assets": "collection_id,asset_id", "prompt_revisions": "id"}
for table, order in orders.items():
    rows = [list(row) for row in con.execute(f"SELECT * FROM {table} ORDER BY {order}")]
    if rows != expected[table]:
        raise SystemExit(f"business sentinel changed in {table}: {rows!r}")
con.close()
'@

foreach ($label in @("first upgraded launch", "upgraded restart")) {
  Invoke-AppStartupAndClose $currentExe $logPath $label
  Assert-True (Test-Path -LiteralPath $newDb) "$label did not migrate the legacy database to the isolated data root"
  Assert-True (-not (Test-Path -LiteralPath $legacyDb)) "$label left the legacy database in the install directory"
  $verifyDb | python - $newDb $expectedSnapshot
  Assert-True ($LASTEXITCODE -eq 0) "$label failed schema, integrity, foreign-key or business-sentinel readback"
  $newBackup = Join-Path $dataRoot "backups\upgrade-sentinel.sqlite3"
  $newModel = Join-Path $dataRoot "models\upgrade-sentinel.bin"
  Assert-True (Test-Path -LiteralPath $newBackup) "$label did not preserve the backup directory"
  Assert-True (Test-Path -LiteralPath $newModel) "$label did not preserve the model directory"
  Assert-True ((Get-FileHash -LiteralPath $newBackup -Algorithm SHA256).Hash -eq $backupHash) "$label changed the preserved backup bytes"
  Assert-True ((Get-FileHash -LiteralPath $newModel -Algorithm SHA256).Hash -eq $modelHash) "$label changed the preserved model bytes"
}

# Only successful runs are cleaned; a failure keeps diagnostics on the runner.
Invoke-RegisteredUninstaller
Remove-ExpectedDataRoot $legacyRoot
Remove-ExpectedDataRoot $dataRoot
Write-Host "ImageLore Windows upgrade smoke test: PASS ($PreviousVersion -> $CurrentVersion; two normal closes and complete readbacks)"
