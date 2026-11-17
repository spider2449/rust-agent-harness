<#
.SYNOPSIS
Installs or updates RAH: the one command to get the current version onto
this machine.

.DESCRIPTION
Downloads the latest combined LSP/Graphify GitHub release ZIP, checks it against
the release's SHA256SUMS.txt, and copies `rah`, `rah-desktop`, the Web
launcher, the icon and the README into the install directory, the same one
the installer uses (`%LOCALAPPDATA%\Programs\RAH`). Nothing is compiled.

Run from a checkout whose version is newer than the latest release, it
builds that checkout instead (release profile), so unreleased changes can be
installed too. Auto mode also builds when the release has no combined ZIP.
-Source chooses explicitly: `release` or `build`. Builds require prepared
runtimes under .rah or explicit -LspRuntime and -GraphifyRuntime directories.

An existing install, its Start Menu entries and its PATH entry keep working
and now run the new version. On a machine without RAH it also adds the Start
Menu entries and puts the directory on the user's PATH.

RAH may keep running while it downloads or builds. Before copying, a running
`rah` or `rah-desktop` from the install directory is closed after asking
(-Force: without asking), and RAH Desktop is opened again afterwards.

Both LSP and Graphify runtimes are required and installed beside RAH. Existing
configuration is preserved and backed up when enabling LSP and Graphify;
sessions are not changed. A fresh profile leaves model selection to setup.

After a build, when `target` has grown past -MaxTargetGB, the debug build
cache is removed (`cargo clean --profile dev`); the release build is kept.

.EXAMPLE
.\scripts\update.ps1

.EXAMPLE
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/spider2449/rust-agent-harness/master/scripts/update.ps1)))
Installs the latest release on a machine without a checkout.
#>
[CmdletBinding()]
param(
    # Where RAH is installed.
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\RAH"),
    # `auto` (default): the latest release, or this checkout when it is
    # newer. `release`: always download. `build`: always build the checkout.
    [ValidateSet("auto", "release", "build")]
    [string]$Source = "auto",
    # The GitHub repository releases come from.
    [string]$Repository = "spider2449/rust-agent-harness",
    # Remove the debug build cache when target is larger than this.
    [int]$MaxTargetGB = 30,
    # Do not create Start Menu entries or change PATH on a new install.
    [switch]$NoShortcuts,
    # Close a running RAH without asking. Same-version updates also repair runtimes.
    [switch]$Force,
    # Prepared runtime directories for a source build (otherwise discovered under .rah).
    [string]$LspRuntime,
    [string]$GraphifyRuntime,
    # Profile to enable the integrations in; respects RAH_HOME by default.
    [string]$DataDir = $(if ($env:RAH_HOME) { $env:RAH_HOME } else { Join-Path $env:USERPROFILE '.rah' }),
    [string]$Config
)

$ErrorActionPreference = "Stop"
# Windows PowerShell 5.1 may default to TLS 1.0, which GitHub refuses.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$DataDir = [IO.Path]::GetFullPath($DataDir)
if (-not $Config) { $Config = Join-Path $DataDir 'rah.toml' }
$Config = [IO.Path]::GetFullPath($Config)

function Find-UpdateRuntime([string]$ExplicitPath, [string[]]$Candidates, [string]$Name) {
    if ($ExplicitPath) { $Candidates = @($ExplicitPath) }
    foreach ($candidatePath in $Candidates) {
        if (Test-Path -LiteralPath (Join-Path $candidatePath 'runtime.json')) {
            return (Resolve-Path -LiteralPath $candidatePath).Path
        }
    }
    throw "Missing $Name runtime; prepare it first or supply -${Name}Runtime. See docs/LSP_PACKAGING.md and docs/GRAPHIFY_PACKAGING.md. Nothing was installed."
}

function Move-UpdateDirectory([string]$From, [string]$To) {
    # A scanner can briefly retain directory handles after copying thousands
    # of files. Directory.Move never merges into an existing destination.
    for ($attempt = 0; $attempt -lt 12; $attempt++) {
        try { [IO.Directory]::Move($From, $To); return } catch {
            if ($attempt -eq 11 -or -not (Test-Path -LiteralPath $From) -or (Test-Path -LiteralPath $To)) { throw }
            Start-Sleep -Milliseconds 250
        }
    }
}

