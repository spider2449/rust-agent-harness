[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$NativeCodex
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$baselineScript = Join-Path $PSScriptRoot 'codex-baseline.ps1'
$store = Join-Path ([IO.Path]::GetTempPath()) ('codex-baseline-test-' + [guid]::NewGuid().ToString('N'))
$sourceNative = [IO.Path]::GetFullPath($NativeCodex)
$sourceCodeModeHost = Join-Path (Split-Path -Parent $sourceNative) 'codex-code-mode-host.exe'
$version = ((& $sourceNative --version).Trim() -replace '^codex-cli ', '')
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'NativeCodex did not report an exact Codex version' }
if (-not (Test-Path -LiteralPath $sourceCodeModeHost -PathType Leaf)) { throw 'NativeCodex sibling code-mode host is required' }

function Invoke-Baseline([string[]]$Arguments) {
    $named = @{}
    for ($index = 2; $index -lt $Arguments.Count; $index += 2) {
        if ($Arguments[$index] -notmatch '^-[A-Za-z]+$' -or $index + 1 -ge $Arguments.Count) { throw "invalid baseline test argument vector: $($Arguments -join ' ')" }
        $named[$Arguments[$index].Substring(1)] = $Arguments[$index + 1]
    }
    & $baselineScript -Command $Arguments[0] -Version $Arguments[1] @named *> $null
    return $LASTEXITCODE
}

function Assert-Succeeds([string[]]$Arguments, [string]$Message) {
    $exitCode = Invoke-Baseline $Arguments
    if ($exitCode -ne 0) { throw $Message }
}

function Assert-Fails([string[]]$Arguments, [string]$Message) {
    $exitCode = Invoke-Baseline $Arguments
    if ($exitCode -eq 0) { throw $Message }
}

function Reset-Store {
    if (Test-Path -LiteralPath $store) { Remove-Item -LiteralPath $store -Recurse -Force }
    [IO.Directory]::CreateDirectory($store) | Out-Null
}

function Source-Arguments([string]$Path = $sourceNative, [string]$CodeModeHostPath = $sourceCodeModeHost) {
    return @('-SourcePath', $Path, '-SourceCodeModeHostPath', $CodeModeHostPath)
}

function Save-Valid([string]$RequestedVersion = $version) {
    Assert-Succeeds (@('save', $RequestedVersion) + (Source-Arguments)) 'save failed'
}

function Get-BaselinePath([string]$RequestedVersion = $version) {
    return (Join-Path $store $RequestedVersion)
}

function Get-Manifest([string]$RequestedVersion = $version) {
    return Get-Content -LiteralPath (Join-Path (Get-BaselinePath $RequestedVersion) 'manifest.json') -Raw | ConvertFrom-Json
}

function Get-ManifestText([string]$RequestedVersion = $version) {
    return [IO.File]::ReadAllText((Join-Path (Get-BaselinePath $RequestedVersion) 'manifest.json'))
}

function Assert-NoRepairArtifacts {
    if (-not (Test-Path -LiteralPath $store -PathType Container)) { return }
    $artifacts = @(Get-ChildItem -LiteralPath $store -Force -Directory | Where-Object { $_.Name -match '^\..+-repair-(staging|backup)-' })
    if ($artifacts.Count -ne 0) { throw "repair artifacts remain: $($artifacts.Name -join ', ')" }
}

