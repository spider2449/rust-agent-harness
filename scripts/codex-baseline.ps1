[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('save', 'verify', 'repair', 'path', 'list', 'verify-all', 'inspect-installed')]
    [string]$Command,

    [Parameter(Position = 1)]
    [string]$Version,

    [string]$StorePath,

    # This host-only escape hatch makes deterministic recovery/testing possible.
    # It is deliberately not read from model or tool input.
    [string]$SourcePath,

    [string]$SourceCodeModeHostPath,

    # Test-only deterministic fault seam. This is never supplied by Desktop,
    # the model, or a provider.
    [ValidateSet('none', 'staged-verification', 'destination-to-backup', 'staging-to-destination')]
    [string]$TestFault = 'none'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ManifestVersion = 2
$Platform = 'windows-x86_64'
$Architecture = 'x86_64'
$BinaryName = 'codex.exe'
$CodeModeHostName = 'codex-code-mode-host.exe'

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

function Test-WindowsX64Host {
    if ($env:OS -ne 'Windows_NT' -or [Environment]::Is64BitOperatingSystem -ne $true) {
        return $false
    }

    $runtimeInformationType = [Runtime.InteropServices.RuntimeInformation]
    $osArchitectureProperty = $runtimeInformationType.GetProperty(
        'OSArchitecture',
        [Reflection.BindingFlags]'Public, Static'
    )
    if ($null -ne $osArchitectureProperty) {
        return ([string]$osArchitectureProperty.GetValue($null) -eq 'X64')
    }

    $architecture = $env:PROCESSOR_ARCHITEW6432
    if ([string]::IsNullOrWhiteSpace($architecture)) {
        $architecture = $env:PROCESSOR_ARCHITECTURE
    }
    return ($architecture -eq 'AMD64')
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

function Get-BaselineDirectory([string]$RequestedVersion) {
    return (Join-Path (Get-StoreRoot) $RequestedVersion)
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
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        Fail "baseline payload is not a Windows PE executable: $Path"
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($peOffset -lt 0 -or $peOffset -gt ($bytes.Length - 26) -or
        $bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset + 1] -ne 0x45 -or
        $bytes[$peOffset + 2] -ne 0x00 -or $bytes[$peOffset + 3] -ne 0x00 -or
        [BitConverter]::ToUInt16($bytes, $peOffset + 4) -ne 0x8664 -or
        [BitConverter]::ToUInt16($bytes, $peOffset + 24) -ne 0x20b) {
        Fail "baseline payload is not a native Windows x64 PE executable: $Path"
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

function Get-CodeModeHostForBinary([string]$NativePath) {
    $candidate = Join-Path (Split-Path -Parent $NativePath) $CodeModeHostName
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) { return $null }
    return (Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($candidate)))
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
        $codeModeHost = Get-CodeModeHostForBinary $binary
        if ($reported -eq "codex-cli $RequestedVersion" -and $null -ne $codeModeHost) {
            $matches.Add([pscustomobject]@{ Path = $binary; CodeModeHostPath = $codeModeHost; Source = 'npm-global'; SourcePackage = "@openai/codex@$($package.version)" })
        }
    }
    $unique = @($matches | Group-Object Path | ForEach-Object { $_.Group[0] })
    if ($unique.Count -eq 0) { return $null }
    if ($unique.Count -ne 1) { Fail "multiple installed native Codex binaries report requested version $RequestedVersion" }
    return $unique[0]
}

