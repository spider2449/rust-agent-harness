<#
.SYNOPSIS
Builds RAH from this checkout and installs it: the one command to get the
current version onto this machine.

.DESCRIPTION
Builds `rah` and `rah-desktop` (release), then copies them, the Web launcher,
the icon and the README into the install directory, the same one the
installer uses (`%LOCALAPPDATA%\Programs\RAH`). An existing install, its
Start Menu entries and its PATH entry keep working and now run the new
version. On a machine without RAH it also adds the Start Menu entries and
puts the directory on the user's PATH.

RAH may keep running while it builds. Before copying, a running `rah` or
`rah-desktop` from the install directory is closed after asking (-Force:
without asking), and RAH Desktop is opened again afterwards.

Your data (`~\.rah`: rah.toml, sessions, settings) is never touched.

When `target` has grown past -MaxTargetGB, the debug build cache is removed
afterwards (`cargo clean --profile dev`); the release build is kept, so the
next update is quick.

.EXAMPLE
.\scripts\update.ps1
#>
[CmdletBinding()]
param(
    # Where RAH is installed.
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\RAH"),
    # Remove the debug build cache when target is larger than this.
    [int]$MaxTargetGB = 30,
    # Do not create Start Menu entries or change PATH on a new install.
    [switch]$NoShortcuts,
    # Close a running RAH without asking.
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH; install Rust from https://rustup.rs"
}

$manifest = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "No workspace version in Cargo.toml." }
$newVersion = $Matches[1]

$installedRah = Join-Path $InstallDir "rah.exe"
$oldVersion = $null
if (Test-Path -LiteralPath $installedRah) {
    $oldVersion = ((& $installedRah --version) -replace '^rah\s+', '').Trim()
}
$fresh = -not (Test-Path -LiteralPath $InstallDir)

Write-Host "Building RAH $newVersion (release)..."
Push-Location $repo
try {
    # cargo reports progress on stderr, which Windows PowerShell turns into
    # error records; the exit code decides.
    $ErrorActionPreference = "Continue"
    & cargo build --release --locked -p rah-cli -p rah-desktop
    $code = $LASTEXITCODE
    $ErrorActionPreference = "Stop"
    if ($code -ne 0) { exit $code }
} finally {
    Pop-Location
}

# A running copy keeps its files locked. RAH may keep running during the
# build; only now is it closed, after asking (or at once with -Force).
$running = @(Get-Process -Name rah, rah-desktop -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($InstallDir, [StringComparison]::OrdinalIgnoreCase) })
$reopen = $false
if ($running.Count -gt 0) {
    $names = ($running | ForEach-Object { "$($_.Name) (pid $($_.Id))" }) -join ", "
    if (-not $Force) {
        Write-Host "RAH is running from ${InstallDir}: $names"
        $answer = Read-Host "Close it to finish the update? A running turn is cancelled. [Y/n]"
        if ($answer -and $answer -notmatch '^(y|yes)$') {
            throw "Not updated. Close RAH and run this again; the build is done, so it is quick."
        }
    }
    $reopen = [bool]($running | Where-Object Name -eq "rah-desktop")
    foreach ($process in $running) {
        # The desktop window shuts its plugins down when asked to close.
        if (-not ($process.MainWindowHandle -ne 0 -and $process.CloseMainWindow())) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    foreach ($process in $running) {
        if (-not $process.WaitForExit(10000)) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            $null = $process.WaitForExit(5000)
        }
    }
}

$release = Join-Path $repo "target\release"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$files = @(
    (Join-Path $release "rah.exe"),
    (Join-Path $release "rah-desktop.exe"),
    (Join-Path $repo "packaging\windows\rah-web.ps1"),
    (Join-Path $repo "packaging\windows\rah.ico"),
    (Join-Path $repo "README.md")
)
Copy-Item -LiteralPath $files -Destination $InstallDir -Force

if ($fresh -and -not $NoShortcuts) {
    # What the installer adds: Start Menu entries and a user PATH entry.
    $menuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "RAH"
    New-Item -ItemType Directory -Force -Path $menuDir | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    $web = $shell.CreateShortcut((Join-Path $menuDir "RAH Web.lnk"))
    $web.TargetPath = "powershell.exe"
    $web.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $InstallDir 'rah-web.ps1')`""
    $web.WorkingDirectory = $env:USERPROFILE
    $web.IconLocation = Join-Path $InstallDir "rah.ico"
    $web.Description = "RAH Web UI in the default browser"
    $web.Save()
    $desktop = $shell.CreateShortcut((Join-Path $menuDir "RAH Desktop.lnk"))
    $desktop.TargetPath = Join-Path $InstallDir "rah-desktop.exe"
    $desktop.WorkingDirectory = $env:USERPROFILE
    $desktop.IconLocation = Join-Path $InstallDir "rah.ico"
    $desktop.Description = "RAH Web UI in a native window"
    $desktop.Save()
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $entries = @($userPath -split ";" | Where-Object { $_ })
    if (-not ($entries | Where-Object { $_.TrimEnd("\") -ieq $InstallDir.TrimEnd("\") })) {
        [Environment]::SetEnvironmentVariable("Path", (($entries + $InstallDir) -join ";"), "User")
        Write-Host "Added $InstallDir to your PATH; open a new terminal to use it."
    }
}

$installed = ((& $installedRah --version) -replace '^rah\s+', '').Trim()
if ($oldVersion -and $oldVersion -ne $installed) {
    Write-Host "RAH updated: $oldVersion -> $installed ($InstallDir)"
} elseif ($oldVersion) {
    Write-Host "RAH $installed reinstalled ($InstallDir)"
} else {
    Write-Host "RAH $installed installed ($InstallDir)"
}
if ($reopen) {
    Start-Process -FilePath (Join-Path $InstallDir "rah-desktop.exe") -WorkingDirectory $env:USERPROFILE
    Write-Host "Reopened RAH Desktop."
}

# Another `rah` earlier on PATH would still run instead of this one.
$found = Get-Command rah -All -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
$first = $found | Select-Object -First 1
if ($first -and -not $first.StartsWith($InstallDir, [StringComparison]::OrdinalIgnoreCase)) {
    Write-Warning "``rah`` in a terminal runs $first, not this install. Remove it (for a cargo install: cargo uninstall rah-cli rah-desktop)."
}

# The build cache only grows; trim it when it is large.
$target = Join-Path $repo "target"
$bytes = (Get-ChildItem -LiteralPath $target -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
if ($bytes -gt [int64]$MaxTargetGB * 1GB) {
    Write-Host ("target is {0:N0} GB; removing the debug build cache..." -f ($bytes / 1GB))
    Push-Location $repo
    try {
        $ErrorActionPreference = "Continue"
        & cargo clean --profile dev
        $ErrorActionPreference = "Stop"
    } finally {
        Pop-Location
    }
}
