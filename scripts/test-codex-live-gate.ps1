[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'codex-live-gate.ps1'
$authRoot = Join-Path ([IO.Path]::GetTempPath()) ('rah-live-gate-auth-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($authRoot) | Out-Null
$auth = Join-Path $authRoot 'auth.json'
[IO.File]::WriteAllText($auth, '{"tokens":{"access_token":"test-secret-must-not-appear"}}', [Text.UTF8Encoding]::new($false))

function Invoke-Prepare([string]$ExtraExpectedSha256) {
    $arguments = @{ PrepareOnly = $true; KeepIsolatedHome = $true; AuthSourcePath = $auth }
    if (-not [string]::IsNullOrWhiteSpace($ExtraExpectedSha256)) {
        $arguments.ExpectedSha256 = $ExtraExpectedSha256
    }
    return @(& $script @arguments)
}

try {
    $first = Invoke-Prepare $null
    $homeLine = $first | Where-Object { $_ -like 'RAH_CODEX_LIVE_GATE_TEST_HOME=*' } | Select-Object -First 1
    if ($null -eq $homeLine) { throw 'isolated home was not reported for test cleanup' }
    $isolatedHome = $homeLine.Substring('RAH_CODEX_LIVE_GATE_TEST_HOME='.Length)
    if (-not (Test-Path -LiteralPath $isolatedHome -PathType Container)) { throw 'isolated home was not created' }
    $config = Get-Content -LiteralPath (Join-Path $isolatedHome 'config.toml') -Raw
    foreach ($required in @('model = "gpt-5.4"', 'model_reasoning_effort = "medium"', 'code_mode = false', 'plugins = false')) {
        if (-not $config.Contains($required)) { throw "isolated config omitted $required" }
    }
    if ($config.Contains($env:USERPROFILE) -or $config.Contains('test-secret-must-not-appear')) { throw 'isolated config leaked a host path or secret' }
    if (-not (Test-Path -LiteralPath (Join-Path $isolatedHome 'auth.json') -PathType Leaf)) { throw 'ephemeral auth file was not copied' }
    Remove-Item -LiteralPath $isolatedHome -Recurse -Force
    if (Test-Path -LiteralPath $isolatedHome) { throw 'isolated home cleanup failed' }

    $second = Invoke-Prepare $null
    $firstFingerprint = $first | Where-Object { $_ -like 'RAH_CODEX_CONFIG_SHA256=*' } | Select-Object -First 1
    $secondFingerprint = $second | Where-Object { $_ -like 'RAH_CODEX_CONFIG_SHA256=*' } | Select-Object -First 1
    if ($firstFingerprint -ne $secondFingerprint) { throw 'config fingerprint was not stable' }
    $secondHome = ($second | Where-Object { $_ -like 'RAH_CODEX_LIVE_GATE_TEST_HOME=*' } | Select-Object -First 1).Substring('RAH_CODEX_LIVE_GATE_TEST_HOME='.Length)
    Remove-Item -LiteralPath $secondHome -Recurse -Force

    $failed = $false
    try { $null = Invoke-Prepare ('0' * 64) } catch { $failed = $true }
    if (-not $failed) { throw 'baseline SHA mismatch was accepted' }
    $failed = $false
    try { & $script -Version 9.9.9 -PrepareOnly -AuthSourcePath $auth | Out-Null; if ($LASTEXITCODE -ne 0) { $failed = $true } } catch { $failed = $true }
    if (-not $failed) { throw 'baseline version mismatch was accepted' }
    'codex-live-gate deterministic tests passed'
} finally {
    if (Test-Path -LiteralPath $authRoot) { Remove-Item -LiteralPath $authRoot -Recurse -Force }
}
