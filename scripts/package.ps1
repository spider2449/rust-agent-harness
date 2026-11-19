<#
.SYNOPSIS
Builds RAH with LSP and Graphify (Windows installer and portable ZIP).

.DESCRIPTION
Builds `rah` and `rah-desktop` in release mode, stages them with the Web
launcher under `dist\stage`, zips the stage as `rah-<version>-portable.zip`,
and compiles `packaging\windows\rah.iss` with Inno Setup (ISCC.exe) into
`dist\rah-<version>-setup.exe`. The installer is per-user (no administrator
rights), adds Start Menu entries and optionally the PATH, and has an
uninstaller that keeps the data directory (`~\.rah`). By default both prepared
runtimes are discovered under .rah and included in a combined package named
`rah-<version>-graphify-bundled-lsp-bundled-*`. -Lightweight explicitly builds
the original package without either runtime. Missing runtimes fail before build.

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
Override Graphify's location; LSP is still discovered automatically.

.EXAMPLE
.\scripts\package.ps1 -Lightweight
Build without either runtime.
#>
[CmdletBinding()]
param(
    # Path of ISCC.exe when it is not found automatically.
    [string]$Iscc,
    # Reuse target\release instead of building.
    [switch]$SkipBuild,
    # Prepared relocatable Node/Python/Graphify bundle; target machines need no uv.
    [string]$GraphifyRuntime,
    # Prepared Windows x64 language servers and Node; see docs/LSP_PACKAGING.md.
    [string]$LspRuntime,
    # Explicitly omit both runtimes (for the original lightweight release).
    [switch]$Lightweight
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

function Get-BundlePython([string]$Graph) {
    $name = (Get-Content -LiteralPath (Join-Path $Graph 'runtime.json') -Raw | ConvertFrom-Json).python
    switch -CaseSensitive ($name) {
        'python/python.exe' { return Join-Path $Graph 'python/python.exe' }
        '../runtime/bin/python.exe' { return Join-Path (Split-Path -Parent $Graph) 'runtime/bin/python.exe' }
        default { throw "Unsupported bundled Python location: $name" }
    }
}

function Resolve-PackageRuntimes([string]$Root, [string]$Graphify, [string]$Lsp, [bool]$Minimal) {
    if ($Minimal) {
        if ($Graphify -or $Lsp) { throw '-Lightweight cannot be combined with runtime paths.' }
        return @{ Graphify = ''; Lsp = '' }
    }
    $selected = @{}
    foreach ($entry in @(
        @{ Name = 'Graphify'; Explicit = $Graphify; Defaults = @('.rah/graphify-no-uv-runtime', '.rah/graphify-runtime') },
        @{ Name = 'Lsp'; Explicit = $Lsp; Defaults = @('.rah/lsp-runtime') }
    )) {
        $paths = if ($entry.Explicit) { @($entry.Explicit) } else { @($entry.Defaults | ForEach-Object { Join-Path $Root $_ }) }
        $foundPath = $null
        foreach ($path in $paths) {
            if (Test-Path -LiteralPath (Join-Path $path 'runtime.json')) {
                $foundPath = (Resolve-Path -LiteralPath $path).Path
                break
            }
        }
        if (-not $foundPath) { throw "Missing $($entry.Name) runtime. Prepare it first or pass -$($entry.Name)Runtime DIR; use -Lightweight to omit both. See docs/LSP_PACKAGING.md and docs/GRAPHIFY_PACKAGING.md." }
        $selected[$entry.Name] = $foundPath
    }
    return $selected
}

$runtimes = Resolve-PackageRuntimes $repo $GraphifyRuntime $LspRuntime ([bool]$Lightweight)
$GraphifyRuntime = $runtimes.Graphify
$LspRuntime = $runtimes.Lsp
if ($GraphifyRuntime) { $runtimePython = Get-BundlePython $GraphifyRuntime }

$manifest = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "No workspace version in Cargo.toml." }
$version = $Matches[1]
$suffix = if ($GraphifyRuntime) { '-graphify-bundled' } else { '' }
if ($LspRuntime) {
    $suffix += '-lsp-bundled'
    $LspRuntime = (Resolve-Path -LiteralPath $LspRuntime).Path
    & $runtimePython -I (Join-Path $repo 'scripts/prepare-lsp-runtime.py') --check $LspRuntime
    if ($LASTEXITCODE -ne 0) { throw 'LSP runtime validation failed' }
}
if ($GraphifyRuntime) {
    $GraphifyRuntime = (Resolve-Path -LiteralPath $GraphifyRuntime).Path
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
    if (($runtimeHelp -join "`n") -notmatch 'runtime/bin/python.exe') { throw 'Rebuild RAH with shared Python support before packaging' }
}

