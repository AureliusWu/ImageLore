# Produces a library-test executable, never an application or installer release.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# No local data directories, desktop programs or signing credentials are used.
# Match the disposable-runner boundary already used by test_windows_upgrade.ps1.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Optimized candidate build requires a disposable GitHub-hosted Windows runner.'
}
if ($env:GITHUB_REPOSITORY -ne 'AureliusWu/ImageLore') {
    throw 'Unexpected candidate repository.'
}
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ([IO.Path]::GetFullPath($env:GITHUB_WORKSPACE) -ne $project) {
    throw 'Candidate checkout must be the current GitHub workspace.'
}
$head = (& git -C $project rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $head -notmatch '^[0-9a-f]{40}$' -or $head -ne $env:GITHUB_SHA) {
    throw 'Candidate checkout does not match the workflow SHA.'
}
if (@(& git -C $project status --porcelain --untracked-files=no).Count -ne 0) {
    throw 'Candidate checkout is already modified.'
}
$target = Join-Path $project 'src-tauri/target'
$output = Join-Path $target 'optimized-candidate-evidence'
if (Test-Path -LiteralPath $output) {
    throw 'Candidate evidence directory must be fresh.'
}
$null = New-Item -ItemType Directory -Path $output
$env:CARGO_TARGET_DIR = $target
$children = [Collections.Generic.List[object]]::new()
$record = [ordered]@{
    state = 'STARTED_NOT_A_RESULT'
    head = $head
    runId = $env:GITHUB_RUN_ID
    runAttempt = $env:GITHUB_RUN_ATTEMPT
    startedUtc = [DateTime]::UtcNow.ToString('o')
    scope = 'Optimized Rust library only; no explicit ignored profiles, desktop, signing, installer, updater or release acceptance.'
    binaryExported = $false
}