function Get-SourceBundle([string]$RequestedVersion) {
    $hasNative = -not [string]::IsNullOrWhiteSpace($SourcePath)
    $hasCodeModeHost = -not [string]::IsNullOrWhiteSpace($SourceCodeModeHostPath)
    if ($hasNative -xor $hasCodeModeHost) {
        Fail '-SourcePath and -SourceCodeModeHostPath must be supplied together'
    }

    $acquired = $null
    if ($hasNative) {
        $native = Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($SourcePath))
        $codeModeHost = Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($SourceCodeModeHostPath))
        $acquired = [pscustomobject]@{
            Path = $native
            CodeModeHostPath = $codeModeHost
            Source = 'host-path'
            SourcePackage = 'host-provided-exact-version-bundle'
            TemporaryRoot = $null
        }
    } else {
        try {
            $acquired = Acquire-IsolatedNative $RequestedVersion
        } catch {
            Write-Diagnostic "isolated acquisition unavailable: $($_.Exception.Message); checking the exact installed global package"
            $acquired = Find-InstalledNative $RequestedVersion
            if ($null -eq $acquired) { throw }
            $acquired | Add-Member -NotePropertyName TemporaryRoot -NotePropertyValue $null
        }
    }

    try {
        # Revalidate both members here even when acquisition already checked
        # them. Repair must not mutate the destination from a partial bundle.
        $acquired.Path = Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($acquired.Path))
        $acquired.CodeModeHostPath = Assert-NativeWindowsExecutable ([IO.Path]::GetFullPath($acquired.CodeModeHostPath))
        $reported = Get-ExactVersion $acquired.Path
        if ($reported -ne "codex-cli $RequestedVersion") {
            Fail "source native binary version mismatch: $reported"
        }
        $acquired | Add-Member -NotePropertyName ReportedVersion -NotePropertyValue $reported -Force
        $acquired | Add-Member -NotePropertyName SourceHash -NotePropertyValue (Get-Hash $acquired.Path) -Force
        $acquired | Add-Member -NotePropertyName SourceCodeModeHostHash -NotePropertyValue (Get-Hash $acquired.CodeModeHostPath) -Force
        return $acquired
    } catch {
        if ($null -ne $acquired -and -not [string]::IsNullOrWhiteSpace($acquired.TemporaryRoot) -and (Test-Path -LiteralPath $acquired.TemporaryRoot)) {
            Remove-PathWithRetry $acquired.TemporaryRoot
        }
        throw
    }
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
        $codeModeHost = Get-CodeModeHostForBinary $binary
        if ($null -eq $codeModeHost) { Fail "isolated npm package did not contain the Windows x64 Codex code-mode host" }
        $reported = Get-ExactVersion $binary
        if ($reported -ne "codex-cli $RequestedVersion") { Fail "isolated native binary version mismatch: $reported" }
        $staged = Join-Path $temporaryRoot $BinaryName
        $stagedCodeModeHost = Join-Path $temporaryRoot $CodeModeHostName
        [IO.File]::Copy($binary, $staged, $true)
        [IO.File]::Copy($codeModeHost, $stagedCodeModeHost, $true)
        return [pscustomobject]@{ Path = $staged; CodeModeHostPath = $stagedCodeModeHost; Source = 'npm-isolated'; SourcePackage = "@openai/codex@$platformPackageVersion"; TemporaryRoot = $temporaryRoot }
    } catch {
        if (Test-Path -LiteralPath $temporaryRoot) { Remove-PathWithRetry $temporaryRoot }
        throw
    }
}

function Read-Manifest([string]$ManifestPath) {
    try { $manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json } catch { Fail "manifest is not valid JSON: $ManifestPath" }
    $allowed = @('manifest_version', 'version', 'reported_version', 'sha256', 'platform', 'architecture', 'binary', 'code_mode_host', 'code_mode_host_sha256', 'source', 'source_package', 'archived_at_utc')
    foreach ($property in $manifest.PSObject.Properties.Name) {
        if ($property -notin $allowed) { Fail "manifest has unsupported property: $property" }
    }
    foreach ($property in @('manifest_version', 'version', 'reported_version', 'sha256', 'platform', 'architecture', 'binary', 'code_mode_host', 'code_mode_host_sha256', 'source', 'source_package', 'archived_at_utc')) {
        if ($null -eq $manifest.$property -or [string]::IsNullOrWhiteSpace([string]$manifest.$property)) { Fail "manifest is missing $property" }
    }
    return $manifest
}

function Invoke-TestFault([string]$Point) {
    if ($TestFault -eq $Point) {
        Fail "deterministic test fault at $Point"
    }
}

function Remove-PathWithRetry([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path)) { return }
    $lastError = $null
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        try {
            Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
            return
        } catch {
            $lastError = $_.Exception.Message
            if ($attempt -lt 30) { Start-Sleep -Milliseconds 100 }
        }
    }
    Fail "could not remove temporary baseline path after bounded retry: $Path ($lastError)"
}

