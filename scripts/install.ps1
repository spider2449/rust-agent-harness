<#
.SYNOPSIS
Installs the `rah` binary from this repository into cargo's bin directory.

.DESCRIPTION
Runs `cargo install --path crates/rah-cli --locked`. Run it again after
updating the repository: the Web UI is compiled into the binary, so an old
install keeps serving the old UI. Afterwards `rah --web` works from any
directory (the file tools then use that directory as their root).

.EXAMPLE
.\scripts\install.ps1

.EXAMPLE
.\scripts\install.ps1 -Uninstall
#>
[CmdletBinding()]
param(
    # Remove the installed binary instead.
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

if ($Uninstall) {
    & cargo uninstall rah-cli
    exit $LASTEXITCODE
}

& cargo install --path (Join-Path $repo "crates/rah-cli") --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$rah = Get-Command rah -ErrorAction SilentlyContinue
if ($rah) {
    Write-Host "Installed $(& rah --version) at $($rah.Source)"
} else {
    Write-Warning "rah is installed but not on PATH; add cargo's bin directory (usually $HOME\.cargo\bin) to PATH."
}
