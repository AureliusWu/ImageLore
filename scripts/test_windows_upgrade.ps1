param(
  [Parameter(Mandatory=$true)][string]$PreviousInstaller,
  [Parameter(Mandatory=$true)][string]$CurrentInstaller,
  [Parameter(Mandatory=$true)][string]$CurrentExecutable,
  [Parameter(Mandatory=$true)][string]$PreviousSchema,
  [ValidateSet('0.19.0', '0.25.1')][string]$PreviousVersion = "0.19.0",
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
$expectedBaselineSchema = if ($PreviousVersion -eq '0.19.0') { '7' } else { '11' }
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
  $installDirectory = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Executable))
  $requiredRuntime = @('DirectML.dll', 'msvcp140.dll', 'msvcp140_1.dll')
  $directMLIdentity = (Get-Content -LiteralPath (Join-Path $PSScriptRoot '../src-tauri/windows-runtime.json') -Raw -Encoding UTF8 | ConvertFrom-Json).directML
  foreach ($runtime in $requiredRuntime) {
    $runtimePath = Join-Path $installDirectory $runtime
    Assert-NoReparsePoint $runtimePath
    Assert-True (Test-Path -LiteralPath $runtimePath -PathType Leaf) "$Label is missing bundled $runtime"
  }
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
  # A runner may have global native runtimes. Require the application's own DLLs
  # to be loaded, so that machine-wide dependencies cannot hide a missing payload.
  foreach ($runtime in $requiredRuntime) {
    $runtimePath = [IO.Path]::GetFullPath((Join-Path $installDirectory $runtime))
    $loaded = @($app.Modules | Where-Object { $_.ModuleName -ieq $runtime })
    Assert-True ($loaded.Count -eq 1) "$Label did not load exactly one $runtime module"
    Assert-True ([string]::Equals([IO.Path]::GetFullPath($loaded[0].FileName), $runtimePath, [StringComparison]::OrdinalIgnoreCase)) "$Label loaded $runtime outside the installation directory"
    $runtimeHash = (Get-FileHash -LiteralPath $runtimePath -Algorithm SHA256).Hash
    $runtimeVersion = (Get-Item -LiteralPath $runtimePath).VersionInfo.FileVersion
    if ($runtime -ieq 'DirectML.dll') {
      Assert-True ($runtimeHash -ieq $directMLIdentity.sha256 -and (Get-Item -LiteralPath $runtimePath).Length -eq $directMLIdentity.bytes) "$Label installed DirectML.dll does not match the reviewed manifest"
    }
    Write-Host "$Label bundled runtime: $runtime, version=$runtimeVersion, SHA256=$runtimeHash, loaded-from-install=true"
  }
  Assert-True ([ImageLoreUpgradeWindow]::Close($window)) "$Label WM_CLOSE could not be posted"
  Assert-True ($app.WaitForExit(30000)) "$Label did not exit normally within 30 seconds after WM_CLOSE; no forced termination was attempted"
  Assert-True ($app.ExitCode -eq 0) "$Label exited with code $($app.ExitCode)"
}

