# Starts the RAH Web UI and opens the first URL that carries the access
# token. The console window stays open as the server's log.
$ErrorActionPreference = "Continue"
$opened = $false
& (Join-Path $PSScriptRoot "rah.exe") --web @args 2>&1 | ForEach-Object {
    $line = "$_"
    Write-Host $line
    if (-not $opened -and $line -match '(http://\S+/#token=[0-9a-f]+)') {
        $opened = $true
        Start-Process $Matches[1]
    }
}
