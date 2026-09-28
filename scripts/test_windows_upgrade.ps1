param(
  [Parameter(Mandatory=$true)][string]$PreviousInstaller,
  [Parameter(Mandatory=$true)][string]$CurrentInstaller,
  [Parameter(Mandatory=$true)][string]$PreviousSchema,
  [string]$PreviousVersion = "0.19.0",
  [string]$CurrentVersion = "0.20.0"
)

$ErrorActionPreference = "Stop"

function Assert-True([bool]$Condition, [string]$Message) {
  if (-not $Condition) { throw $Message }
}

function Unquote([string]$Value) {
  if ($null -eq $Value) { return "" }
  return $Value.Trim().Trim('"')
}

function Invoke-Installer([string]$Path) {
  Assert-True (Test-Path $Path) "Installer not found: $Path"
  $process = Start-Process -FilePath $Path -ArgumentList "/S" -Wait -PassThru
  Assert-True ($process.ExitCode -eq 0) "Installer failed with exit code $($process.ExitCode): $Path"
}

$uninstallRoot = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall"
$uninstallKey = Join-Path $uninstallRoot "ImageLore"
$legacyRoot = Join-Path $env:LOCALAPPDATA "ImageLore"
$dataRoot = Join-Path $env:LOCALAPPDATA "app.imagelore.desktop"
$startMenuLink = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\ImageLore\ImageLore.lnk"

# The hosted runner should be clean, but make the smoke test repeatable.
if (Test-Path $uninstallKey) {
  $existing = Get-ItemProperty $uninstallKey
  $uninstaller = Unquote $existing.UninstallString
  if ($uninstaller -and (Test-Path $uninstaller)) {
    $p = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
    if ($p.ExitCode -ne 0) { Write-Warning "Pre-test uninstall returned $($p.ExitCode)" }
  }
}
Remove-Item $legacyRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $dataRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $startMenuLink -Force -ErrorAction SilentlyContinue

Write-Host "Installing upgrade baseline $PreviousVersion..."
Invoke-Installer $PreviousInstaller

Assert-True (Test-Path $uninstallKey) "Baseline installer did not register ImageLore"
$oldReg = Get-ItemProperty $uninstallKey
Assert-True ($oldReg.DisplayVersion -eq $PreviousVersion) "Expected baseline DisplayVersion $PreviousVersion, got $($oldReg.DisplayVersion)"
$oldInstallDir = Unquote $oldReg.InstallLocation
Assert-True ($oldInstallDir -eq $legacyRoot) "Baseline install location changed: $oldInstallDir"
Assert-True (Test-Path (Join-Path $oldInstallDir "ImageLore.exe")) "Baseline ImageLore.exe missing"
Assert-True (Test-Path (Join-Path $oldInstallDir "uninstall.exe")) "Baseline uninstaller missing"

# Seed a real v0.19-compatible database plus persistent folders in the legacy
# install/data directory. v0.20 must preserve and migrate them on first launch.
$legacyDb = Join-Path $legacyRoot "library.sqlite3"
$seedDb = @'
import pathlib, sqlite3, sys
schema = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
db = pathlib.Path(sys.argv[2])
if db.exists():
    db.unlink()
con = sqlite3.connect(db)
con.executescript(schema)
con.execute(
    "INSERT INTO assets(path,name,portable_id,created_at,updated_at) VALUES(?,?,?,?,?)",
    ("C:/upgrade-sentinel.png", "Upgrade Sentinel", "il-upgrade-smoke", 1760000000, 1760000000),
)
con.commit()
con.close()
'@
$seedDb | python - $PreviousSchema $legacyDb
Assert-True (Test-Path $legacyDb) "Failed to seed legacy database"

New-Item -ItemType Directory -Path (Join-Path $legacyRoot "backups") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $legacyRoot "models") -Force | Out-Null
Set-Content -Path (Join-Path $legacyRoot "backups\upgrade-sentinel.txt") -Value "keep-backup" -NoNewline
Set-Content -Path (Join-Path $legacyRoot "models\upgrade-sentinel.bin") -Value "keep-model" -NoNewline