Assert-NoReparsePoint $CurrentExecutable
# Keep the built EXE immutable while the installer payload is exercised.
$builtExecutable = [IO.File]::Open([IO.Path]::GetFullPath($CurrentExecutable), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
try {
$expectedExecutableHash = (Get-FileHash -LiteralPath $CurrentExecutable -Algorithm SHA256).Hash
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

# Seed the actual pinned old schema. Version 0.19 uses the install directory;
# 0.25.1 already uses the separate data root and has all schema-11 features.
$fixtureId = [Guid]::NewGuid().ToString("N")
$sentinelImage = Join-Path $env:RUNNER_TEMP "imagelore-upgrade-$fixtureId.png"
$expectedSnapshot = Join-Path $env:RUNNER_TEMP "imagelore-upgrade-$fixtureId.json"
$legacyDb = Join-Path $legacyRoot "library.sqlite3"
$baselineDataRoot = if ($PreviousVersion -eq '0.19.0') { $legacyRoot } else { $dataRoot }
New-Item -ItemType Directory -Path $baselineDataRoot -Force | Out-Null
$baselineDb = Join-Path $baselineDataRoot 'library.sqlite3'
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
con.execute("PRAGMA foreign_keys=ON")
con.executescript(schema)
version = con.execute("SELECT value FROM app_meta WHERE key='schema_version'").fetchone()
if version != (sys.argv[5],) or sys.argv[5] not in ("7", "11"):
    raise SystemExit(f"expected pinned baseline schema {sys.argv[5]}, got {version!r}")
con.execute("INSERT INTO assets(id,path,name,favorite,width,height,file_size,format,mime_type,metadata_type,generation_json,fingerprint,portable_id,file_mtime,missing,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", (1, str(image.resolve()), "Upgrade Sentinel", 1, 2, 2, len(png), "png", "image/png", "a1111", '{"seed":"12345","steps":20}', hashlib.sha256(png).hexdigest(), "il-upgrade-smoke", int(image.stat().st_mtime), 0, 1760000000, 1760000001))
con.execute("INSERT INTO prompt_state VALUES(?,?,?,?,?)", (1, "Upgrade prompt: keep colors and details", "Upgrade negative: blur", "upgrade-model-v1", 1760000002))
con.execute("INSERT INTO tags VALUES(?,?,?)", (1, "Upgrade Tag", 1760000003))
con.execute("INSERT INTO asset_tags VALUES(?,?,?)", (1, 1, 1760000004))
con.execute("INSERT INTO collections VALUES(?,?,?,?,?)", (1, "Upgrade Collection", "Preserve collection description", 1760000005, 1760000006))
con.execute("INSERT INTO collection_assets VALUES(?,?,?)", (1, 1, 1760000007))
con.execute("INSERT INTO prompt_revisions VALUES(?,?,?,?,?,?,?,?)", (1, 1, "Earlier upgrade prompt", "Earlier negative", "earlier-model", '["Upgrade Tag"]', "Preserve revision note", 1760000008))
orders = {"assets": "id", "prompt_state": "asset_id", "tags": "id", "asset_tags": "asset_id,tag_id", "collections": "id", "collection_assets": "collection_id,asset_id", "prompt_revisions": "id"}
if version == ("11",):
    reference_image = image.with_name(image.stem + "-reference.png")
    if reference_image.exists():
        raise SystemExit("refusing to overwrite a reference image fixture")
    reference_png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress((b"\x00" + b"\x80\x40\xc0" * 2) * 2)) + chunk(b"IEND", b"")
    reference_image.write_bytes(reference_png)
    columns = [row[1] for row in con.execute("PRAGMA table_info(assets)")]
    reference_row = dict(zip(columns, con.execute("SELECT * FROM assets WHERE id=1").fetchone()))
    reference_row.update(id=2, path=str(reference_image.resolve()), name="Upgrade Reference", file_size=len(reference_png), fingerprint=hashlib.sha256(reference_png).hexdigest(), portable_id="il-upgrade-reference")
    con.execute("INSERT INTO assets(" + ",".join(columns) + ") VALUES(" + ",".join("?" for _ in columns) + ")", [reference_row[column] for column in columns])
    con.executescript("""
      INSERT INTO relations VALUES(1,2,1,'reference','Preserve lineage',1760000009);
      INSERT INTO visual_dna(asset_id,subject,style,search_text,source,updated_at) VALUES(1,'Upgrade subject','Upgrade style','Upgrade subject Upgrade style','manual',1760000010);
      INSERT INTO generation_index VALUES(1,'12345',20,'','',NULL,NULL);
      INSERT INTO generation_sessions VALUES(1,'Upgrade session','Preserve session',1760000011,1760000012);
      INSERT INTO asset_sessions VALUES(1,1,'Preserve session membership',1760000013,1760000014);
      INSERT INTO model_aliases VALUES('upgrade alias','upgrade-model-v1',1760000015,1760000016);
      INSERT INTO saved_filters VALUES(1,'Upgrade filter','{"favorite":true}',1760000017,1760000018);
      INSERT INTO pending_relations VALUES('il-upgrade-smoke','unavailable-parent','unavailable-fingerprint','reference','Preserve unresolved parent',1760000019);
      UPDATE vision_settings SET base_url='https://example.test/v1',model='upgrade vision',updated_at=1760000020 WHERE id=1;
      INSERT INTO image_prompt_analyses VALUES(1,1,'openai-compatible','upgrade vision','Preserve analysis','Preserve analyzed prompt','{"subject":"Upgrade subject"}',1760000021);
      INSERT INTO remix_drafts VALUES(1,1,'Preserve remix',1760000022,1760000023);
      INSERT INTO remix_sources VALUES(1,2,'["subject","style"]','https://example.test/reference.png','{"pageTitle":"Upgrade reference"}',0,1760000024);
      INSERT INTO reference_sources VALUES(1,1,'https://example.test/reference.png','https://example.test/page','Upgrade page','web','{"note":"Preserve reference"}',1760000025,1760000026,1760000027);
    """)
    con.execute("INSERT INTO source_folders VALUES(?,?,?,?,?,?,?)", (1,str(image.parent.resolve()),"Upgrade source",0,1760000028,1760000029,1760000030))
    con.execute("INSERT INTO semantic_embeddings VALUES(?,?,?,?,?,?)", (1,"clip-vit-b32-qdrant-v1",512,struct.pack("<512f",1.0,*([0.0]*511)),hashlib.sha256(png).hexdigest(),1760000031))
    orders.update({"relations":"id","visual_dna":"asset_id","generation_index":"asset_id","semantic_embeddings":"asset_id","semantic_settings":"key","generation_sessions":"id","asset_sessions":"asset_id","model_aliases":"alias","saved_filters":"id","pending_relations":"child_portable_id,parent_portable_id,parent_fingerprint,relation_type","source_folders":"id","vision_settings":"id","image_prompt_analyses":"id","remix_drafts":"id","remix_sources":"draft_id,asset_id","reference_sources":"id"})
con.commit()
if con.execute("PRAGMA integrity_check").fetchall() != [("ok",)] or con.execute("PRAGMA foreign_key_check").fetchall():
    raise SystemExit("baseline fixture is not intact")
def encoded(row):
    return [{"blob_hex": value.hex()} if isinstance(value, bytes) else value for value in row]
snapshot = {table: [encoded(row) for row in con.execute(f"SELECT * FROM {table} ORDER BY {order}")] for table, order in orders.items()}
expected.write_text(json.dumps(snapshot), encoding="utf-8")
print(f"Pinned schema {version[0]} fixture: {len(snapshot)} populated business/settings tables")
con.close()
'@
$seedDb | python - $PreviousSchema $baselineDb $sentinelImage $expectedSnapshot $expectedBaselineSchema
Assert-True ($LASTEXITCODE -eq 0) "Failed to seed baseline business sentinels"
Assert-True (Test-Path -LiteralPath $baselineDb) "Failed to seed baseline database"

New-Item -ItemType Directory -Path (Join-Path $baselineDataRoot "backups") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $baselineDataRoot "models") -Force | Out-Null
$legacyBackup = Join-Path $baselineDataRoot "backups\upgrade-sentinel.sqlite3"
$legacyModel = Join-Path $baselineDataRoot "models\upgrade-sentinel.bin"
Copy-Item -LiteralPath $baselineDb -Destination $legacyBackup
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
$installedExecutableHash = (Get-FileHash -LiteralPath $currentExe -Algorithm SHA256).Hash
Write-Host "Upgrade application binding: built-SHA256=$expectedExecutableHash, installed-SHA256=$installedExecutableHash"
Assert-True ($installedExecutableHash -eq $expectedExecutableHash) "Installed executable bytes differ from the built release application"
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
import hashlib, json, pathlib, sqlite3, sys
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
orders.update({"relations":"id","visual_dna":"asset_id","generation_index":"asset_id","semantic_embeddings":"asset_id","semantic_settings":"key","generation_sessions":"id","asset_sessions":"asset_id","model_aliases":"alias","saved_filters":"id","pending_relations":"child_portable_id,parent_portable_id,parent_fingerprint,relation_type","source_folders":"id","vision_settings":"id","image_prompt_analyses":"id","remix_drafts":"id","remix_sources":"draft_id,asset_id","reference_sources":"id"})
def encoded(row):
    return [{"blob_hex": value.hex()} if isinstance(value, bytes) else value for value in row]