function Write-JsonFile([string]$Name, $Value) {
    [IO.File]::WriteAllText((Join-Path $output $Name), ($Value | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
}

function Source-Manifest {
    $paths = @(& git -C $project -c core.quotePath=false ls-files)
    if ($LASTEXITCODE -ne 0 -or $paths.Count -eq 0) { throw 'Cannot inventory tracked candidate inputs.' }
    @($paths | Sort-Object | ForEach-Object {
        $file = Join-Path $project $_
        $item = Get-Item -LiteralPath $file
        if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'Tracked source must be a regular file.'
        }
        [ordered]@{path = $_; bytes = $item.Length; sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
}

function Invoke-Owned([string]$Name, [string]$Program, [string[]]$Arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Program
    $info.WorkingDirectory = $project
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $started = [DateTime]::UtcNow.ToString('o')
    if (-not $process.Start()) { throw "Cannot start candidate step $Name." }
    $pidValue = $process.Id
    $created = $process.StartTime.ToUniversalTime().ToString('o')
    Write-JsonFile "$Name-started.json" ([ordered]@{state = 'CHILD_STARTED_NOT_A_RESULT'; pid = $pidValue; processStartTimeUtc = $created; startedUtc = $started; program = $Program; arguments = $Arguments})
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    while (-not $process.WaitForExit(30000)) {
        Write-Host "Candidate step $Name is still running (owned process $pidValue)."
    }
    $exitCode = $process.ExitCode
    [IO.File]::WriteAllText((Join-Path $output "$Name.stdout.log"), $stdout.GetAwaiter().GetResult(), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $output "$Name.stderr.log"), $stderr.GetAwaiter().GetResult(), [Text.UTF8Encoding]::new($false))
    $receipt = [ordered]@{
        name = $Name; pid = $pidValue; processStartTimeUtc = $created
        program = $Program; arguments = $Arguments; exitCode = $exitCode
        startedUtc = $started; finishedUtc = [DateTime]::UtcNow.ToString('o')
        stdoutSha256 = (Get-FileHash -LiteralPath (Join-Path $output "$Name.stdout.log") -Algorithm SHA256).Hash.ToLowerInvariant()
        stderrSha256 = (Get-FileHash -LiteralPath (Join-Path $output "$Name.stderr.log") -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    $process.Dispose()
    Write-JsonFile "$Name-terminal.json" $receipt
    $children.Add($receipt)
    Write-Host "Candidate step $Name exited $exitCode."
    if ($exitCode -ne 0) { throw "Candidate step $Name failed; original logs retained." }
}

Write-JsonFile 'candidate-started.json' $record
try {
    $before = @(Source-Manifest)
    Write-JsonFile 'source-before.json' $before
    $record.sourceCount = $before.Count
    $record.sourceBeforeSha256 = (Get-FileHash -LiteralPath (Join-Path $output 'source-before.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    $record.rustc = (& rustc -vV) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify rustc.' }
    $record.cargo = (& cargo -V) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify Cargo.' }
    $manifest = 'src-tauri/Cargo.toml'
    Invoke-Owned 'fmt' 'cargo' @('fmt', '--all', '--manifest-path', $manifest, '--', '--check')
    Invoke-Owned 'check' 'cargo' @('check', '--release', '--locked', '--manifest-path', $manifest, '--lib', '--jobs', '1')
    Invoke-Owned 'clippy' 'cargo' @('clippy', '--release', '--locked', '--manifest-path', $manifest, '--lib', '--tests', '--jobs', '1', '--', '-D', 'warnings')
    # Clear only this package's disposable release outputs so a fresh test target
    # is observed, while retaining dependency caches and every earlier receipt.
    Invoke-Owned 'clean-package' 'cargo' @('clean', '--release', '--manifest-path', $manifest, '--package', 'imagelore')
    Invoke-Owned 'build' 'cargo' @('test', '--release', '--locked', '--manifest-path', $manifest, '--lib', '--no-run', '--message-format=json', '--jobs', '1')
    $messages = @(Get-Content -LiteralPath (Join-Path $output 'build.stdout.log') -Encoding UTF8 | ForEach-Object { $_ | ConvertFrom-Json })
    $artifacts = @($messages | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'imagelore_lib' -and $_.profile.test -and $_.executable })
    if ($artifacts.Count -ne 1) { throw 'Expected exactly one actual library-test artifact.' }
    $artifact = $artifacts[0]
    if ($artifact.fresh -or $artifact.profile.opt_level -ne '3' -or $artifact.profile.debug_assertions) {
        throw 'Cargo did not report a fresh optimized test artifact with debug assertions disabled.'
    }
    $builtPath = [IO.Path]::GetFullPath($artifact.executable)
    $releaseRoot = [IO.Path]::GetFullPath((Join-Path $target 'release')) + [IO.Path]::DirectorySeparatorChar
    if (-not $builtPath.StartsWith($releaseRoot, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetExtension($builtPath) -ne '.exe') {
        throw 'Unexpected optimized artifact path.'
    }
    $archive = Join-Path $output 'imagelore-lib-test.exe'
    Copy-Item -LiteralPath $builtPath -Destination $archive
    $record.profile = $artifact.profile
    $record.artifactFresh = $artifact.fresh
    $record.binarySha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $record.binaryBytes = (Get-Item -LiteralPath $archive).Length
    Write-JsonFile 'cargo-test-artifact.json' $artifact
    # Resolve the runtime from this build's actual ort-sys link search paths.
    # The static ONNX Runtime library is not an onnxruntime.dll dependency.
    $ortScripts = @($messages | Where-Object { $_.reason -eq 'build-script-executed' -and $_.package_id -match 'ort-sys[@ :]' })
    $runtimeDirs = @($ortScripts | ForEach-Object { $_.linked_paths } | ForEach-Object {
        $candidate = $_ -replace '^(native|dependency|crate|framework|all)=', ''
        if ((Test-Path -LiteralPath (Join-Path $candidate 'DirectML.dll')) -and (Test-Path -LiteralPath (Join-Path $candidate 'onnxruntime.lib'))) {
            [IO.Path]::GetFullPath($candidate)
        }
    } | Sort-Object -Unique)
    if ($runtimeDirs.Count -ne 1) { throw 'Expected one actual ort-sys static-library and DirectML runtime location.' }
    $record.nativeRuntime = [ordered]@{
        linkedDirectory = $runtimeDirs[0]
        staticLibrarySha256 = (Get-FileHash -LiteralPath (Join-Path $runtimeDirs[0] 'onnxruntime.lib') -Algorithm SHA256).Hash.ToLowerInvariant()
        directMlSha256 = (Get-FileHash -LiteralPath (Join-Path $runtimeDirs[0] 'DirectML.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
        redistributed = $false
    }
    $env:PATH = $runtimeDirs[0] + [IO.Path]::PathSeparator + $env:PATH
    Invoke-Owned 'list' $archive @('--list', '--format=terse')
    $listed = @([regex]::Matches([IO.File]::ReadAllText((Join-Path $output 'list.stdout.log')), '(?m)^(.+): test\r?$') | ForEach-Object { $_.Groups[1].Value })
    if ($listed.Count -eq 0 -or @($listed | Sort-Object -Unique).Count -ne $listed.Count) { throw 'Invalid actual executable test inventory.' }
    Write-JsonFile 'test-inventory.json' $listed
    $record.listedTests = $listed.Count
    Invoke-Owned 'full' $archive @('--nocapture', '--test-threads=1')
    $fullLog = [IO.File]::ReadAllText((Join-Path $output 'full.stdout.log'))
    $summaries = [regex]::Matches($fullLog, 'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in ([0-9.]+)s')
    if ($summaries.Count -ne 1) { throw 'Expected one unfiltered full-suite summary.' }
    $summary = $summaries[0]
    if ([int]$summary.Groups[1].Value -le 0 -or [int]$summary.Groups[2].Value -ne 0 -or [int]$summary.Groups[4].Value -ne 0 -or [int]$summary.Groups[5].Value -ne 0) {
        throw 'Full-suite summary does not prove an unfiltered successful run.'
    }
    $record.tests = [ordered]@{passed = [int]$summary.Groups[1].Value; failed = 0; ignored = [int]$summary.Groups[3].Value; measured = 0; filtered = 0; seconds = [double]::Parse($summary.Groups[6].Value, [Globalization.CultureInfo]::InvariantCulture)}
    if ($record.tests.passed + $record.tests.ignored -ne $listed.Count) { throw 'Full-suite count does not match the actual executable inventory.' }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $record.binarySha256) {
        throw 'Archived program changed during execution.'
    }
    $record.state = 'PASS_OPTIMIZED_LIBRARY_FULL_SUITE'
} catch {
    $record.state = 'FAILED_OPTIMIZED_LIBRARY_CANDIDATE'
    $record.error = $_.Exception.Message
} finally {
    try {
        $after = @(Source-Manifest)
        Write-JsonFile 'source-after.json' $after
        $record.sourceAfterSha256 = (Get-FileHash -LiteralPath (Join-Path $output 'source-after.json') -Algorithm SHA256).Hash.ToLowerInvariant()
        $record.sourceUnchanged = $record.sourceBeforeSha256 -eq $record.sourceAfterSha256
        $record.headAfter = (& git -C $project rev-parse HEAD).Trim()
        if (-not $record.sourceUnchanged -or $record.headAfter -ne $head) { throw 'Source identity changed during candidate gates.' }
        if (@(& git -C $project status --porcelain --untracked-files=no).Count -ne 0) { throw 'Candidate gates modified tracked source.' }
    } catch {
        $record.state = 'FAILED_OPTIMIZED_LIBRARY_CANDIDATE'
        $record.sourceError = $_.Exception.Message
    }
    $record.finishedUtc = [DateTime]::UtcNow.ToString('o')
    $record.children = @($children.ToArray())
    Write-JsonFile 'candidate-terminal.json' $record
}
if ($record.state -ne 'PASS_OPTIMIZED_LIBRARY_FULL_SUITE') { throw 'Optimized candidate failed; inspect the archived terminal and original logs.' }
Write-Host "PASS optimized library suite: $($record.tests.passed) passed / $($record.tests.ignored) ignored; no release claim."
