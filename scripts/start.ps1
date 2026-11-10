<#
.SYNOPSIS
Starts RAH: the Web UI (default), the Web UI in a desktop window, or the
terminal REPL.

.DESCRIPTION
Builds and runs the `rah` binary from this repository with cargo. With the
Web UI, the script opens the printed URL (it carries the per-start access
token) in the default browser. With -Desktop it runs `rah-desktop` instead,
which shows the same Web UI in a native window. What runs comes from
`rah.toml` in the data directory; the options override it for one run.

.EXAMPLE
.\scripts\start.ps1 -InitConfig -Model anthropic/claude-sonnet-5-5
Writes `rah.toml` in the data directory (`~\.rah`) and exits; an existing
file is never overwritten.

.EXAMPLE
.\scripts\start.ps1
Web UI with your `rah.toml`.

.EXAMPLE
.\scripts\start.ps1 -Model mock
Web UI with the mock model; needs no model server or key.

.EXAMPLE
.\scripts\start.ps1 -Desktop
The Web UI in a native window.

.EXAMPLE
.\scripts\start.ps1 -Cli -Root D:\work\my-repo -Permissions auto-edit
Terminal REPL in another repository; file writes run without asking.
#>
[CmdletBinding()]
param(
    # Model as <provider>/<model id>: anthropic, openai, deepseek,
    # openai-compatible, llamacpp or mock.
    [string]$Model,
    # What tools may do without asking.
    [ValidateSet("read-only", "ask", "auto-edit", "full")]
    [string]$Permissions,
    # A configuration file instead of rah.toml in the data directory.
    [string]$Config,
    # Workspace root for file tools and processes (default: current directory).
    [string]$Root,
    # Data directory for sessions, settings and the rest (default: RAH_HOME,
    # else ~\.rah).
    [string]$DataDir,
    # Web UI address, loopback only.
    [string]$Bind = "127.0.0.1:18765",
    # The terminal REPL instead of the Web UI.
    [switch]$Cli,
    # The Web UI in a native window (rah-desktop) instead of the browser.
    [switch]$Desktop,
    # Do not open the browser.
    [switch]$NoBrowser,
    # Build with optimizations (slower first build, faster runs).
    [switch]$Release,
    # Write rah.toml (or -Config) from -Model, -Permissions and -Root, and exit.
    [switch]$InitConfig
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

if ($Desktop -and $Cli) { throw "-Desktop and -Cli cannot be combined." }
if ($Desktop -and $PSBoundParameters.ContainsKey("Bind")) {
    throw "-Bind is not supported with -Desktop; the window picks its own port."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH; install Rust from https://rustup.rs"
}

# Relative to where the script was called from, not the repository it runs in.
function Resolve-FromCaller([string]$Path) {
    $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
}

$global = @()
if ($Model) { $global += @("--model", $Model) }
if ($Permissions) { $global += @("--permissions", $Permissions) }
if ($Config) { $global += @("--config", (Resolve-FromCaller $Config)) }
if ($Root) { $global += @("--root", (Resolve-FromCaller $Root)) }
if ($DataDir) { $global += @("--data-dir", (Resolve-FromCaller $DataDir)) }

$package = if ($Desktop) { "rah-desktop" } else { "rah-cli" }
$cargoArgs = @("run", "--quiet", "-p", $package)
if ($Release) { $cargoArgs += "--release" }
$cargoArgs += "--"
if ($InitConfig) {
    if ($Desktop -or $Cli) { throw "-InitConfig cannot be combined with -Desktop or -Cli." }
    $cargoArgs += @("config", "init")
} elseif (-not $Desktop -and -not $Cli) {
    $cargoArgs += @("web", "--bind", $Bind)
}
$cargoArgs += $global

Push-Location $repo
try {
    if ($InitConfig -or $Cli -or $Desktop) {
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
