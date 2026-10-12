<#
.SYNOPSIS
Runs the repository's validation: format, check, tests, clippy, whitespace.

.DESCRIPTION
The same commands as AGENTS.md and CI. The Web UI browser tests run when a
Chromium-based browser is found (Edge or Chrome) or named with -Browser;
otherwise they are skipped. Every step runs; the script reports each result
and exits non-zero if any failed.

.EXAMPLE
.\scripts\check.ps1

.EXAMPLE
.\scripts\check.ps1 -Quick
Format and compile only, without tests or clippy.
#>
[CmdletBinding()]
param(
    # Only `cargo fmt --check` and `cargo check`.
    [switch]$Quick,
    # Path of a Chromium-based browser for the Web UI tests.
    [string]$Browser,
    # Skip the browser tests even if a browser is found.
    [switch]$NoBrowser,
    # Apply `cargo fmt` instead of only checking it.
    [switch]$Fix
)

$ErrorActionPreference = "Continue"
$repo = Split-Path -Parent $PSScriptRoot

if (-not $NoBrowser -and -not $Browser -and -not $env:RAH_TEST_BROWSER) {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe"
    )
    $Browser = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
}
if ($NoBrowser) {
    Remove-Item Env:RAH_TEST_BROWSER -ErrorAction SilentlyContinue
} elseif ($Browser) {
    $env:RAH_TEST_BROWSER = $Browser
}

$steps = @()
if ($Fix) {
    $steps += , @("cargo fmt", @("cargo", "fmt"))
} else {
    $steps += , @("cargo fmt --check", @("cargo", "fmt", "--check"))
}
$steps += , @("cargo check", @("cargo", "check", "--workspace"))
if (-not $Quick) {
    $steps += , @("cargo test", @("cargo", "test", "--workspace", "--no-fail-fast"))
    $steps += , @("cargo clippy", @("cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"))
    $steps += , @("git diff --check", @("git", "diff", "--check"))
}

$results = @()
Push-Location $repo
try {
    if ($env:RAH_TEST_BROWSER) {
        Write-Host "Browser tests use $env:RAH_TEST_BROWSER"
    } elseif (-not $Quick) {
        Write-Host "No browser found; the Web UI browser tests are skipped."
    }
    foreach ($step in $steps) {
        $name = $step[0]
        $command = $step[1]
        Write-Host ""
        Write-Host "==> $name" -ForegroundColor Cyan
        $program = $command[0]
        $arguments = @($command | Select-Object -Skip 1)
        & $program @arguments
        $results += [pscustomobject]@{ Step = $name; Ok = ($LASTEXITCODE -eq 0) }
    }
} finally {
    Pop-Location
}

Write-Host ""
foreach ($result in $results) {
    if ($result.Ok) {
        Write-Host "  ok      $($result.Step)" -ForegroundColor Green
    } else {
        Write-Host "  FAILED  $($result.Step)" -ForegroundColor Red
    }
}
if ($results | Where-Object { -not $_.Ok }) { exit 1 }
exit 0
