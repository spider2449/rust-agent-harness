<#
.SYNOPSIS
Starts RAH: the Web UI (default), the Web UI in a desktop window, or the
terminal REPL.

.DESCRIPTION
Builds and runs the `rah` binary from this repository with cargo. With the
Web UI, the script opens the printed URL (it carries the per-start access
token) in the default browser. With -Desktop it runs `rah-desktop` instead,
which shows the same Web UI in a native window. With the `default` preset it
first checks that llama-server answers, and only warns if it does not.

.EXAMPLE
.\scripts\start.ps1
Web UI with your own `rah.toml` in the data directory (`~\.rah`) when it
exists, otherwise with llama.cpp (the `default` preset).

.EXAMPLE
.\scripts\start.ps1 -Preset dev
Web UI with the mock model; needs no model server.

.EXAMPLE
.\scripts\start.ps1 -Desktop -Mode coding
The Web UI in a native window, in the coding mode.

.EXAMPLE
.\scripts\start.ps1 -Cli -Root D:\work\my-repo
Terminal REPL, with file and Git tools rooted at another repository.

.EXAMPLE
.\scripts\start.ps1 -InitConfig -Preset openai
Writes the preset as `rah.toml` in the data directory to edit, and exits;
an existing file is never overwritten.

.EXAMPLE
.\scripts\start.ps1 -Config .\rah.toml -Release
Your own plugin composition, built with optimizations.
#>
[CmdletBinding()]
param(
    # Built-in preset: default (llama.cpp), dev (mock model), minimal, openai.
    # Without it (and -Config), rah.toml in the data directory runs when it
    # exists, otherwise `default`.
    [ValidateSet("default", "dev", "minimal", "openai")]
    [string]$Preset,
    # A distribution file; overrides -Preset.
    [string]$Config,
    # Model id for model-llamacpp or model-openai.
    [string]$Model,
    # Workspace root for file tools and processes (default: current directory).
    [string]$Root,
    # Data directory for sessions, settings and the rest (default: RAH_HOME,
    # else ~\.rah).
    [string]$DataDir,
    # Session log directory.
    [string]$SessionDir,
    # Web UI address, loopback only.
    [string]$Bind = "127.0.0.1:8765",
    # The terminal REPL instead of the Web UI.
    [switch]$Cli,
    # The Web UI in a native window (rah-desktop) instead of the browser.
    [switch]$Desktop,
    # Mode to enter at start, such as daily or coding, instead of the last
    # one used.
    [string]$Mode,
    # Do not open the browser.
    [switch]$NoBrowser,
    # Build with optimizations (slower first build, faster runs).
    [switch]$Release,
    # llama-server address checked before starting the `default` preset.
    [string]$LlamaServer = "http://127.0.0.1:8080",
    # Write the preset (-Preset, else `default`) as rah.toml in the data
    # directory, or as -Config, and exit.
    [switch]$InitConfig
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

if ($Desktop -and $Cli) { throw "-Desktop and -Cli cannot be combined." }
if ($Cli -and $Mode) { throw "-Mode needs the Web UI; it cannot be combined with -Cli." }
if ($Desktop) {
    # rah-desktop takes what the distribution and the window need, and picks
    # its own port.
    foreach ($name in "Model", "SessionDir", "Bind") {
        if ($PSBoundParameters.ContainsKey($name)) { throw "-$name is not supported with -Desktop." }
    }
}
$package = if ($Desktop) { "rah-desktop" } else { "rah-cli" }

$cargoArgs = @("run", "--quiet", "-p", $package)
if ($Release) { $cargoArgs += "--release" }
$cargoArgs += "--"
# Relative to where the script was called from, not the repository it runs in.
if ($DataDir) { $DataDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($DataDir) }
$dataHome = if ($DataDir) { $DataDir } elseif ($env:RAH_HOME) { $env:RAH_HOME } else { Join-Path $env:USERPROFILE ".rah" }
$userConfig = Join-Path $dataHome "rah.toml"
# The preset that runs: none with a configuration file.
$runPreset = if ($Config) { "" } elseif ($Preset) { $Preset } elseif (Test-Path -LiteralPath $userConfig -PathType Leaf) { "" } else { "default" }
if ($Config) { $cargoArgs += @("--config", $Config) } elseif ($Preset) { $cargoArgs += @("--preset", $Preset) }
if ($DataDir) { $cargoArgs += @("--data-dir", $DataDir) }
if ($Model) { $cargoArgs += @("--model", $Model) }
if ($Root) { $cargoArgs += @("--root", $Root) }
if ($SessionDir) { $cargoArgs += @("--session-dir", $SessionDir) }
if ($Mode) { $cargoArgs += @("--mode", $Mode) }
if (-not $Cli -and -not $Desktop) { $cargoArgs += @("--web", "--bind", $Bind) }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH; install Rust from https://rustup.rs"
}

if ($InitConfig) {
    $initArgs = @("run", "--quiet", "-p", "rah-cli")
    if ($Release) { $initArgs += "--release" }
    $initArgs += @("--", "--init-config")
    if ($Preset) { $initArgs += @("--preset", $Preset) }
    # Relative to where the script was called from, like -DataDir.
    if ($Config) { $initArgs += @("--config", $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Config)) }
    if ($DataDir) { $initArgs += @("--data-dir", $DataDir) }
    Push-Location $repo
    try {
        & cargo @initArgs
        exit $LASTEXITCODE
    } finally {
        Pop-Location
    }
}

if ($runPreset -eq "default") {
    try {
        Invoke-WebRequest -Uri "$LlamaServer/health" -UseBasicParsing -TimeoutSec 3 | Out-Null
        Write-Host "llama-server is up at $LlamaServer"
    } catch {
        Write-Warning "llama-server does not answer at $LlamaServer/health. Start it first (for example: llama-server -m model.gguf --port 8080 --jinja), or use -Preset dev."
    }
}
if ($runPreset -eq "openai" -and -not $env:OPENAI_API_KEY) {
    Write-Warning "OPENAI_API_KEY is not set; the openai preset needs it."
}

Push-Location $repo
try {
    if ($Cli -or $Desktop) {
        # The REPL needs the console's stdin and stdout as they are; the
        # desktop window needs no URL opened.
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
