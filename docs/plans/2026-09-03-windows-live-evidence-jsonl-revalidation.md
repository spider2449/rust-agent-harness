# Task 175F — Windows live evidence JSONL revalidation

## Result

**PASS.** Task 175F closes the evidence and documentation revalidation for the
Windows live A → reconnect → B flow and the raw JSONL evidence stream.

Task 175F does not complete Task 175. Final destructive `repo.rename-file`
certification remains pending.

## Chronology

### Initial prerequisite attempt

The initial Task 175F prerequisite attempt was **INCONCLUSIVE** because the
installed Codex was `0.152.1` rather than the certified `0.149.0`. No Desktop
launch or live effect occurred.

### Certified baseline restoration

The actual certified native baseline restored before the successful live run
was:

- Codex: `codex-cli 0.149.0`
- `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`

The RAH source baseline before the Task 175F evidence commit was
`ae7de8af680a9a5bd44bd83d42a89dd51005a4fa`. Task 175E exact-head CI was PASS,
run `33709213199`.

### Invalid fixture attempt

An intermediate manual attempt was **INVALID** because its disposable fixture
was built incorrectly. Wrong filenames were created, the expected
`A_ONLY.txt`, `B_ONLY.txt`, and `shared.txt` paths did not exist, and the
repositories had no committed HEAD. Consequently, `fs.read("shared.txt")`
terminated with bounded `tool_dispatch_failure`.

No repository mutation occurred. This was a fixture-construction error and did
not demonstrate a RAH product defect. It is not classified as a Task 175F
product FAIL.

## Correct successful fixtures

Both repositories were clean before the successful Desktop run.

### Repository A

- HEAD: `0720dd70fe18f1a8fc44030bd0d4b50463f610fc`
- Tracked: `A_ONLY.txt`, `shared.txt`
- Expected: `A_ONLY.txt` present; `B_ONLY.txt` absent;
  `shared.txt = TASK175F_SHARED_A`

### Repository B

- HEAD: `47f75c9169c0f4198fc697297ade49f524eeccfb`
- Tracked: `B_ONLY.txt`, `shared.txt`
- Expected: `B_ONLY.txt` present; `A_ONLY.txt` absent;
  `shared.txt = TASK175F_SHARED_B`

## Repository A live evidence

- `repository_generation = 1`
- `connection_generation = 1`
- `runtime_generation = 1`
- `session_generation = 1`
- Repository fingerprint:
  `repo-context:eef555efab227cdc55bbdfccf9ee84915c4e6322a9312b493a8062601ee4f359`

Evidence showed:

- `A_ONLY.txt` present as a clean tracked regular file
- `B_ONLY.txt` absent/untracked
- `shared.txt` present
- `fs.read` result: `TASK175F_SHARED_A`

## Repository B live evidence

- `repository_generation = 2`
- `connection_generation = 2`
- `runtime_generation = 2`
- `session_generation = 2`
- Repository fingerprint:
  `repo-context:47dd2cc75cfa22168b5a79e9778bdb3bbfb0b182c632a84f9a1515e72fcb6c0b`

Evidence showed:

- `B_ONLY.txt` present as a clean tracked regular file
- `A_ONLY.txt` absent/untracked
- `shared.txt` present
- `fs.read` result: `TASK175F_SHARED_B`

The A and B fingerprints differed. B received fresh connection, runtime, and
session context. No stale A runtime was rebound as B.

## Task 175E live JSONL verification

The exact live hard gate was:

```powershell
$ErrorActionPreference = "Stop"

Get-Content $env:RAH_LIVE_EVIDENCE_PATH |
    Where-Object { $_.Length -gt 0 } |
    ForEach-Object {
        $_ | ConvertFrom-Json -ErrorAction Stop | Out-Null
    }

Write-Host "RAH_JSONL_PARSE_OK"
```

Observed output was `RAH_JSONL_PARSE_OK`, with zero `ConvertFrom-Json` errors.
The raw evidence stream itself parsed successfully. No repair, splitting, or
recovery of malformed lines was necessary. Therefore the prior Task 175D
`{...}{...}` concurrent JSONL framing defect did not recur. Task 175E's
process-wide shared append fix is live-validated.

## Failure and mutation gate

- Failure count: `0`
- Mutation request count: `0`

There were no calls to `repo.rename-file`, `repo.delete-file`,
`repo.create-file`, `repo.edit-files`, `repo.patch`, or `repo.commit`. No
destructive Tool was invoked.

## Post-state

Repository A remained at HEAD
`0720dd70fe18f1a8fc44030bd0d4b50463f610fc`.

Repository B remained at HEAD
`47f75c9169c0f4198fc697297ade49f524eeccfb`.

For both repositories, `git status --short`,
`git diff --cached --name-status`, and `git diff --name-status` were empty.
Refs were unchanged. There was no staging, commit, or worktree mutation.

## Conclusion and platform boundary

- Task 175F: **PASS**
- Task 175C repository-context lifecycle: live-validated on Windows normal
  A → reconnect → B flow
- Task 175E atomic JSONL framing: live-validated on Windows
- Task 175: **STILL NOT COMPLETE**

Windows is the live-certified project platform and current live validation
platform. Linux has deterministic validation only; Linux live certification is
not established. This document does not rewrite the historical Task 175 first
WebView automation failure, first successful rename run, Task 175A
observability defects/fixes, historical Chat failed root cause, or malformed
Task 175D JSONL evidence. Task 175F validates corrected normal behavior
prospectively.

The protected untracked chronology
`docs/plans/2026-09-02-windows-live-repo-rename-file-validation.md` remains
untouched and must remain untracked and unstaged.