function Install-UpdateRuntime([string]$RuntimeSource, [string]$DestinationRoot, [string]$Name) {
    $rootPath = [IO.Path]::GetFullPath($DestinationRoot)
    $destination = Join-Path $rootPath $Name
    $staged = Join-Path $rootPath "$Name.new-$([guid]::NewGuid().ToString('N'))"
    $backup = Join-Path $rootPath "$Name.previous-$([guid]::NewGuid().ToString('N'))"
    foreach ($targetPath in @($destination, $staged, $backup)) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($targetPath)) -ne $rootPath) { throw 'Runtime destination escapes install directory' }
        if ((Test-Path -LiteralPath $targetPath) -and ((Get-Item -LiteralPath $targetPath).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Runtime destination must not be a link' }
    }
    $sourcePath = (Resolve-Path -LiteralPath $RuntimeSource).Path
    if ($sourcePath -eq $destination -or $sourcePath.StartsWith($destination + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime source must be outside its installed destination' }
    try {
        Copy-Item -LiteralPath $sourcePath -Destination $staged -Recurse
        if (Test-Path -LiteralPath $destination) { Move-UpdateDirectory $destination $backup }
        try { Move-UpdateDirectory $staged $destination } catch {
            if (Test-Path -LiteralPath $backup) { Move-UpdateDirectory $backup $destination }
            throw
        }
        if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Recurse -Force }
    } finally {
        if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged -Recurse -Force }
    }
}

# The checkout this script is in, if any (none when run from a download).
$repo = $null
if ($PSScriptRoot) {
    $candidate = Split-Path -Parent $PSScriptRoot
    if (Test-Path -LiteralPath (Join-Path $candidate "Cargo.toml")) { $repo = $candidate }
}
$checkoutVersion = $null
if ($repo) {
    $manifest = Get-Content -LiteralPath (Join-Path $repo "Cargo.toml") -Raw
    if ($manifest -match '(?m)^version\s*=\s*"([^"]+)"') { $checkoutVersion = $Matches[1] }
}

$installedRah = Join-Path $InstallDir "rah.exe"
$oldVersion = $null
if (Test-Path -LiteralPath $installedRah) {
    $oldVersion = ((& $installedRah --version) -replace '^rah\s+', '').Trim()
}
$fresh = -not (Test-Path -LiteralPath $InstallDir)

# The latest release, unless only a build was asked for.
$latest = $null
if ($Source -ne "build") {
    try {
        $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest" -Headers @{ "User-Agent" = "rah-update" }
    } catch {
        if ($Source -eq "release" -or -not $repo) { throw "Cannot read the latest release of ${Repository}: $($_.Exception.Message)" }
        Write-Warning "Cannot read the latest release ($($_.Exception.Message)); building this checkout instead."
    }
}
$releaseVersion = if ($latest) { $latest.tag_name.TrimStart("v") } else { $null }

if ($Source -eq "auto") {
    $Source = "release"
    if ($repo -and $checkoutVersion -and (-not $releaseVersion -or [version]$checkoutVersion -gt [version]$releaseVersion)) {
        if ($releaseVersion) { Write-Host "This checkout ($checkoutVersion) is newer than the latest release ($releaseVersion)." }
        $Source = "build"
    }
    if ($repo -and $latest -and -not ($latest.assets | Where-Object name -eq "rah-$releaseVersion-graphify-bundled-lsp-bundled-portable.zip")) {
        Write-Host 'The release has no combined LSP/Graphify package; building this checkout.'
        $Source = 'build'
    }
}
if ($Source -eq "build" -and -not $repo) { throw "-Source build needs this script inside a checkout." }

