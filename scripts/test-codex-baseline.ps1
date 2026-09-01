[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$NativeCodex
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'codex-baseline.ps1'
$store = Join-Path ([IO.Path]::GetTempPath()) ('codex-baseline-test-' + [guid]::NewGuid().ToString('N'))
$version = ((& $NativeCodex --version).Trim() -replace '^codex-cli ', '')
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'NativeCodex did not report an exact Codex version' }
$nativeCodeModeHost = Join-Path (Split-Path -Parent $NativeCodex) 'codex-code-mode-host.exe'
if (-not (Test-Path -LiteralPath $nativeCodeModeHost -PathType Leaf)) { throw 'NativeCodex sibling code-mode host is required' }

try {
    $env:CODEX_BASELINE_HOME = $store
    & $script save $version -SourcePath $NativeCodex -SourceCodeModeHostPath $nativeCodeModeHost
    if ($LASTEXITCODE -ne 0) { throw 'initial save failed' }
    & $script save $version -SourcePath $NativeCodex -SourceCodeModeHostPath $nativeCodeModeHost
    if ($LASTEXITCODE -ne 0) { throw 'idempotent save failed' }
    & $script verify $version
    if ($LASTEXITCODE -ne 0) { throw 'verify failed' }
    $path = & $script path $version
    if ($LASTEXITCODE -ne 0 -or $path -ne (Join-Path $store "$version\codex.exe")) { throw 'path did not return only the verified executable path' }

    $manifest = Join-Path $store "$version\manifest.json"
    $binary = Join-Path $store "$version\codex.exe"
    $codeModeHost = Join-Path $store "$version\codex-code-mode-host.exe"
    $originalManifest = [IO.File]::ReadAllText($manifest)
    [IO.File]::WriteAllText($manifest, '{')
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'corrupt manifest was accepted' }

    [IO.File]::WriteAllText($manifest, $originalManifest)
    Move-Item -LiteralPath $binary -Destination "$binary.missing"
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'missing binary was accepted' }
    Move-Item -LiteralPath "$binary.missing" -Destination $binary

    Move-Item -LiteralPath $codeModeHost -Destination "$codeModeHost.missing"
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'missing code-mode host was accepted' }
    Move-Item -LiteralPath "$codeModeHost.missing" -Destination $codeModeHost

    [IO.File]::WriteAllBytes($codeModeHost, [byte[]](0x4d, 0x5a, 0x00))
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'code-mode host SHA mismatch was accepted' }
    [IO.File]::Copy($nativeCodeModeHost, $codeModeHost, $true)

    $manifestWithUnknownHelperField = $originalManifest.TrimEnd('}', "`r", "`n") + ',"code_mode_host_version":"0.0.0"}'
    [IO.File]::WriteAllText($manifest, $manifestWithUnknownHelperField)
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'unknown helper manifest field was accepted' }
    [IO.File]::WriteAllText($manifest, $originalManifest)

    [IO.File]::WriteAllText($manifest, ($originalManifest -replace '"sha256":\s*"[0-9a-f]+"', '"sha256":"0000000000000000000000000000000000000000000000000000000000000000"'))
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'SHA mismatch was accepted' }

    [IO.File]::WriteAllText($manifest, ($originalManifest -replace '"reported_version":\s*"codex-cli [^"]+"', '"reported_version":"codex-cli 9.9.9"'))
    & $script verify $version
    if ($LASTEXITCODE -eq 0) { throw 'version mismatch was accepted' }
    [IO.File]::WriteAllText($manifest, $originalManifest)

    & $script verify 'not-a-version'
    if ($LASTEXITCODE -eq 0) { throw 'invalid version was accepted' }
    & $script verify '9.9.9'
    if ($LASTEXITCODE -eq 0) { throw 'missing baseline was accepted' }
    'codex-baseline deterministic tests passed'
    exit 0
} finally {
    Remove-Item Env:CODEX_BASELINE_HOME -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $store) { Remove-Item -LiteralPath $store -Recurse -Force }
}
