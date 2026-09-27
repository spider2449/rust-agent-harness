[CmdletBinding()]
param(
    [string]$TargetDirectory,
    [string]$OutputDirectory,
    [ValidateRange(1, 1440)][int]$WatchdogMinutes = 10,
    [switch]$PrepareOnly,
    [switch]$SelfTestFailure
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-LoggedProcess {
    param([string]$Program, [string[]]$Arguments, [string]$Prefix, [int]$TimeoutMinutes)
    $out = "$Prefix.stdout.log"
    $err = "$Prefix.stderr.log"
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Program
    $info.Arguments = $Arguments -join ' '
    $info.WorkingDirectory = $repoRoot
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $stdoutFile = [IO.File]::Create($out)
    $stderrFile = [IO.File]::Create($err)
    try {
        if (-not $process.Start()) { throw "could not start $Program" }
        $stdoutCopy = $process.StandardOutput.BaseStream.CopyToAsync($stdoutFile)
        $stderrCopy = $process.StandardError.BaseStream.CopyToAsync($stderrFile)
        if (-not $process.WaitForExit($TimeoutMinutes * 60000)) {
            try {
                $all = @(Get-CimInstance Win32_Process)
                $children = @($process.Id)
                do {
                    $before = $children.Count
                    foreach ($item in $all) {
                        if ($children -contains [int]$item.ParentProcessId -and $children -notcontains [int]$item.ProcessId) {
                            $children += [int]$item.ProcessId
                        }
                    }
                } while ($children.Count -ne $before)
                $names = @('cargo.exe', 'rah-desktop.exe', 'git.exe', 'where.exe', 'rah-mcp-echo-server.exe', 'rah-plugin-echo.exe')
                $all | Where-Object { $children -contains [int]$_.ProcessId -or $names -contains $_.Name } |
                    Select-Object ProcessId, ParentProcessId, Name, @{Name='InHarnessTree';Expression={ $children -contains [int]$_.ProcessId }} |
                    ConvertTo-Json -Depth 3 | Set-Content -LiteralPath "$Prefix.processes.json" -Encoding UTF8
            } catch {
                $_ | Out-String | Set-Content -LiteralPath "$Prefix.process-snapshot-error.log"
            } finally {
                # taskkill's /T follows descendants of this exact Cargo PID. Never kill by name.
                & taskkill.exe /PID $process.Id /T /F *> "$Prefix.taskkill.log"
                $null = $process.WaitForExit(30000)
            }
            return @{ ExitCode = $null; TimedOut = $true }
        }
        $process.WaitForExit()
        $null = $stdoutCopy.GetAwaiter().GetResult()
        $null = $stderrCopy.GetAwaiter().GetResult()
        return @{ ExitCode = $process.ExitCode; TimedOut = $false }
    } finally {
        $stdoutFile.Dispose()
        $stderrFile.Dispose()
        $process.Dispose()
    }
}

$start = [DateTimeOffset]::Now
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ([string]::IsNullOrWhiteSpace($TargetDirectory)) { $TargetDirectory = Join-Path $repoRoot 'target\windows-desktop-gate' }
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Join-Path ([IO.Path]::GetTempPath()) 'rah-windows-desktop-gate' }
$target = [IO.Path]::GetFullPath($TargetDirectory)
$output = [IO.Path]::GetFullPath($OutputDirectory)
$runDirectory = Join-Path $output ("{0}-{1}" -f $start.ToString('yyyyMMdd-HHmmss-fff'), [Guid]::NewGuid().ToString('N'))
$oldCargoTarget = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')
$oldTestTarget = [Environment]::GetEnvironmentVariable('RAH_TEST_TARGET_DIR', 'Process')
$result = 'HARNESS_ERROR'
$buildExit = $null
$testExit = $null
$timedOut = $false
$exitCode = 70

