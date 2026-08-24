[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('save', 'verify', 'path', 'list', 'verify-all', 'inspect-installed')]
    [string]$Command,

    [Parameter(Position = 1)]
    [string]$Version,

    [string]$StorePath,

    # This host-only escape hatch makes deterministic recovery/testing possible.
    # It is deliberately not read from model or tool input.
    [string]$SourcePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ManifestVersion = 1
$Platform = 'windows-x86_64'
$Architecture = 'x86_64'
$BinaryName = 'codex.exe'

function Write-Diagnostic([string]$Message) {
    [Console]::Error.WriteLine($Message)
}

function Fail([string]$Message) {
    throw $Message
}

function Assert-Version([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -notmatch '^\d+\.\d+\.\d+$') {
        Fail "version must be an exact semantic version such as 0.149.0"
    }
}

function Get-StoreRoot {
    if (-not [string]::IsNullOrWhiteSpace($StorePath)) {
        return [IO.Path]::GetFullPath($StorePath)
    }
    if (-not [string]::IsNullOrWhiteSpace($env:CODEX_BASELINE_HOME)) {
        return [IO.Path]::GetFullPath($env:CODEX_BASELINE_HOME)
    }
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        Fail 'LOCALAPPDATA is required when CODEX_BASELINE_HOME and -StorePath are not set'
    }
    return (Join-Path $env:LOCALAPPDATA 'codex-baselines')
}

function Get-ExactVersion([string]$NativePath) {
    $output = & $NativePath '--version' 2>&1
    if ($LASTEXITCODE -ne 0) {
        Fail "native executable failed --version: $NativePath"
    }
    $reported = ([string]($output -join "`n")).Trim()
    if ($reported -notmatch '^codex-cli (\d+\.\d+\.\d+)$') {
        Fail "native executable reported an unsupported version string: $reported"
    }
    return $reported
}

function Assert-NativeWindowsExecutable([string]$Path) {
    if (-not [IO.Path]::IsPathRooted($Path)) {
        Fail "native executable path must be absolute: $Path"
    }
    if ([IO.Path]::GetExtension($Path).ToLowerInvariant() -ne '.exe') {
        Fail "baseline payload must be a native .exe: $Path"
    }
    $item = Get-Item -LiteralPath $Path -Force
    if (-not ($item -is [IO.FileInfo])) {
        Fail "baseline payload must be a regular file: $Path"
    }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        Fail "baseline payload must not be a reparse point: $Path"
    }
    $bytes = [IO.File]::ReadAllBytes($item.FullName)
    if ($bytes.Length -lt 2 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        Fail "baseline payload is not a Windows PE executable: $Path"
    }
    return $item.FullName
}

function Get-Hash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-NativeBinaryFromPackage([string]$PackageRoot) {
    $packageJson = Join-Path $PackageRoot 'package.json'
    if (-not (Test-Path -LiteralPath $packageJson -PathType Leaf)) { return $null }
    $platformPackage = Join-Path $PackageRoot 'node_modules\@openai\codex-win32-x64'
    $candidates = @(
        (Join-Path $platformPackage 'vendor\x86_64-pc-windows-msvc\bin\codex.exe'),
        (Join-Path $PackageRoot 'vendor\x86_64-pc-windows-msvc\bin\codex.exe')
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($candidate)))
        }
    }
    return $null
}

function Get-GlobalCodexPackageRoots {
    $roots = [Collections.Generic.List[string]]::new()
    $npm = Get-Command npm -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $npm) {
        $npmRoot = (& $npm.Source 'root' '-g' 2>$null | Select-Object -First 1)
        if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($npmRoot)) {
            $roots.Add((Join-Path $npmRoot.Trim() '@openai\codex'))
        }
    }
    foreach ($command in @(Get-Command codex -All -ErrorAction SilentlyContinue)) {
        $source = $command.Source
        if ([string]::IsNullOrWhiteSpace($source)) { continue }
        $directory = Split-Path -Parent $source
        $roots.Add((Join-Path $directory 'node_modules\@openai\codex'))
    }
    return $roots | Select-Object -Unique
}

function Find-InstalledNative([string]$RequestedVersion) {
    $matches = [Collections.Generic.List[object]]::new()
    foreach ($packageRoot in Get-GlobalCodexPackageRoots) {
        $packageJson = Join-Path $packageRoot 'package.json'
        if (-not (Test-Path -LiteralPath $packageJson -PathType Leaf)) { continue }
        $package = Get-Content -LiteralPath $packageJson -Raw | ConvertFrom-Json
        if ($package.name -ne '@openai/codex' -or $package.version -ne $RequestedVersion) { continue }
        $binary = Get-NativeBinaryFromPackage $packageRoot
        if ($null -eq $binary) { continue }
        $reported = Get-ExactVersion $binary
        if ($reported -eq "codex-cli $RequestedVersion") {
            $matches.Add([pscustomobject]@{ Path = $binary; Source = 'npm-global'; SourcePackage = "@openai/codex@$($package.version)" })
        }
    }
    $unique = @($matches | Group-Object Path | ForEach-Object { $_.Group[0] })
    if ($unique.Count -eq 0) { return $null }
    if ($unique.Count -ne 1) { Fail "multiple installed native Codex binaries report requested version $RequestedVersion" }
    return $unique[0]
}