function Move-DirectoryWithRetry([string]$Source, [string]$Destination) {
    $lastError = $null
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        try {
            [IO.Directory]::Move($Source, $Destination)
            return
        } catch {
            $lastError = $_.Exception.Message
            if ($attempt -lt 30) { Start-Sleep -Milliseconds 100 }
        }
    }
    Fail "could not move baseline directory after bounded retry: $Source -> $Destination ($lastError)"
}

function Assert-BaselineDirectory([string]$Directory) {
    $item = Get-Item -LiteralPath $Directory -Force -ErrorAction Stop
    if (-not ($item -is [IO.DirectoryInfo])) { Fail "baseline path must be a directory: $Directory" }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { Fail "baseline directory must not be a reparse point: $Directory" }
    return [IO.Path]::GetFullPath($item.FullName)
}

function Assert-RegularBaselineFile([string]$Path, [string]$Description) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if (-not ($item -is [IO.FileInfo])) { Fail "$Description must be a regular file: $Path" }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { Fail "$Description must not be a reparse point: $Path" }
    return [IO.Path]::GetFullPath($item.FullName)
}

function Verify-BaselineDirectory([string]$RequestedVersion, [string]$Directory) {
    Assert-Version $RequestedVersion
    if (-not (Test-WindowsX64Host)) {
        Fail 'Windows x64 baseline verification is required for this baseline store'
    }
    $directory = Assert-BaselineDirectory $Directory
    $manifestPath = Join-Path $directory 'manifest.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { Fail "baseline manifest does not exist: $RequestedVersion" }
    $manifestPath = Assert-RegularBaselineFile $manifestPath 'baseline manifest'
    $manifest = Read-Manifest $manifestPath
    if ([int]$manifest.manifest_version -ne $ManifestVersion) { Fail "unsupported manifest version: $($manifest.manifest_version)" }
    if ($manifest.version -ne $RequestedVersion -or $manifest.reported_version -ne "codex-cli $RequestedVersion") { Fail "manifest version does not match requested version: $RequestedVersion" }
    if ($manifest.platform -ne $Platform -or $manifest.architecture -ne $Architecture) { Fail "baseline is not compatible with Windows x64" }
    if ($manifest.binary -ne $BinaryName) { Fail "baseline binary must be $BinaryName" }
    if ($manifest.code_mode_host -ne $CodeModeHostName) { Fail "baseline code-mode host must be $CodeModeHostName" }
    if ($manifest.sha256 -notmatch '^[0-9a-f]{64}$') { Fail 'manifest SHA-256 must be lowercase hexadecimal' }
    if ($manifest.code_mode_host_sha256 -notmatch '^[0-9a-f]{64}$') { Fail 'manifest code-mode host SHA-256 must be lowercase hexadecimal' }
    $binaryPath = [IO.Path]::GetFullPath((Join-Path $directory $manifest.binary))
    $codeModeHostPath = [IO.Path]::GetFullPath((Join-Path $directory $manifest.code_mode_host))
    if ((Split-Path -Parent $binaryPath) -ne $directory) { Fail 'baseline binary must be inside the certified directory' }
    if ((Split-Path -Parent $codeModeHostPath) -ne $directory) { Fail 'baseline code-mode host must be inside the certified directory' }
    $binary = Assert-NativeWindowsExecutable $binaryPath
    $codeModeHost = Assert-NativeWindowsExecutable $codeModeHostPath
    if ((Get-Hash $binary) -ne $manifest.sha256) { Fail "baseline SHA-256 mismatch: $RequestedVersion" }
    if ((Get-Hash $codeModeHost) -ne $manifest.code_mode_host_sha256) { Fail "baseline code-mode host SHA-256 mismatch: $RequestedVersion" }
    if ((Get-ExactVersion $binary) -ne $manifest.reported_version) { Fail "baseline native binary version mismatch: $RequestedVersion" }
    return [pscustomobject]@{ Binary = $binary; CodeModeHost = $codeModeHost; Manifest = $manifest; ManifestPath = $manifestPath }
}