# $files: what gets copied into the install directory.
$work = $null
if ($Source -eq "release") {
    $version = $releaseVersion
    # Same-version updates also repair missing runtimes and enable the profile.
    $zipName = "rah-$version-graphify-bundled-lsp-bundled-portable.zip"
    $zipAsset = $latest.assets | Where-Object name -eq $zipName
    $sumsAsset = $latest.assets | Where-Object name -eq "SHA256SUMS.txt"
    if (-not $zipAsset -or -not $sumsAsset) { throw "Release $($latest.tag_name) has no $zipName or SHA256SUMS.txt." }
    $work = Join-Path ([IO.Path]::GetTempPath()) "rah-update-$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $work | Out-Null
    $zip = Join-Path $work $zipName
    $sums = Join-Path $work "SHA256SUMS.txt"
    Write-Host "Downloading RAH $version..."
    $ProgressPreference = "SilentlyContinue"
    # The API's asset URL first: some networks time out on the github.com
    # download link but reach the API. Each is tried up to three times.
    function Get-Asset($asset, $path) {
        $sources = @(
            @{ Uri = $asset.url; Headers = @{ "User-Agent" = "rah-update"; "Accept" = "application/octet-stream" } },
            @{ Uri = $asset.browser_download_url; Headers = @{ "User-Agent" = "rah-update" } }
        )
        $failure = $null
        foreach ($source in $sources) {
            for ($attempt = 1; $attempt -le 3; $attempt++) {
                try {
                    Invoke-WebRequest -Uri $source.Uri -Headers $source.Headers -OutFile $path -UseBasicParsing
                    return
                } catch {
                    $failure = $_.Exception.Message
                    Start-Sleep -Seconds $attempt
                }
            }
        }
        throw "Cannot download $($asset.name): $failure"
    }
    Get-Asset $zipAsset $zip
    Get-Asset $sumsAsset $sums
    $expected = Get-Content -LiteralPath $sums | ForEach-Object {
        $parts = $_ -split '\s+\*?', 2
        if ($parts.Count -eq 2 -and $parts[1].Trim() -eq $zipName) { $parts[0].Trim().ToLowerInvariant() }
    } | Select-Object -First 1
    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $expected -or $expected -ne $actual) { throw "$zipName does not match SHA256SUMS.txt; nothing was installed." }
    $unpacked = Join-Path $work "rah"
    Expand-Archive -LiteralPath $zip -DestinationPath $unpacked
    $files = "rah.exe", "rah-desktop.exe", "rah-web.ps1", "rah.ico", "README.md" | ForEach-Object { Join-Path $unpacked $_ }
    $LspRuntime = Find-UpdateRuntime '' @((Join-Path $unpacked 'lsp-runtime')) 'Lsp'
    $GraphifyRuntime = Find-UpdateRuntime '' @((Join-Path $unpacked 'graphify-runtime')) 'Graphify'
} else {
    $version = $checkoutVersion
    $LspRuntime = Find-UpdateRuntime $LspRuntime @((Join-Path $repo '.rah/lsp-runtime')) 'Lsp'
    $GraphifyRuntime = Find-UpdateRuntime $GraphifyRuntime @((Join-Path $repo '.rah/graphify-no-uv-runtime'), (Join-Path $repo '.rah/graphify-runtime')) 'Graphify'
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo was not found on PATH; install Rust from https://rustup.rs, or use -Source release"
    }
    Write-Host "Building RAH $version from $repo (release)..."
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
    $release = Join-Path $repo "target\release"
    $files = @(
        (Join-Path $release "rah.exe"),
        (Join-Path $release "rah-desktop.exe"),
        (Join-Path $repo "packaging\windows\rah-web.ps1"),
        (Join-Path $repo "packaging\windows\rah.ico"),
        (Join-Path $repo "README.md")
    )
}
foreach ($file in $files) {
    if (-not (Test-Path -LiteralPath $file)) { throw "$file is missing; nothing was installed." }
}