try {
    New-Item -ItemType Directory -Path $target -Force | Out-Null
    New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
    $env:CARGO_TARGET_DIR = $target
    $env:RAH_TEST_TARGET_DIR = $target
    $cargo = (Get-Command cargo.exe -ErrorAction Stop).Source

    if ($SelfTestFailure) {
        $shell = (Get-Process -Id $PID).Path
        $test = Invoke-LoggedProcess -Program $shell -Arguments @('-NoProfile', '-NonInteractive', '-Command', 'exit 23') -Prefix (Join-Path $runDirectory 'self-test') -TimeoutMinutes $WatchdogMinutes
        if ($test -isnot [hashtable]) { throw "unexpected launcher value: $($test | ConvertTo-Json -Depth 3 -Compress)" }
        $testExit = $test.ExitCode
        if ($test.TimedOut) { $result = 'WATCHDOG_TIMEOUT'; $timedOut = $true; $exitCode = 124 }
        elseif ($testExit -eq 23) { $result = 'TEST_FAILURE'; $exitCode = 23 }
        else { throw "self-test returned unexpected result: $($test | ConvertTo-Json -Depth 3 -Compress)" }
    } else {
        $build = Invoke-LoggedProcess -Program $cargo -Arguments @('build', '-p', 'rah-tools-mcp', '--bin', 'rah-mcp-echo-server', '-p', 'rah-tools-plugin', '--bin', 'rah-plugin-echo') -Prefix (Join-Path $runDirectory 'helpers') -TimeoutMinutes $WatchdogMinutes
        $buildExit = $build.ExitCode
        if ($build.TimedOut) { $result = 'WATCHDOG_TIMEOUT'; $timedOut = $true; $exitCode = 124 }
        elseif ($buildExit -ne 0) { $result = 'HELPER_BUILD_FAILURE'; $exitCode = $buildExit }
        else {
            $missing = @(@('rah-mcp-echo-server.exe', 'rah-plugin-echo.exe') | Where-Object { -not (Test-Path -LiteralPath (Join-Path $target "debug\$_") -PathType Leaf) })
            if ($missing.Count -gt 0) {
                $result = 'HELPER_MISSING'; $exitCode = 71
                "Missing: $($missing -join ', ')" | Set-Content -LiteralPath (Join-Path $runDirectory 'missing-helpers.txt')
            } elseif ($PrepareOnly) {
                $result = 'PASS'; $exitCode = 0
            } else {
                $test = Invoke-LoggedProcess -Program $cargo -Arguments @('test', '-p', 'rah-desktop', '--bin', 'rah-desktop', '--', '--nocapture') -Prefix (Join-Path $runDirectory 'desktop') -TimeoutMinutes $WatchdogMinutes
                $testExit = $test.ExitCode
                if ($test.TimedOut) { $result = 'WATCHDOG_TIMEOUT'; $timedOut = $true; $exitCode = 124 }
                elseif ($testExit -ne 0) { $result = 'TEST_FAILURE'; $exitCode = $testExit }
                else { $result = 'PASS'; $exitCode = 0 }
            }
        }
    }
} catch {
    if (-not (Test-Path -LiteralPath $runDirectory)) { New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null }
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $runDirectory 'harness-error.log')
    $result = 'HARNESS_ERROR'; $exitCode = 70
} finally {
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $oldCargoTarget, 'Process')
    [Environment]::SetEnvironmentVariable('RAH_TEST_TARGET_DIR', $oldTestTarget, 'Process')
    $end = [DateTimeOffset]::Now
    [ordered]@{
        result = $result; startTime = $start.ToString('o'); endTime = $end.ToString('o')
        elapsedSeconds = [math]::Round(($end - $start).TotalSeconds, 3)
        repositoryRoot = $repoRoot; targetDirectory = $target; evidenceDirectory = $runDirectory
        helperBuildExitStatus = $buildExit; desktopTestExitStatus = $testExit
        watchdogTimeout = $timedOut; watchdogMinutes = $WatchdogMinutes
        prepareOnly = [bool]$PrepareOnly; selfTestFailure = [bool]$SelfTestFailure
    } | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $runDirectory 'status.json') -Encoding UTF8
    Write-Host "$result (exit $exitCode): $runDirectory"
}
exit $exitCode