function Acquire-IsolatedNative([string]$RequestedVersion) {
    $npm = Get-Command npm -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $npm) { Fail 'npm is required to acquire an exact Codex package' }
    $temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("codex-baseline-$RequestedVersion-" + [guid]::NewGuid().ToString('N'))
    try {
        [IO.Directory]::CreateDirectory($temporaryRoot) | Out-Null
        $platformPackageVersion = "$RequestedVersion-win32-x64"
        Write-Diagnostic "acquiring @openai/codex@$platformPackageVersion in an isolated npm directory"
        & $npm.Source 'install' '--ignore-scripts' '--no-audit' '--no-fund' '--prefix' $temporaryRoot "@openai/codex@$platformPackageVersion" | Out-Null
        if ($LASTEXITCODE -ne 0) { Fail "isolated npm acquisition failed for @openai/codex@$platformPackageVersion" }
        $packageRoot = Join-Path $temporaryRoot 'node_modules\@openai\codex'
        $package = Get-Content -LiteralPath (Join-Path $packageRoot 'package.json') -Raw | ConvertFrom-Json
        if ($package.name -ne '@openai/codex' -or $package.version -ne $platformPackageVersion) {
            Fail "isolated npm package did not resolve exact @openai/codex@$platformPackageVersion"
        }
        $binary = Get-NativeBinaryFromPackage $packageRoot
        if ($null -eq $binary) { Fail "isolated npm package did not contain the Windows x64 native Codex executable" }
        $reported = Get-ExactVersion $binary
        if ($reported -ne "codex-cli $RequestedVersion") { Fail "isolated native binary version mismatch: $reported" }
        $staged = Join-Path $temporaryRoot $BinaryName
        [IO.File]::Copy($binary, $staged, $true)
        return [pscustomobject]@{ Path = $staged; Source = 'npm-isolated'; SourcePackage = "@openai/codex@$platformPackageVersion"; TemporaryRoot = $temporaryRoot }
    } catch {
        if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
        throw
    }
}

function Read-Manifest([string]$ManifestPath) {
    try { $manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json } catch { Fail "manifest is not valid JSON: $ManifestPath" }
    $allowed = @('manifest_version', 'version', 'reported_version', 'sha256', 'platform', 'architecture', 'binary', 'source', 'source_package', 'archived_at_utc')
    foreach ($property in $manifest.PSObject.Properties.Name) {
        if ($property -notin $allowed) { Fail "manifest has unsupported property: $property" }
    }
    foreach ($property in @('manifest_version', 'version', 'reported_version', 'sha256', 'platform', 'architecture', 'binary', 'source')) {
        if ($null -eq $manifest.$property -or [string]::IsNullOrWhiteSpace([string]$manifest.$property)) { Fail "manifest is missing $property" }
    }
    return $manifest
}

function Verify-Baseline([string]$RequestedVersion) {
    Assert-Version $RequestedVersion
    if ($env:OS -ne 'Windows_NT' -or [Environment]::Is64BitOperatingSystem -ne $true -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() -ne 'X64') {
        Fail 'Windows x64 baseline verification is required for this baseline store'
    }
    $directory = Join-Path (Get-StoreRoot) $RequestedVersion
    $manifestPath = Join-Path $directory 'manifest.json'
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) { Fail "baseline directory does not exist: $RequestedVersion" }
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { Fail "baseline manifest does not exist: $RequestedVersion" }
    $manifest = Read-Manifest $manifestPath
    if ([int]$manifest.manifest_version -ne $ManifestVersion) { Fail "unsupported manifest version: $($manifest.manifest_version)" }
    if ($manifest.version -ne $RequestedVersion -or $manifest.reported_version -ne "codex-cli $RequestedVersion") { Fail "manifest version does not match requested version: $RequestedVersion" }
    if ($manifest.platform -ne $Platform -or $manifest.architecture -ne $Architecture) { Fail "baseline is not compatible with Windows x64" }
    if ($manifest.binary -ne $BinaryName) { Fail "baseline binary must be $BinaryName" }
    if ($manifest.sha256 -notmatch '^[0-9a-f]{64}$') { Fail 'manifest SHA-256 must be lowercase hexadecimal' }
    $binary = Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath((Join-Path $directory $manifest.binary)))
    if ((Get-Hash $binary) -ne $manifest.sha256) { Fail "baseline SHA-256 mismatch: $RequestedVersion" }
    if ((Get-ExactVersion $binary) -ne $manifest.reported_version) { Fail "baseline native binary version mismatch: $RequestedVersion" }
    return [pscustomobject]@{ Binary = $binary; Manifest = $manifest; ManifestPath = $manifestPath }
}

