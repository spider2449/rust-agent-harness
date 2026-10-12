<#
.SYNOPSIS
Lists, restores or purges deleted sessions.

.DESCRIPTION
Deleting a session in RAH moves its log to `<session dir>/deleted/`. This
script lists those logs, moves one back (restore), or removes them for good
(purge, which asks first). Stop `rah` before restoring, so it does not keep a
stale view of the session.

.EXAMPLE
.\scripts\sessions.ps1
Lists deleted sessions.

.EXAMPLE
.\scripts\sessions.ps1 -Restore 541197a1
Restores the deleted session whose id starts with 541197a1.

.EXAMPLE
.\scripts\sessions.ps1 -Purge -OlderThanDays 30
Permanently removes deleted sessions older than 30 days.
#>
[CmdletBinding(DefaultParameterSetName = "List")]
param(
    # The session directory (default: .rah/sessions under the current directory).
    [string]$SessionDir = ".rah/sessions",
    # Restore the deleted session with this id or id prefix.
    [Parameter(ParameterSetName = "Restore", Mandatory)]
    [string]$Restore,
    # Permanently remove deleted sessions.
    [Parameter(ParameterSetName = "Purge", Mandatory)]
    [switch]$Purge,
    # With -Purge: only sessions deleted at least this many days ago.
    [Parameter(ParameterSetName = "Purge")]
    [int]$OlderThanDays = 0
)

$ErrorActionPreference = "Stop"
$deleted = Join-Path $SessionDir "deleted"
if (-not (Test-Path $deleted)) {
    Write-Host "No deleted sessions in $SessionDir."
    exit 0
}

# Reads the title the way session-jsonl does: the last title line wins.
function Get-Title([string]$path) {
    $title = $null
    foreach ($line in Get-Content -LiteralPath $path -Encoding UTF8) {
        if ($line -match '^\{"type":"title"') {
            $title = ($line | ConvertFrom-Json).title
        }
    }
    $title
}

$files = @(Get-ChildItem -LiteralPath $deleted -Filter "*.jsonl" | Sort-Object LastWriteTime -Descending)

switch ($PSCmdlet.ParameterSetName) {
    "List" {
        if (-not $files) { Write-Host "No deleted sessions."; break }
        $files | ForEach-Object {
            [pscustomobject]@{
                Id = $_.BaseName
                Title = Get-Title $_.FullName
                Changed = $_.LastWriteTime
            }
        } | Format-Table -AutoSize
    }
    "Restore" {
        $match = @($files | Where-Object { $_.BaseName.StartsWith($Restore) })
        if ($match.Count -eq 0) { throw "No deleted session starts with '$Restore'." }
        if ($match.Count -gt 1) { throw "'$Restore' matches $($match.Count) sessions; give more of the id." }
        $target = Join-Path $SessionDir $match[0].Name
        if (Test-Path $target) { throw "$target already exists." }
        Move-Item -LiteralPath $match[0].FullName -Destination $target
        Write-Host "Restored $($match[0].BaseName)."
    }
    "Purge" {
        $cutoff = (Get-Date).AddDays(-$OlderThanDays)
        $old = @($files | Where-Object { $_.LastWriteTime -le $cutoff })
        if ($old.Count -eq 0) { Write-Host "Nothing to purge."; break }
        $answer = Read-Host "Permanently remove $($old.Count) deleted session(s)? [y/N]"
        if ($answer -ne "y") { Write-Host "Cancelled."; break }
        $old | Remove-Item -Confirm:$false
        Write-Host "Removed $($old.Count) session(s)."
    }
}
