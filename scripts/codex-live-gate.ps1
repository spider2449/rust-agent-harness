[CmdletBinding()]
param(
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version = '0.149.0',

    [ValidatePattern('^[0-9a-f]{64}$')]
    [string]$ExpectedSha256 = '14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00',

    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$Model = 'gpt-5.4',

    [ValidateSet('minimal', 'low', 'medium', 'high', 'xhigh')]
    [string]$ReasoningEffort = 'medium',

    [scriptblock]$Command,

    [string]$AuthSourcePath,

    [switch]$PrepareOnly,

    [switch]$KeepIsolatedHome
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Fail([string]$Message) {
    throw "codex-live-gate: $Message"
}

function Assert-RegularFile([string]$Path, [string]$Description) {
    $item = Get-Item -LiteralPath $Path -Force
    if (-not ($item -is [IO.FileInfo])) { Fail "$Description must be a regular file" }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        Fail "$Description must not be a reparse point"
    }
    return $item.FullName
}

function Get-DefaultAuthSourcePath {
    if (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        return (Join-Path $env:USERPROFILE '.codex\auth.json')
    }
    return $null
}

function Write-IsolatedConfig([string]$CodexHomeRoot) {
    # This file intentionally contains no authentication material or host paths.
    $config = @"
model = "$Model"
model_reasoning_effort = "$ReasoningEffort"

[features]
apps = false
browser_use = false
code_mode = false
code_mode_host = false
computer_use = false
enable_mcp_apps = false
image_generation = false
plugins = false
"@
    [IO.File]::WriteAllText(
        (Join-Path $CodexHomeRoot 'config.toml'),
        $config,
        [Text.UTF8Encoding]::new($false)
    )
}

function Get-Fingerprint([string]$BinarySha256) {
    $lines = @(
        'schema=1',
        "codex_version=$Version",
        "codex_binary_sha256=$BinarySha256",
        "model=$Model",
        "reasoning_effort=$ReasoningEffort",
        'codex_home=isolated-temporary',
        'mcp_servers=0',
        'plugins=disabled',
        'apps=disabled',
        'code_mode=disabled',
        'browser_use=disabled',
        'computer_use=disabled',
        'image_generation=disabled',
        'codex_owned_shell=disabled-by-rah-request',
        'codex_owned_file_change=disabled-by-rah-request',
        'codex_owned_web_network=disabled-by-rah-request',
        'approvals=never-by-rah-request'
    )
    $bytes = [Text.Encoding]::UTF8.GetBytes(($lines -join "`n") + "`n")
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        return (($sha256.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') }) -join '')
    } finally {
        $sha256.Dispose()
    }
}

if (-not $PrepareOnly -and $null -eq $Command) {
    Fail 'provide -Command or use -PrepareOnly'
}

$baselineScript = Join-Path $PSScriptRoot 'codex-baseline.ps1'
if (-not (Test-Path -LiteralPath $baselineScript -PathType Leaf)) {
    Fail "missing baseline helper: $baselineScript"
}

$binary = (& $baselineScript path $Version).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($binary)) {
    Fail "baseline verification failed for $Version"
}
$binary = Assert-RegularFile $binary 'certified Codex binary'
$actualSha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha256 -ne $ExpectedSha256) {
    Fail "certified binary SHA-256 mismatch: expected $ExpectedSha256, got $actualSha256"
}

$isolatedHome = Join-Path ([IO.Path]::GetTempPath()) ('rah-codex-live-gate-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($isolatedHome) | Out-Null
$previousCodexHome = $env:CODEX_HOME
$hadCodexHome = Test-Path Env:CODEX_HOME
$previousRahCodexExecutable = $env:RAH_CODEX_EXECUTABLE
$hadRahCodexExecutable = Test-Path Env:RAH_CODEX_EXECUTABLE

try {
    Write-IsolatedConfig $isolatedHome

    $authMode = 'environment'
    if ([string]::IsNullOrWhiteSpace($env:CODEX_ACCESS_TOKEN) -and [string]::IsNullOrWhiteSpace($env:OPENAI_API_KEY)) {
        if ([string]::IsNullOrWhiteSpace($AuthSourcePath)) { $AuthSourcePath = Get-DefaultAuthSourcePath }
        if ([string]::IsNullOrWhiteSpace($AuthSourcePath) -or -not (Test-Path -LiteralPath $AuthSourcePath -PathType Leaf)) {
            Fail 'no supported environment authentication is present and no readable auth.json source was supplied'
        }
        $authSource = Assert-RegularFile ([IO.Path]::GetFullPath($AuthSourcePath)) 'authentication source'
        [IO.File]::Copy($authSource, (Join-Path $isolatedHome 'auth.json'), $false)
        $authMode = 'ephemeral-auth-file-copy'
    }

    $env:CODEX_HOME = $isolatedHome
    $env:RAH_CODEX_EXECUTABLE = $binary
    $fingerprint = Get-Fingerprint $actualSha256
    Write-Output "RAH_CODEX_LIVE_GATE_VERSION=$Version"
    Write-Output "RAH_CODEX_LIVE_GATE_BINARY_SHA256=$actualSha256"
    Write-Output "RAH_CODEX_LIVE_GATE_MODEL=$Model"
    Write-Output "RAH_CODEX_LIVE_GATE_REASONING_EFFORT=$ReasoningEffort"
    Write-Output 'RAH_CODEX_LIVE_GATE_HOME_MODE=isolated-temporary'
    Write-Output 'RAH_CODEX_LIVE_GATE_MCP_SERVERS=0'
    Write-Output 'RAH_CODEX_LIVE_GATE_PLUGINS=disabled'
    Write-Output 'RAH_CODEX_LIVE_GATE_APPS=disabled'
    Write-Output 'RAH_CODEX_LIVE_GATE_CODE_MODE=disabled'
    Write-Output "RAH_CODEX_LIVE_GATE_AUTH_MODE=$authMode"
    Write-Output "RAH_CODEX_CONFIG_SHA256=$fingerprint"
    if ($KeepIsolatedHome) { Write-Output "RAH_CODEX_LIVE_GATE_TEST_HOME=$isolatedHome" }

    if (-not $PrepareOnly) {
        & $Command
        if ($LASTEXITCODE -ne 0) { Fail "live command failed with exit code $LASTEXITCODE" }
    }
} finally {
    if ($hadCodexHome) { $env:CODEX_HOME = $previousCodexHome } else { Remove-Item Env:CODEX_HOME -ErrorAction SilentlyContinue }
    if ($hadRahCodexExecutable) { $env:RAH_CODEX_EXECUTABLE = $previousRahCodexExecutable } else { Remove-Item Env:RAH_CODEX_EXECUTABLE -ErrorAction SilentlyContinue }
    if (-not $KeepIsolatedHome -and (Test-Path -LiteralPath $isolatedHome)) {
        Remove-Item -LiteralPath $isolatedHome -Recurse -Force
    }
}
