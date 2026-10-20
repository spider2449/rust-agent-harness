<#
.SYNOPSIS
Installs RAH from this repository: `rah` (Web UI and terminal) and
`rah-desktop` (the Web UI in a native window).

.DESCRIPTION
Runs `cargo install --locked` for `crates/rah-cli` and `crates/rah-desktop`
into cargo's bin directory. Run it again after updating the repository: the
Web UI is compiled into both binaries, so an old install keeps serving the old
UI. Afterwards `rah --web` and `rah-desktop` work from any directory (the file
tools then use that directory as their root).

With -Shortcuts it also adds Start Menu entries "RAH Web" (starts the Web UI
and opens the browser) and "RAH Desktop". The Web launcher is a small script
copied to the data directory (`~\.rah\bin`).

.EXAMPLE
.\scripts\install.ps1
Installs both.

.EXAMPLE
.\scripts\install.ps1 -Only Web -Shortcuts
Installs `rah` only, with a Start Menu entry.

.EXAMPLE
.\scripts\install.ps1 -Uninstall
Removes the binaries and the Start Menu entries; the data directory stays.
#>
[CmdletBinding()]
param(
    # Install only one of the two (default: both).
    [ValidateSet("Both", "Web", "Desktop")]
    [string]$Only = "Both",
    # Add Start Menu shortcuts.
    [switch]$Shortcuts,
    # Remove what this script installed instead.
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

$dataHome = if ($env:RAH_HOME) { $env:RAH_HOME } else { Join-Path $env:USERPROFILE ".rah" }
$launcherDir = Join-Path $dataHome "bin"
$launcher = Join-Path $launcherDir "rah-web.ps1"
$menuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "RAH"

# `rah` serves the Web UI (and the REPL); `rah-desktop` shows it in a window.
$packages = @()
if ($Only -ne "Desktop") { $packages += "rah-cli" }
if ($Only -ne "Web") { $packages += "rah-desktop" }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH; install Rust from https://rustup.rs"
}

if ($Uninstall) {
    # Remove every known package regardless of -Only; cargo reports absent ones.
    foreach ($package in "rah-cli", "rah-desktop") {
        & cargo uninstall $package
    }
    Remove-Item -LiteralPath $menuDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $launcher -Force -ErrorAction SilentlyContinue
    exit 0
}

foreach ($package in $packages) {
    & cargo install --path (Join-Path $repo "crates/$package") --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$cargoBin = if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME "bin" } else { Join-Path $env:USERPROFILE ".cargo\bin" }

if ($Only -ne "Desktop") {
    $rah = Get-Command rah -ErrorAction SilentlyContinue
    if ($rah) {
        Write-Host "Installed $(& rah --version) at $($rah.Source)"
    } else {
        Write-Warning "rah is installed but not on PATH; add $cargoBin to PATH."
    }
}

if ($Only -ne "Web") {
    $desktop = Get-Command rah-desktop -ErrorAction SilentlyContinue
    if ($desktop) {
        Write-Host "Installed rah-desktop at $($desktop.Source)"
    } else {
        Write-Warning "rah-desktop is installed but not on PATH; add $cargoBin to PATH."
    }
    # The window is a WebView2; Windows 10 may not ship the runtime.
    $webview2 = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    $found = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webview2",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$webview2",
        "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$webview2"
    ) | Where-Object { Test-Path -LiteralPath $_ }
    if (-not $found) {
        Write-Warning "The WebView2 runtime was not found; rah-desktop needs it. Install it from https://developer.microsoft.com/microsoft-edge/webview2/"
    }
}

if ($Shortcuts) {
    New-Item -ItemType Directory -Force -Path $menuDir | Out-Null
    $shell = New-Object -ComObject WScript.Shell

    if ($Only -ne "Desktop") {
        New-Item -ItemType Directory -Force -Path $launcherDir | Out-Null
        # Starts the Web UI and opens the first URL that carries the access
        # token; the console window stays open as the server's log.
        @'
$ErrorActionPreference = "Continue"
$opened = $false
& rah --web @args 2>&1 | ForEach-Object {
    $line = "$_"
    Write-Host $line
    if (-not $opened -and $line -match '(http://\S+/#token=[0-9a-f]+)') {
        $opened = $true
        Start-Process $Matches[1]
    }
}
'@ | Set-Content -LiteralPath $launcher -Encoding UTF8

        $link = $shell.CreateShortcut((Join-Path $menuDir "RAH Web.lnk"))
        $link.TargetPath = "powershell.exe"
        $link.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$launcher`""
        $link.WorkingDirectory = $env:USERPROFILE
        $link.Description = "RAH Web UI in the default browser"
        $link.Save()
    }

    if ($Only -ne "Web") {
        $link = $shell.CreateShortcut((Join-Path $menuDir "RAH Desktop.lnk"))
        $link.TargetPath = Join-Path $cargoBin "rah-desktop.exe"
        $link.WorkingDirectory = $env:USERPROFILE
        $link.Description = "RAH Web UI in a native window"
        $link.Save()
    }
    Write-Host "Start Menu entries added under $menuDir"
}

Write-Host "Run: rah --web   |   rah-desktop"