# Validate before closing the user's running application. Use the bundled Python,
# never npm, pip or a global interpreter on the target machine.
$bundlePython = Join-Path $GraphifyRuntime 'python/python.exe'
$validateBundles = @'
import hashlib, importlib.metadata, json, pathlib, platform, sys
graph, lsp = (pathlib.Path(p).resolve(strict=True) for p in sys.argv[1:])
assert sys.version_info >= (3, 11), 'Updater needs bundled Python 3.11+'
assert pathlib.Path(sys.base_prefix).resolve().is_relative_to(graph), 'Python is not portable'
assert importlib.metadata.version('graphifyy') == '0.9.107', 'Graphify version mismatch'
import graphify, graphify.serve, mcp, tree_sitter
assert pathlib.Path(graphify.__file__).resolve().is_relative_to(graph)
for root in (graph, lsp):
    manifest = json.loads((root / 'runtime.json').read_text(encoding='utf-8'))
    assert (manifest['format'], manifest['platform'], manifest['arch']) == (1, 'windows', 'x86_64'), 'Unsupported runtime'
    assert platform.machine().lower() in ('amd64', 'x86_64'), 'Windows x64 required'
for root, names in ((graph, ['node.exe', 'python/python.exe']), (lsp, ['node.exe', 'rust-analyzer.exe', 'clangd/bin/clangd.exe', 'node_modules/typescript/lib/tsserver.js', 'node_modules/typescript-language-server/lib/cli.mjs', 'node_modules/pyright/langserver.index.js'])):
    for name in names:
        path = (root / name).resolve(strict=True)
        assert path.is_file() and path.is_relative_to(root), 'Missing or escaped runtime program'
manifest = json.loads((lsp / 'runtime.json').read_text(encoding='utf-8'))
for name, expected in manifest['sha256'].items():
    path = (lsp / name).resolve(strict=True)
    assert path.is_relative_to(lsp), 'Runtime path escapes bundle'
    with path.open('rb') as stream:
        assert hashlib.file_digest(stream, 'sha256').hexdigest() == expected, f'Checksum mismatch: {name}'
'@
$validateBundles | & $bundlePython -I - $GraphifyRuntime $LspRuntime
if ($LASTEXITCODE -ne 0) { throw 'Runtime validation failed; nothing was installed.' }

