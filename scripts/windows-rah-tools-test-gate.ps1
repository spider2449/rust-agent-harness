param(
    [string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    $EvidenceDirectory = Join-Path $env:TEMP ("rah-tools-gate-{0}" -f [guid]::NewGuid().ToString('N'))
}
$EvidenceDirectory = [System.IO.Path]::GetFullPath($EvidenceDirectory)
$logPath = Join-Path $EvidenceDirectory 'cargo-test.log'
$metadataPath = Join-Path $EvidenceDirectory 'metadata.json'
$started = [DateTimeOffset]::UtcNow
$cargoExitCode = $null
$disposition = 'HARNESS_ERROR'
$harnessError = $null
$head = $null

try {
    New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
    $head = (& git -C $repositoryRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Unable to read repository HEAD.' }
    Push-Location -LiteralPath $repositoryRoot
    try {
        # Keep libtest's ordinary parallelism and let the child finish naturally.
        # Windows PowerShell represents native stderr as error records. Let Cargo
        # write those records into the combined log without treating them as a
        # PowerShell exception.
        $previousErrorAction = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        try {
            & cargo test -p rah-tools -- --nocapture 2>&1 |
                Out-File -LiteralPath $logPath -Encoding utf8
        }
        finally { $ErrorActionPreference = $previousErrorAction }
        $cargoExitCode = $LASTEXITCODE
        if ($null -eq $cargoExitCode) { throw 'Cargo did not provide an exit code.' }
        if ($cargoExitCode -eq 0) { $disposition = 'PASS' }
        else { $disposition = 'TEST_FAILURE' }
    }
    finally { Pop-Location }
}
catch {
    $harnessError = $_.Exception.Message
}
finally {
    $ended = [DateTimeOffset]::UtcNow
    $metadata = [ordered]@{
        startTimeUtc = $started.ToString('o')
        endTimeUtc = $ended.ToString('o')
        elapsedSeconds = ($ended - $started).TotalSeconds
        repositoryRoot = $repositoryRoot
        repositoryHead = $head
        cargoExitCode = $cargoExitCode
        disposition = $disposition
        harnessError = $harnessError
        logPath = $logPath
    }
    New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
    $metadata | ConvertTo-Json | Set-Content -LiteralPath $metadataPath -Encoding utf8
    Write-Host "RAH tools gate: $disposition; Cargo exit: $cargoExitCode; evidence: $EvidenceDirectory"
}

if ($disposition -eq 'PASS') { exit 0 }
if ($disposition -eq 'TEST_FAILURE') { exit $cargoExitCode }
exit 1
