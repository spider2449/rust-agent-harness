<#
.SYNOPSIS
Builds the Windows installer `rah-<version>-setup.exe` (and a portable zip).

.DESCRIPTION
Builds `rah` and `rah-desktop` in release mode, stages them with the Web
launcher under `dist\stage`, zips the stage as `rah-<version>-portable.zip`,
and compiles `packaging\windows\rah.iss` with Inno Setup (ISCC.exe) into
`dist\rah-<version>-setup.exe`. The installer is per-user (no administrator
rights), adds Start Menu entries and optionally the PATH, and has an
uninstaller that keeps the data directory (`~\.rah`).

Inno Setup 6 is a build-time tool only: install it yourself
(https://jrsoftware.org/isinfo.php, or `winget install JRSoftware.InnoSetup`),
or name ISCC.exe with -Iscc. Without it the script still produces the zip.

.EXAMPLE
.\scripts\package.ps1

.EXAMPLE
.\scripts\package.ps1 -SkipBuild
Package the existing release build.

.EXAMPLE
.\scripts\package.ps1 -GraphifyRuntime '.rah\graphify-runtime'
Build a separate bundled package whose target computer needs no uv, Python or Node installation.
#>
[CmdletBinding()]
param(
    # Path of ISCC.exe when it is not found automatically.
    [string]$Iscc,
    # Reuse target\release instead of building.
    [switch]$SkipBuild,
    # Prepared relocatable Node/Python/Graphify bundle; target machines need no uv.
    [string]$GraphifyRuntime
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

$manifest = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "No workspace version in Cargo.toml." }
$version = $Matches[1]
$suffix = if ($GraphifyRuntime) { '-graphify-bundled' } else { '' }
if ($GraphifyRuntime) {
    $GraphifyRuntime = (Resolve-Path -LiteralPath $GraphifyRuntime).Path
    $runtimePython = Join-Path $GraphifyRuntime 'python/python.exe'
    & $runtimePython -I (Join-Path $repo 'scripts/prepare-graphify-runtime.py') --check $GraphifyRuntime
    if ($LASTEXITCODE -ne 0) { throw 'Graphify runtime validation failed' }
}

if (-not $SkipBuild) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo was not found on PATH; install Rust from https://rustup.rs"
    }
    Push-Location $repo
    try {
        # cargo reports progress on stderr, which Windows PowerShell turns
        # into error records; the exit code decides.
        $ErrorActionPreference = "Continue"
        & cargo build --release --locked -p rah-cli -p rah-desktop
        $code = $LASTEXITCODE
        $ErrorActionPreference = "Stop"
        if ($code -ne 0) { exit $code }
    } finally {
        Pop-Location
    }
}

$release = Join-Path $repo "target\release"
$dist = Join-Path $repo "dist"
$stage = Join-Path $dist "stage$suffix"
foreach ($exe in "rah.exe", "rah-desktop.exe") {
    if (-not (Test-Path -LiteralPath (Join-Path $release $exe))) { throw "$exe was not found in $release; build first." }
}
if ($GraphifyRuntime) {
    $runtimeHelp = & (Join-Path $release 'rah.exe') graphify install --help
    if ($LASTEXITCODE -ne 0 -or ($runtimeHelp -join "`n") -notmatch '--runtime-dir') { throw 'Rebuild RAH with bundled-runtime support before packaging' }
}

# Keep other package variants; only replace this generated staging directory.
$resolvedDist = [IO.Path]::GetFullPath($dist)
$resolvedStage = [IO.Path]::GetFullPath($stage)
if ([IO.Path]::GetDirectoryName($resolvedStage) -ne $resolvedDist) { throw 'Staging directory escapes dist' }
if ((Test-Path -LiteralPath $dist) -and ((Get-Item -LiteralPath $dist).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'dist must not be a link' }
if ($GraphifyRuntime -and ($GraphifyRuntime -eq $resolvedStage -or $GraphifyRuntime.StartsWith($resolvedStage + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) { throw 'Runtime source must be outside the staging directory' }
if (Test-Path -LiteralPath $stage) {
    if ((Get-Item -LiteralPath $stage).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Staging directory must not be a link' }
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item (Join-Path $release "rah.exe"), (Join-Path $release "rah-desktop.exe") -Destination $stage
Copy-Item (Join-Path $repo "packaging\windows\rah-web.ps1") -Destination $stage
Copy-Item (Join-Path $repo "packaging\windows\rah.ico") -Destination $stage
Copy-Item (Join-Path $repo "README.md") -Destination $stage
if ($GraphifyRuntime) {
    Copy-Item -LiteralPath $GraphifyRuntime -Destination (Join-Path $stage 'graphify-runtime') -Recurse
}

$zip = Join-Path $dist "rah-$version$suffix-portable.zip"
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -Force
Write-Host "Portable zip: $zip"

if (-not $Iscc) {
    $found = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    $candidates = @($(if ($found) { $found.Source }))
    foreach ($major in 7, 6) {
        $candidates += "${env:ProgramFiles(x86)}\Inno Setup $major\ISCC.exe"
        $candidates += "$env:ProgramFiles\Inno Setup $major\ISCC.exe"
        $candidates += "$env:LOCALAPPDATA\Programs\Inno Setup $major\ISCC.exe"
    }
    $candidates = $candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_) }
    $Iscc = $candidates | Select-Object -First 1
}
if (-not $Iscc) {
    Write-Warning "Inno Setup (ISCC.exe) was not found, so no installer was built. Install it (winget install JRSoftware.InnoSetup) or pass -Iscc."
    exit 1
}

& $Iscc "/DAppVersion=$version" "/DStageDir=$stage" "/Frah-$version$suffix-setup" (Join-Path $repo "packaging\windows\rah.iss")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Installer: $(Join-Path $dist "rah-$version$suffix-setup.exe")"