function Save-Baseline([string]$RequestedVersion) {
    Assert-Version $RequestedVersion
    $acquired = $null
    try {
        if (-not [string]::IsNullOrWhiteSpace($SourcePath)) {
            $native = Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($SourcePath))
            $acquired = [pscustomobject]@{ Path = $native; Source = 'host-path'; SourcePackage = $null; TemporaryRoot = $null }
        } else {
            try { $acquired = Acquire-IsolatedNative $RequestedVersion } catch {
                Write-Diagnostic "isolated acquisition unavailable: $($_.Exception.Message); checking the exact installed global package"
                $acquired = Find-InstalledNative $RequestedVersion
                if ($null -eq $acquired) { throw }
                $acquired | Add-Member -NotePropertyName TemporaryRoot -NotePropertyValue $null
            }
        }
        $reported = Get-ExactVersion $acquired.Path
        if ($reported -ne "codex-cli $RequestedVersion") { Fail "source native binary version mismatch: $reported" }
        $sourceHash = Get-Hash $acquired.Path
        $root = Get-StoreRoot
        [IO.Directory]::CreateDirectory($root) | Out-Null
        $destination = Join-Path $root $RequestedVersion
        if (Test-Path -LiteralPath $destination) {
            $verified = Verify-Baseline $RequestedVersion
            if ($verified.Manifest.sha256 -eq $sourceHash) {
                Write-Diagnostic "baseline $RequestedVersion already exists with the same SHA-256"
                return
            }
            Fail "baseline $RequestedVersion already exists with a different SHA-256"
        }
        $staging = Join-Path $root (".$RequestedVersion-staging-" + [guid]::NewGuid().ToString('N'))
        [IO.Directory]::CreateDirectory($staging) | Out-Null
        try {
            $persisted = Join-Path $staging $BinaryName
            [IO.File]::Copy($acquired.Path, $persisted, $false)
            $persistedHash = Get-Hash $persisted
            if ($persistedHash -ne $sourceHash) { Fail 'persisted baseline SHA-256 differs from source' }
            $manifest = [ordered]@{
                manifest_version = $ManifestVersion
                version = $RequestedVersion
                reported_version = $reported
                sha256 = $sourceHash
                platform = $Platform
                architecture = $Architecture
                binary = $BinaryName
                source = $acquired.Source
                source_package = $acquired.SourcePackage
                archived_at_utc = [DateTime]::UtcNow.ToString('o')
            }
            [IO.File]::WriteAllText((Join-Path $staging 'manifest.json'), ($manifest | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
            [IO.Directory]::Move($staging, $destination)
        } finally {
            if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging -Recurse -Force }
        }
        $null = Verify-Baseline $RequestedVersion
        Write-Diagnostic "saved verified baseline $RequestedVersion"
    } finally {
        if ($null -ne $acquired -and -not [string]::IsNullOrWhiteSpace($acquired.TemporaryRoot) -and (Test-Path -LiteralPath $acquired.TemporaryRoot)) {
            Remove-Item -LiteralPath $acquired.TemporaryRoot -Recurse -Force
        }
    }
}

try {
    switch ($Command) {
        'save' { if ($null -eq $Version) { Fail 'save requires a version' }; Save-Baseline $Version }
        'verify' { if ($null -eq $Version) { Fail 'verify requires a version' }; $null = Verify-Baseline $Version; Write-Diagnostic "verified baseline $Version" }
        'path' { if ($null -eq $Version) { Fail 'path requires a version' }; (Verify-Baseline $Version).Binary }
        'list' {
            $root = Get-StoreRoot
            if (Test-Path -LiteralPath $root -PathType Container) {
                Get-ChildItem -LiteralPath $root -Directory | Sort-Object Name | ForEach-Object {
                    try { $verified = Verify-Baseline $_.Name; "{0} {1} {2} verified" -f $_.Name, $verified.Manifest.platform, $verified.Manifest.sha256 } catch { "{0} invalid {1}" -f $_.Name, $_.Exception.Message }
                }
            }
        }
        'verify-all' {
            $root = Get-StoreRoot
            $failed = $false
            if (Test-Path -LiteralPath $root -PathType Container) {
                foreach ($directory in Get-ChildItem -LiteralPath $root -Directory | Sort-Object Name) {
                    try { $null = Verify-Baseline $directory.Name; Write-Diagnostic "verified baseline $($directory.Name)" } catch { Write-Diagnostic "invalid baseline $($directory.Name): $($_.Exception.Message)"; $failed = $true }
                }
            }
            if ($failed) { exit 1 }
        }
        'inspect-installed' {
            if ($null -eq $Version) { Fail 'inspect-installed requires a version' }
            Assert-Version $Version
            $installed = Find-InstalledNative $Version
            if ($null -eq $installed) { Fail "no exact installed native Codex $Version was found" }
            "{0} {1} {2}" -f $installed.Path, $installed.Source, $installed.SourcePackage
        }
    }
} catch {
    Write-Diagnostic "codex-baseline: $($_.Exception.Message)"
    exit 1
}