Write-Host "Installing current version $CurrentVersion over $PreviousVersion..."
Invoke-Installer $CurrentInstaller

Assert-True (Test-Path $uninstallKey) "Current installer did not preserve ImageLore registration"
$newReg = Get-ItemProperty $uninstallKey
Assert-True ($newReg.DisplayVersion -eq $CurrentVersion) "Expected DisplayVersion $CurrentVersion, got $($newReg.DisplayVersion)"
$newInstallDir = Unquote $newReg.InstallLocation
Assert-True ($newInstallDir -eq $oldInstallDir) "Upgrade installed side-by-side instead of replacing old location"
Assert-True (Test-Path (Join-Path $newInstallDir "ImageLore.exe")) "Upgraded ImageLore.exe missing"
Assert-True (Test-Path (Join-Path $newInstallDir "uninstall.exe")) "Upgraded uninstaller missing"

$entries = @(
  Get-ChildItem $uninstallRoot | ForEach-Object {
    try { Get-ItemProperty $_.PSPath } catch { $null }
  } | Where-Object { $_ -and $_.DisplayName -eq "ImageLore" }
)
Assert-True ($entries.Count -eq 1) "Expected exactly one ImageLore installed-app entry, found $($entries.Count)"

$exeVersion = (Get-Item (Join-Path $newInstallDir "ImageLore.exe")).VersionInfo.ProductVersion
Assert-True ($exeVersion -like "$CurrentVersion*") "Installed executable version is not ${CurrentVersion}: $exeVersion"

Assert-True (Test-Path $startMenuLink) "Start menu shortcut missing after upgrade"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($startMenuLink)
Assert-True (([IO.Path]::GetFullPath($shortcut.TargetPath)) -eq ([IO.Path]::GetFullPath((Join-Path $newInstallDir "ImageLore.exe")))) "Start menu shortcut points to the wrong executable"

# First launch performs the one-time persistent-data migration from the legacy
# install directory into the identifier-scoped data directory.
$app = Start-Process -FilePath (Join-Path $newInstallDir "ImageLore.exe") -PassThru
$newDb = Join-Path $dataRoot "library.sqlite3"
$deadline = (Get-Date).AddSeconds(20)
while ((Get-Date) -lt $deadline -and -not (Test-Path $newDb)) {
  Start-Sleep -Milliseconds 500
}
if (-not $app.HasExited) {
  Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
}
Assert-True (Test-Path $newDb) "v0.20 did not migrate the legacy database to the isolated data root"
Assert-True (-not (Test-Path $legacyDb)) "Legacy database remained in the install directory"
Assert-True (Test-Path (Join-Path $dataRoot "backups\upgrade-sentinel.txt")) "Backup directory was not preserved"
Assert-True (Test-Path (Join-Path $dataRoot "models\upgrade-sentinel.bin")) "Model directory was not preserved"

$verifyDb = @'
import sqlite3, sys
con = sqlite3.connect(sys.argv[1])
row = con.execute("SELECT name FROM assets WHERE portable_id='il-upgrade-smoke'").fetchone()
con.close()
if row != ("Upgrade Sentinel",):
    raise SystemExit(f"sentinel row missing after upgrade: {row!r}")
'@
$verifyDb | python - $newDb

Write-Host "ImageLore Windows upgrade smoke test: PASS ($PreviousVersion -> $CurrentVersion)"

# Clean up the hosted runner.
$currentUninstaller = Unquote (Get-ItemProperty $uninstallKey).UninstallString
if ($currentUninstaller -and (Test-Path $currentUninstaller)) {
  $p = Start-Process -FilePath $currentUninstaller -ArgumentList "/S" -Wait -PassThru
  if ($p.ExitCode -ne 0) { Write-Warning "Post-test uninstall returned $($p.ExitCode)" }
}
Remove-Item $legacyRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $dataRoot -Recurse -Force -ErrorAction SilentlyContinue