function Prepare-Legacy([string]$RequestedVersion = $version) {
    if ($RequestedVersion -eq $version) {
        Save-Valid $RequestedVersion
    } else {
        Save-Valid $version
        Move-Item -LiteralPath (Get-BaselinePath $version) -Destination (Get-BaselinePath $RequestedVersion)
    }
    $directory = Get-BaselinePath $RequestedVersion
    $manifest = Get-Manifest $RequestedVersion
    Remove-Item -LiteralPath (Join-Path $directory 'codex-code-mode-host.exe') -Force
    $legacy = [ordered]@{
        manifest_version = 1
        version = $manifest.version
        reported_version = $manifest.reported_version
        sha256 = $manifest.sha256
        platform = $manifest.platform
        architecture = $manifest.architecture
        binary = $manifest.binary
    }
    [IO.File]::WriteAllText((Join-Path $directory 'manifest.json'), ($legacy | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
}

function Copy-WithTrailingByte([string]$Source, [string]$Destination) {
    $original = [IO.File]::ReadAllBytes($Source)
    $copy = New-Object byte[] ($original.Length + 1)
    [Array]::Copy($original, $copy, $original.Length)
    $copy[$original.Length] = 0
    [IO.File]::WriteAllBytes($Destination, $copy)
}

try {
    $env:CODEX_BASELINE_HOME = $store

    # 1. Absent destination: explicit repair creates and verifies a v2 pair.
    Reset-Store
    Assert-Succeeds (@('repair', $version) + (Source-Arguments)) 'repair of absent baseline failed'
    Assert-Succeeds @('verify', $version) 'repaired absent baseline did not verify'
    if ((Get-Manifest).manifest_version -ne 2) { throw 'absent repair did not write manifest v2' }
    Assert-NoRepairArtifacts

    # 2. Valid destination: repair is idempotent and leaves hashes unchanged.
    $before = Get-Manifest
    $beforeBinaryHash = (Get-FileHash -LiteralPath (Join-Path (Get-BaselinePath) 'codex.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    $beforeHostHash = (Get-FileHash -LiteralPath (Join-Path (Get-BaselinePath) 'codex-code-mode-host.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    Assert-Succeeds (@('repair', $version) + (Source-Arguments)) 'repair of valid baseline failed'
    $after = Get-Manifest
    if ($before.sha256 -ne $after.sha256 -or $before.code_mode_host_sha256 -ne $after.code_mode_host_sha256) { throw 'idempotent repair changed manifest hashes' }
    if ($beforeBinaryHash -ne (Get-FileHash -LiteralPath (Join-Path (Get-BaselinePath) 'codex.exe') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'idempotent repair changed codex hash' }
    if ($beforeHostHash -ne (Get-FileHash -LiteralPath (Join-Path (Get-BaselinePath) 'codex-code-mode-host.exe') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'idempotent repair changed code-mode host hash' }
    Assert-NoRepairArtifacts

    # 3. Exact Task 217 blocker: v1 legacy store is rejected, then repaired
    # only from a complete independently supplied pair.
    Reset-Store
    Prepare-Legacy
    Assert-Fails @('verify', $version) 'legacy v1 baseline was accepted before repair'
    Assert-Succeeds (@('repair', $version) + (Source-Arguments)) 'legacy v1 repair failed'
    Assert-Succeeds @('verify', $version) 'legacy v1 repair did not verify'
    if ((Get-Manifest).manifest_version -ne 2) { throw 'legacy repair did not write manifest v2' }
    if (-not (Test-Path -LiteralPath (Join-Path (Get-BaselinePath) 'codex-code-mode-host.exe') -PathType Leaf)) { throw 'legacy repair omitted code-mode host' }
    Assert-NoRepairArtifacts

    # 4-5. Missing companion and malformed manifest are repairable only by the
    # explicit command; verify remains fail-closed.
    Reset-Store
    Save-Valid
    Remove-Item -LiteralPath (Join-Path (Get-BaselinePath) 'codex-code-mode-host.exe') -Force
    Assert-Fails @('verify', $version) 'missing code-mode host was accepted'
    Assert-Succeeds (@('repair', $version) + (Source-Arguments)) 'missing code-mode host repair failed'
    Assert-Succeeds @('verify', $version) 'missing code-mode host repair did not verify'

    Reset-Store
    Save-Valid
    [IO.File]::WriteAllText((Join-Path (Get-BaselinePath) 'manifest.json'), '{')
    Assert-Fails @('verify', $version) 'malformed manifest was accepted'
    Assert-Succeeds (@('repair', $version) + (Source-Arguments)) 'malformed manifest repair failed'
    Assert-Succeeds @('verify', $version) 'malformed manifest repair did not verify'

    # 6. Wrong source version preserves the old destination.
    $wrongVersion = '9.9.9'
    Reset-Store
    Prepare-Legacy $wrongVersion
    $oldWrongVersionManifest = Get-ManifestText $wrongVersion
    Assert-Fails (@('repair', $wrongVersion) + (Source-Arguments)) 'wrong source version was accepted'
    if ($oldWrongVersionManifest -ne (Get-ManifestText $wrongVersion)) { throw 'wrong source version mutated destination' }

    # 7-8. Non-PE source and invalid source code-mode host are rejected before
    # destination mutation.
    Reset-Store
    Prepare-Legacy
    $oldManifest = Get-ManifestText
    $nonPe = Join-Path $store 'not-an-executable.exe'
    [IO.File]::WriteAllBytes($nonPe, [byte[]](0x6e, 0x6f, 0x74, 0x2d, 0x70, 0x65))
    Assert-Fails (@('repair', $version) + (Source-Arguments $nonPe $sourceCodeModeHost)) 'non-PE source was accepted'
    if ($oldManifest -ne (Get-ManifestText)) { throw 'non-PE source mutated destination' }

    Reset-Store
    Prepare-Legacy
    $oldManifest = Get-ManifestText
    $invalidHost = Join-Path $store 'invalid-code-mode-host.exe'
    [IO.File]::WriteAllBytes($invalidHost, [byte[]](0x4d, 0x5a, 0x00))
    Assert-Fails (@('repair', $version) + (Source-Arguments $sourceNative $invalidHost)) 'invalid source code-mode host was accepted'
    if ($oldManifest -ne (Get-ManifestText)) { throw 'invalid source code-mode host mutated destination' }

    # 9-11. Staged verification, first move, and second move failures preserve
    # or restore the old destination and do not leave successful-operation junk.
    Reset-Store
    Prepare-Legacy
    $oldManifest = Get-ManifestText
    Assert-Fails (@('repair', $version) + (Source-Arguments) + @('-TestFault', 'staged-verification')) 'staged verification fault unexpectedly succeeded'
    if ($oldManifest -ne (Get-ManifestText)) { throw 'staged verification failure changed destination' }
    Assert-NoRepairArtifacts

    Reset-Store
    Prepare-Legacy
    $oldManifest = Get-ManifestText
    Assert-Fails (@('repair', $version) + (Source-Arguments) + @('-TestFault', 'destination-to-backup')) 'destination-to-backup fault unexpectedly succeeded'
    if ($oldManifest -ne (Get-ManifestText)) { throw 'first move failure changed destination' }
    Assert-NoRepairArtifacts

    Reset-Store
    Prepare-Legacy
    $oldManifest = Get-ManifestText
    Assert-Fails (@('repair', $version) + (Source-Arguments) + @('-TestFault', 'staging-to-destination')) 'staging-to-destination fault unexpectedly succeeded'
    if ($oldManifest -ne (Get-ManifestText)) { throw 'second move failure did not restore destination' }
    Assert-Fails @('verify', $version) 'restored legacy destination was falsely verified'
    Assert-NoRepairArtifacts

    # 12. A valid certified destination cannot be replaced by a different
    # acquired pair, even when the new pair reports the requested version.
    Reset-Store
    Save-Valid
    $differentNative = Join-Path $store 'different-codex.exe'
    Copy-WithTrailingByte $sourceNative $differentNative
    $validManifestBefore = Get-ManifestText
    Assert-Fails (@('repair', $version) + (Source-Arguments $differentNative $sourceCodeModeHost)) 'valid baseline accepted a different source pair'
    if ($validManifestBefore -ne (Get-ManifestText)) { throw 'valid baseline was replaced by different source hashes' }
    Assert-Succeeds @('verify', $version) 'valid baseline was damaged by replacement refusal'

    # 13. Destination hash corruption is repaired and all temporary siblings
    # are cleaned after success.
    Reset-Store
    Save-Valid
    $manifestPath = Join-Path (Get-BaselinePath) 'manifest.json'
    $corrupt = Get-Manifest
    $corrupt.sha256 = ('0' * 64)
    [IO.File]::WriteAllText($manifestPath, ($corrupt | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    Assert-Fails @('verify', $version) 'wrong destination hash was accepted'
    Assert-Succeeds (@('repair', $version) + (Source-Arguments)) 'wrong destination hash repair failed'
    Assert-Succeeds @('verify', $version) 'wrong destination hash repair did not verify'
    Assert-NoRepairArtifacts

    'codex-baseline deterministic repair tests passed'
    exit 0
} finally {
    Remove-Item Env:CODEX_BASELINE_HOME -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $store) { Remove-Item -LiteralPath $store -Recurse -Force }
}
