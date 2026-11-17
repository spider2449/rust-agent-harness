# Deterministic selection checks; no build, downloads or installed profile changes.
$ErrorActionPreference = 'Stop'
$tokens = $null; $errors = $null
$scriptPath = Join-Path $PSScriptRoot 'package.ps1'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw ($errors | Out-String) }
$definition = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Resolve-PackageRuntimes' }, $true)
Invoke-Expression $definition.Extent.Text
$root = Join-Path ([IO.Path]::GetTempPath()) "rah-package-test-$([guid]::NewGuid().ToString('N'))"
try {
    $graph = Join-Path $root '.rah/graphify-runtime'
    $preferred = Join-Path $root '.rah/graphify-no-uv-runtime'
    $lsp = Join-Path $root '.rah/lsp-runtime'
    foreach ($path in @($graph, $lsp)) {
        New-Item -ItemType Directory -Path $path -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $path 'runtime.json') -Value '{}'
    }
    $result = Resolve-PackageRuntimes $root '' '' $false
    if ($result.Graphify -ne $graph -or $result.Lsp -ne $lsp) { throw 'Default discovery failed' }
    New-Item -ItemType Directory -Path $preferred | Out-Null
    Set-Content -LiteralPath (Join-Path $preferred 'runtime.json') -Value '{}'
    if ((Resolve-PackageRuntimes $root '' '' $false).Graphify -ne $preferred) { throw 'Preferred Graphify runtime not selected' }
    if ((Resolve-PackageRuntimes $root $graph '' $false).Graphify -ne $graph) { throw 'Explicit override ignored' }
    $result = Resolve-PackageRuntimes (Join-Path $root 'missing') '' '' $true
    if ($result.Graphify -or $result.Lsp) { throw 'Lightweight selected runtimes' }
    $refused = $false
    try { Resolve-PackageRuntimes $root $graph '' $true } catch { $refused = $true }
    if (-not $refused) { throw 'Conflicting lightweight options accepted' }
    $refused = $false
    try { Resolve-PackageRuntimes $root '' (Join-Path $root 'missing') $false } catch { $refused = $true }
    if (-not $refused) { throw 'Missing explicit runtime silently fell back' }
    Remove-Item -LiteralPath (Join-Path $lsp 'runtime.json')
    $refused = $false
    try { Resolve-PackageRuntimes $root '' '' $false } catch { $refused = $true }
    if (-not $refused) { throw 'Incomplete default bundle accepted' }
    Write-Output 'PASS: 7 package runtime selection checks; PowerShell syntax valid'
} finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ([IO.Path]::GetDirectoryName($resolved).TrimEnd('\') -ne [IO.Path]::GetTempPath().TrimEnd('\') -or [IO.Path]::GetFileName($resolved) -notlike 'rah-package-test-*') { throw 'Unexpected test cleanup path' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