if not expected or not set(expected).issubset(orders):
    raise SystemExit("unexpected baseline snapshot table list")
for table in expected:
    rows = [encoded(row) for row in con.execute(f"SELECT * FROM {table} ORDER BY {orders[table]}")]
    if rows != expected[table]:
        raise SystemExit(f"business sentinel changed in {table}: {rows!r}")
asset_columns = [row[1] for row in con.execute("PRAGMA table_info(assets)")]
for row in expected["assets"]:
    asset = dict(zip(asset_columns, row))
    image = pathlib.Path(asset["path"])
    if not image.is_file() or image.stat().st_size != asset["file_size"]:
        raise SystemExit("original image file is missing or its size changed")
    with image.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != asset["fingerprint"]:
            raise SystemExit("original image file bytes changed")
con.close()
'@

foreach ($label in @("first upgraded launch", "upgraded restart")) {
  Invoke-AppStartupAndClose $currentExe $logPath $label
  Assert-True ((Get-FileHash -LiteralPath $currentExe -Algorithm SHA256).Hash -eq $expectedExecutableHash) "$label changed the installed application bytes"
  Write-Host "$label installed application: SHA256=$expectedExecutableHash, matches-built=true"
  Assert-True (Test-Path -LiteralPath $newDb) "$label data-root database is missing"
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
} finally { $builtExecutable.Dispose() }