# Keep other package variants; only replace this generated staging directory.
$resolvedDist = [IO.Path]::GetFullPath($dist)
$resolvedStage = [IO.Path]::GetFullPath($stage)
if ([IO.Path]::GetDirectoryName($resolvedStage) -ne $resolvedDist) { throw 'Staging directory escapes dist' }
if ((Test-Path -LiteralPath $dist) -and ((Get-Item -LiteralPath $dist).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'dist must not be a link' }
if ($GraphifyRuntime -and ($GraphifyRuntime -eq $resolvedStage -or $GraphifyRuntime.StartsWith($resolvedStage + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) { throw 'Runtime source must be outside the staging directory' }
if ($LspRuntime -and ($LspRuntime -eq $resolvedStage -or $LspRuntime.StartsWith($resolvedStage + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) { throw 'LSP runtime source must be outside the staging directory' }
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
    Copy-Item -LiteralPath (Join-Path $repo 'packaging/windows/README-graphify-bundled.md') -Destination (Join-Path $stage 'README.md') -Force
    $graphStage = New-Item -ItemType Directory -Path (Join-Path $stage 'graphify-runtime')
    Get-ChildItem -LiteralPath $GraphifyRuntime -Force | Where-Object Name -ne 'python' |
        Copy-Item -Destination $graphStage.FullName -Recurse
    $bin = New-Item -ItemType Directory -Force -Path (Join-Path $stage 'runtime/bin')
    Get-ChildItem -LiteralPath (Split-Path -Parent $runtimePython) -Force | Where-Object Name -ne 'node.exe' |
        Copy-Item -Destination $bin.FullName -Recurse
    Copy-Item -LiteralPath (Join-Path $stage 'README.md') -Destination (Join-Path $dist "rah-$version$suffix-README.md") -Force
}

if ($LspRuntime) {
    Copy-Item -LiteralPath $LspRuntime -Destination (Join-Path $stage 'lsp-runtime') -Recurse
    & $runtimePython -I (Join-Path $repo 'scripts/prepare-lsp-runtime.py') --share-node (Join-Path $stage 'lsp-runtime') --node-sources $LspRuntime $GraphifyRuntime
    if ($LASTEXITCODE -ne 0) { throw 'Shared Node staging validation failed' }
    $lspReadme = Get-Content -LiteralPath (Join-Path $repo 'packaging/windows/README-lsp-bundled.md') -Raw
    $existingReadme = Get-Content -LiteralPath (Join-Path $stage 'README.md') -Raw
    [IO.File]::WriteAllText((Join-Path $stage 'README.md'), $lspReadme + "`r`n`r`n" + $existingReadme, [Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath (Join-Path $stage 'README.md') -Destination (Join-Path $dist "rah-$version$suffix-README.md") -Force
}

if ($GraphifyRuntime) {
    $metadataFile = Join-Path $graphStage.FullName 'runtime.json'
    $metadata = Get-Content -LiteralPath $metadataFile -Raw | ConvertFrom-Json
    $metadata.python = '../runtime/bin/python.exe'
    [IO.File]::WriteAllText($metadataFile, ($metadata | ConvertTo-Json -Depth 20), [Text.UTF8Encoding]::new($false))
    & (Get-BundlePython $graphStage.FullName) -I (Join-Path $repo 'scripts/prepare-graphify-runtime.py') --check $graphStage.FullName
    if ($LASTEXITCODE -ne 0) { throw 'Shared Python staging validation failed' }
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
