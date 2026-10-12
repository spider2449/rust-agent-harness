<#
.SYNOPSIS
Starts RAH: the Web UI (default) or the terminal REPL.

.DESCRIPTION
Builds and runs the `rah` binary from this repository with cargo. With the
Web UI, the script opens the printed URL (it carries the per-start access
token) in the default browser. With the `default` preset it first checks that
llama-server answers, and only warns if it does not.

.EXAMPLE
.\scripts\start.ps1
Web UI with llama.cpp (the `default` preset).

.EXAMPLE
.\scripts\start.ps1 -Preset dev
Web UI with the mock model; needs no model server.

.EXAMPLE
.\scripts\start.ps1 -Cli -Root D:\work\my-repo
Terminal REPL, with file and Git tools rooted at another repository.

.EXAMPLE
.\scripts\start.ps1 -Config .\rah.toml -Release
Your own plugin composition, built with optimizations.
#>
[CmdletBinding()]
param(
    # Built-in preset: default (llama.cpp), dev (mock model), minimal, openai.
    [ValidateSet("default", "dev", "minimal", "openai")]
    [string]$Preset = "default",
    # A distribution file; overrides -Preset.
    [string]$Config,
    # Model id for model-llamacpp or model-openai.
    [string]$Model,
    # Workspace root for file tools and processes (default: current directory).
    [string]$Root,
    # Session log directory.
    [string]$SessionDir,
    # Web UI address, loopback only.
    [string]$Bind = "127.0.0.1:8765",
    # The terminal REPL instead of the Web UI.
    [switch]$Cli,
    # Do not open the browser.
    [switch]$NoBrowser,
    # Build with optimizations (slower first build, faster runs).
    [switch]$Release,
    # llama-server address checked before starting the `default` preset.
    [string]$LlamaServer = "http://127.0.0.1:8080"
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

$cargoArgs = @("run", "--quiet", "-p", "rah-cli")
if ($Release) { $cargoArgs += "--release" }
$cargoArgs += "--"
if ($Config) { $cargoArgs += @("--config", $Config) } else { $cargoArgs += @("--preset", $Preset) }
if ($Model) { $cargoArgs += @("--model", $Model) }
if ($Root) { $cargoArgs += @("--root", $Root) }
if ($SessionDir) { $cargoArgs += @("--session-dir", $SessionDir) }
if (-not $Cli) { $cargoArgs += @("--web", "--bind", $Bind) }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH; install Rust from https://rustup.rs"
}

if (-not $Config -and $Preset -eq "default") {
    try {
        Invoke-WebRequest -Uri "$LlamaServer/health" -UseBasicParsing -TimeoutSec 3 | Out-Null
        Write-Host "llama-server is up at $LlamaServer"
    } catch {
        Write-Warning "llama-server does not answer at $LlamaServer/health. Start it first (for example: llama-server -m model.gguf --port 8080 --jinja), or use -Preset dev."
    }
}
if (-not $Config -and $Preset -eq "openai" -and -not $env:OPENAI_API_KEY) {
    Write-Warning "OPENAI_API_KEY is not set; the openai preset needs it."
}

Push-Location $repo
try {
    if ($Cli) {
        # The REPL needs the console's stdin and stdout as they are.
        & cargo @cargoArgs
        exit $LASTEXITCODE
    }
    # rah prints the URL on stderr. Pass every line through and open the
    # first URL that carries the token. Native stderr lines arrive as error
    # records in Windows PowerShell, so they are converted to text, and the
    # preference is relaxed for this call only.
    $opened = $false
    $ErrorActionPreference = "Continue"
    & cargo @cargoArgs 2>&1 | ForEach-Object {
        $line = "$_"
        Write-Host $line
        if (-not $opened -and -not $NoBrowser -and $line -match '(http://\S+/#token=[0-9a-f]+)') {
            $opened = $true
            Start-Process $Matches[1]
        }
    }
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