function Verify-Baseline([string]$RequestedVersion) {
    return Verify-BaselineDirectory $RequestedVersion (Get-BaselineDirectory $RequestedVersion)
}

function Get-DestinationState([string]$RequestedVersion) {
    $directory = Get-BaselineDirectory $RequestedVersion
    if (-not (Test-Path -LiteralPath $directory)) {
        return [pscustomobject]@{ Classification = 'Absent'; Directory = $directory; Verified = $null }
    }
    try {
        $verified = Verify-BaselineDirectory $RequestedVersion $directory
        return [pscustomobject]@{ Classification = 'ValidCurrent'; Directory = $directory; Verified = $verified }
    } catch {
        $manifestPath = Join-Path $directory 'manifest.json'
        if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
            try {
                $rawManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
                if ([int]$rawManifest.manifest_version -eq 1) {
                    return [pscustomobject]@{ Classification = 'LegacyManifest'; Directory = $directory; Verified = $null }
                }
            } catch { }
            return [pscustomobject]@{ Classification = 'InvalidCurrent'; Directory = $directory; Verified = $null }
        }
        return [pscustomobject]@{ Classification = 'Incomplete'; Directory = $directory; Verified = $null }
    }
}

function New-StagedBaseline([string]$Root, [string]$RequestedVersion, [object]$Acquired) {
    $staging = Join-Path $Root (".$RequestedVersion-repair-staging-" + [guid]::NewGuid().ToString('N'))
    [IO.Directory]::CreateDirectory($staging) | Out-Null
    try {
        $persisted = Join-Path $staging $BinaryName
        $persistedCodeModeHost = Join-Path $staging $CodeModeHostName
        [IO.File]::Copy($Acquired.Path, $persisted, $false)
        [IO.File]::Copy($Acquired.CodeModeHostPath, $persistedCodeModeHost, $false)
        if ((Get-Hash $persisted) -ne $Acquired.SourceHash) { Fail 'persisted baseline SHA-256 differs from source' }
        if ((Get-Hash $persistedCodeModeHost) -ne $Acquired.SourceCodeModeHostHash) { Fail 'persisted code-mode host SHA-256 differs from source' }
        $manifest = [ordered]@{
            manifest_version = $ManifestVersion
            version = $RequestedVersion
            reported_version = $Acquired.ReportedVersion
            sha256 = $Acquired.SourceHash
            platform = $Platform
            architecture = $Architecture
            binary = $BinaryName
            code_mode_host = $CodeModeHostName
            code_mode_host_sha256 = $Acquired.SourceCodeModeHostHash
            source = $Acquired.Source
            source_package = $Acquired.SourcePackage
            archived_at_utc = [DateTime]::UtcNow.ToString('o')
        }
        [IO.File]::WriteAllText((Join-Path $staging 'manifest.json'), ($manifest | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
        $null = Verify-BaselineDirectory $RequestedVersion $staging
        Invoke-TestFault 'staged-verification'
        return $staging
    } catch {
        if (Test-Path -LiteralPath $staging) { Remove-PathWithRetry $staging }
        throw
    }
}

function Save-Baseline([string]$RequestedVersion) {
    Assert-Version $RequestedVersion
    $acquired = $null
    try {
        $acquired = Get-SourceBundle $RequestedVersion
        $root = Get-StoreRoot
        [IO.Directory]::CreateDirectory($root) | Out-Null
        $destination = Join-Path $root $RequestedVersion
        if (Test-Path -LiteralPath $destination) {
            $verified = Verify-Baseline $RequestedVersion
            if ($verified.Manifest.sha256 -eq $acquired.SourceHash -and $verified.Manifest.code_mode_host_sha256 -eq $acquired.SourceCodeModeHostHash) {
                Write-Diagnostic "baseline $RequestedVersion already exists with the same SHA-256"
                return
            }
            Fail "baseline $RequestedVersion already exists with a different SHA-256"
        }
        $staging = $null
        try {
            $staging = New-StagedBaseline $root $RequestedVersion $acquired
            Move-DirectoryWithRetry $staging $destination
            $staging = $null
        } finally {
            if ($null -ne $staging -and (Test-Path -LiteralPath $staging)) { Remove-PathWithRetry $staging }
        }
        $null = Verify-Baseline $RequestedVersion
        Write-Diagnostic "saved verified baseline $RequestedVersion"
    } finally {
        if ($null -ne $acquired -and -not [string]::IsNullOrWhiteSpace($acquired.TemporaryRoot) -and (Test-Path -LiteralPath $acquired.TemporaryRoot)) {
            Remove-PathWithRetry $acquired.TemporaryRoot
        }
    }
}

function Repair-Baseline([string]$RequestedVersion) {
    Assert-Version $RequestedVersion
    $root = Get-StoreRoot
    [IO.Directory]::CreateDirectory($root) | Out-Null
    $state = Get-DestinationState $RequestedVersion
    $acquired = $null
    $staging = $null
    $backup = $null
    $backupMoved = $false
    $replacementMoved = $false
    try {
        $acquired = Get-SourceBundle $RequestedVersion
        if ($state.Classification -eq 'ValidCurrent') {
            if ($state.Verified.Manifest.sha256 -eq $acquired.SourceHash -and $state.Verified.Manifest.code_mode_host_sha256 -eq $acquired.SourceCodeModeHostHash) {
                Write-Diagnostic "baseline $RequestedVersion is already valid; no replacement needed"
                return
            }
            Fail "baseline $RequestedVersion is valid with different SHA-256; refusing replacement"
        }

        $staging = New-StagedBaseline $root $RequestedVersion $acquired
        if ($state.Classification -ne 'Absent') {
            $backup = Join-Path $root (".$RequestedVersion-repair-backup-" + [guid]::NewGuid().ToString('N'))
            Invoke-TestFault 'destination-to-backup'
            Move-DirectoryWithRetry $state.Directory $backup
            $backupMoved = $true
        }
        Invoke-TestFault 'staging-to-destination'
        Move-DirectoryWithRetry $staging $state.Directory
        $staging = $null
        $replacementMoved = $true
        $null = Verify-Baseline $RequestedVersion
        if ($backupMoved) {
            Remove-PathWithRetry $backup
            $backup = $null
        }
        Write-Diagnostic "repaired verified baseline $RequestedVersion"
    } catch {
        $failure = $_
        $restoreError = $null
        if ($backupMoved) {
            try {
                if ($replacementMoved -and (Test-Path -LiteralPath $state.Directory)) {
                    Remove-PathWithRetry $state.Directory
                }
                if ((Test-Path -LiteralPath $backup) -and -not (Test-Path -LiteralPath $state.Directory)) {
                    Move-DirectoryWithRetry $backup $state.Directory
                    $backup = $null
                    Write-Diagnostic "repair failed; original baseline restored at $RequestedVersion"
                } elseif (Test-Path -LiteralPath $state.Directory) {
                    $restoreError = 'destination exists and backup could not be restored'
                } else {
                    $restoreError = 'backup is missing and original baseline could not be restored'
                }
            } catch {
                $restoreError = $_.Exception.Message
            }
        }
        if ($null -ne $restoreError) {
            throw "baseline repair failed: $($failure.Exception.Message); bounded restoration failed: $restoreError"
        }
        throw $failure.Exception
    } finally {
        if ($null -ne $staging -and (Test-Path -LiteralPath $staging)) { Remove-PathWithRetry $staging }
        if ($null -ne $acquired -and -not [string]::IsNullOrWhiteSpace($acquired.TemporaryRoot) -and (Test-Path -LiteralPath $acquired.TemporaryRoot)) {
            Remove-PathWithRetry $acquired.TemporaryRoot
        }
    }
}

try {
    switch ($Command) {
        'save' { if ($null -eq $Version) { Fail 'save requires a version' }; Save-Baseline $Version }
        'verify' { if ($null -eq $Version) { Fail 'verify requires a version' }; $null = Verify-Baseline $Version; Write-Diagnostic "verified baseline $Version" }
        'repair' { if ($null -eq $Version) { Fail 'repair requires a version' }; Repair-Baseline $Version }
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