try {
    # A running copy keeps its files locked. RAH may keep running until now;
    # it is closed after asking (or at once with -Force).
    $running = @(Get-Process -Name rah, rah-desktop -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($InstallDir.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) })
    $reopen = $false
    if ($running.Count -gt 0) {
        $names = ($running | ForEach-Object { "$($_.Name) (pid $($_.Id))" }) -join ", "
        if (-not $Force) {
            Write-Host "RAH is running from ${InstallDir}: $names"
            $answer = Read-Host "Close it to finish the update? A running turn is cancelled. [Y/n]"
            if ($answer -and $answer -notmatch '^(y|yes)$') {
                throw "Not updated. Close RAH and run this again."
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

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    if ((Get-Item -LiteralPath $InstallDir).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Install directory must not be a link' }
    Install-UpdateRuntime $LspRuntime $InstallDir 'lsp-runtime'
    Install-UpdateRuntime $GraphifyRuntime $InstallDir 'graphify-runtime'
    Copy-Item -LiteralPath $files -Destination $InstallDir -Force
} finally {
    if ($work) {
        $resolvedWork = [IO.Path]::GetFullPath($work)
        if ([IO.Path]::GetDirectoryName($resolvedWork).TrimEnd('\') -ne [IO.Path]::GetTempPath().TrimEnd('\') -or [IO.Path]::GetFileName($resolvedWork) -notlike 'rah-update-*') { throw 'Unexpected update temporary directory' }
        Remove-Item -LiteralPath $resolvedWork -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Append a mount rather than rewriting the user's TOML. Existing LSP settings,
# features, model selection, permissions and comments remain byte-for-byte intact.
$enableLsp = @'
import os, pathlib, sys, tempfile, time, tomllib
path = pathlib.Path(sys.argv[1])
home = pathlib.Path(sys.argv[2])
original = path.read_bytes() if path.exists() else None
text = original.decode('utf-8-sig') if original is not None else 'permissions = "ask"\n'
doc = tomllib.loads(text)
plugins = doc.get('plugin', [])
if not isinstance(plugins, list) or not all(isinstance(p, dict) for p in plugins):
    raise ValueError('plugin must be an array of tables')
features = doc.get('features', [])
if not isinstance(features, list):
    raise ValueError('features must be an array')
enabled = 'lsp' in features or any(p.get('use') == 'lsp-diagnostics' for p in plugins)
if not enabled:
    text += '\n[[plugin]]\nuse = "lsp-diagnostics"\n'
updated = original if enabled and original is not None else text.encode('utf-8')
if updated != original:
    tomllib.loads(updated.decode('utf-8-sig'))
    path.parent.mkdir(parents=True, exist_ok=True)
    if original is not None:
        backup = home / 'config-backups' / ('lsp-update-' + str(time.time_ns()))
        backup.mkdir(parents=True)
        (backup / 'rah.toml.before').write_bytes(original)
    fd, temporary = tempfile.mkstemp(prefix='.rah-lsp-', suffix='.toml', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as output:
            output.write(updated)
        if (path.read_bytes() if path.exists() else None) != original:
            raise RuntimeError('Configuration changed during update; refusing to overwrite')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
'@
$installedPython = Join-Path $InstallDir 'graphify-runtime/python/python.exe'
$enableLsp | & $installedPython -I - $Config $DataDir
if ($LASTEXITCODE -ne 0) { throw 'RAH files were installed, but LSP profile activation failed; configuration was not overwritten.' }
$ErrorActionPreference = 'Continue'
& $installedRah --data-dir $DataDir --config $Config graphify install --offline
$graphifyCode = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
if ($graphifyCode -ne 0) { throw 'RAH and LSP were installed, but Graphify activation failed. Resolve the error and rerun update.ps1.' }
Write-Host "LSP and Graphify installed and enabled in $Config."

if ($fresh -and -not $NoShortcuts) {
    # What the installer adds: Start Menu entries and a user PATH entry.
    $menuDir = Join-Path ([Environment]::GetFolderPath("Programs")) "RAH"
    New-Item -ItemType Directory -Force -Path $menuDir | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    $web = $shell.CreateShortcut((Join-Path $menuDir "RAH Web.lnk"))
    $web.TargetPath = "powershell.exe"
    $web.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $InstallDir 'rah-web.ps1')`""
    $web.Arguments += " --data-dir `"$DataDir`" --config `"$Config`""
    $web.WorkingDirectory = $env:USERPROFILE
    $web.IconLocation = Join-Path $InstallDir "rah.ico"
    $web.Description = "RAH Web UI in the default browser"
    $web.Save()
    $desktop = $shell.CreateShortcut((Join-Path $menuDir "RAH Desktop.lnk"))
    $desktop.TargetPath = Join-Path $InstallDir "rah-desktop.exe"
    $desktop.Arguments = "--data-dir `"$DataDir`" --config `"$Config`""
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
$from = if ($Source -eq "release") { "release" } else { "built from $repo" }
if ($oldVersion -and $oldVersion -ne $installed) {
    Write-Host "RAH updated: $oldVersion -> $installed ($from; $InstallDir)"
} elseif ($oldVersion) {
    Write-Host "RAH $installed reinstalled ($from; $InstallDir)"
} else {
    Write-Host "RAH $installed installed ($from; $InstallDir)"
}
if ($reopen) {
    Start-Process -FilePath (Join-Path $InstallDir "rah-desktop.exe") -ArgumentList @('--data-dir', "`"$DataDir`"", '--config', "`"$Config`"") -WorkingDirectory $env:USERPROFILE
    Write-Host "Reopened RAH Desktop."
}

# Another `rah` earlier on PATH would still run instead of this one.
$found = Get-Command rah -All -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
$first = $found | Select-Object -First 1
if ($first -and -not $first.StartsWith($InstallDir, [StringComparison]::OrdinalIgnoreCase)) {
    Write-Warning "``rah`` in a terminal runs $first, not this install. Remove it (for a cargo install: cargo uninstall rah-cli rah-desktop)."
}

# The build cache only grows; trim it when it is large.
if ($Source -eq "build") {
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
}
