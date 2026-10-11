# Task 151 - Windows live Desktop repository authoring validation

## Scope and starting state

- Starting SHA: `499f726c0be6b977a9ca716e12e49cb62bb65a9c`
- Starting `origin/master`: `499f726c0be6b977a9ca716e12e49cb62bb65a9c`
- Environment: Windows, PowerShell, Asia/Taipei
- Released baseline: RAH v0.11.0
- Task 146: skipped
- Task 120: deferred
- Task 152 milestone audit: not performed

Task 151 validates the existing bounded Desktop repository authoring, human
review, one-shot authorization, and `repo.commit` path. It adds no model-visible
authority and does not claim transport confinement.

## Complete live chronology

1. Task 151 started from the completed Task 150 baseline at
   `499f726c0be6b977a9ca716e12e49cb62bb65a9c`.
2. The first live edit failed because the certified Codex baseline contained
   `codex.exe` but lacked `codex-code-mode-host.exe`.
3. The baseline verifier had accepted an incomplete runtime bundle:
   `CERTIFIED_BASELINE_INCOMPLETE_BUNDLE_GAP = CONFIRMED`.
4. The certified contract was hardened to closed manifest schema v2. It requires
   exactly `codex.exe` and `codex-code-mode-host.exe`, separate SHA-256
   identities, native PE validation, rejection of reparse points, and canonical
   containment in the certified directory.
5. The local certified Codex 0.149.0 bundle was repaired:
   - selected executable:
     `C:\Users\spider.tp\AppData\Local\codex-baselines\0.149.0\codex.exe`
   - reported version: `codex-cli 0.149.0`
   - `codex.exe` SHA-256:
     `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
   - `codex-code-mode-host.exe` SHA-256:
     `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`
6. The minimal runtime smoke passed with `RAH_ECHO_BRIDGE_OK`.
7. The first repository fixture was contaminated by a CRLF-only worktree
   difference. The guarded editor correctly refused an already-dirty target.
   This was fixture contamination, not a production repository-authoring
   defect.
8. A fresh byte-clean fixture was created at `D:\rah-task151-clean` with
   `core.autocrlf=false` and explicit LF bytes.
9. The live model edit passed. `tracked.txt` became exactly
   `RAH_TASK151_EDIT_OK\n`, remained unstaged, and no commit was created.
10. Desktop human Stage passed. Independent Git inspection showed the exact
    intended textual change staged, with no unstaged target change.
11. Desktop Staged Review passed with supported textual review and Authorize
    Commit available.
12. Human Authorize passed. A subsequent accidental Refresh correctly revoked
    that authorization and re-exposed Authorize according to Task 148 policy;
    Refresh itself created no commit.
13. The human authorized again and did not Refresh.
14. The model requested the existing `repo.commit` Tool with message
    `RAH Task 151 live commit`. It did not request staging or further edits.
15. The Desktop reported `Committed the staged change successfully` and the
    sanitized verified commit OID
    `90683f5eaab129a75e815879e69586ff75de5e86`.
16. Independent Git verification passed as recorded below.

No `codex-windows-sandbox-setup.exe` or `codex-command-runner.exe` requirement
was demonstrated. They are intentionally excluded from the Task 151 certified
contract. The only newly demonstrated prerequisite is
`codex-code-mode-host.exe` beside `codex.exe`.

## Human workflow and model prompts

The live edit prompt required the complete target contents and expressly
forbade staging or committing. The durable conversation record confirms that
the model reported the guarded repository-safe exact-replacement path, but the
record does not persist the exact Tool label. Therefore the precise authoring
Tool name is an evidence gap and is not inferred here.

The commit prompt was:

```text
Commit the currently reviewed staged change using the commit message:

RAH Task 151 live commit

Use the repository commit tool.

Do not modify or stage any additional files.
```

Stage and Authorize were human Desktop actions. Model request and Execute
permission were not commit authorization. The frontend did not own
authorization. The existing Rust-owned one-shot reviewed-snapshot
authorization was consumed by `repo.commit`, and `repo.commit` did not
auto-stage.

## Tool lifecycle and Desktop result

The Desktop showed a successful sanitized verified result with commit OID
`90683f5eaab129a75e815879e69586ff75de5e86`. The durable transcript preserves
the model request and successful result but intentionally does not persist
`ToolRequested`, `ToolStarted`, or `ToolFinished` activity events. Exact
`repo.commit` lifecycle counts could not be recovered after the run and are an
explicit evidence gap; no `1 / 1 / 1` count is asserted without evidence.

Independent Git history proves exactly one commit was created, and the reflog
contains only the initial fixture commit followed by the one Task 151 commit.
There was no automatic second commit, retry, or replay in Git-observable state.

## Independent Git verification

- Repository: `D:\rah-task151-clean`
- Branch: `master`
- Old HEAD: `f94d06d4dbe063f60175cf3afa6d0ea726ed547b`
- New HEAD: `90683f5eaab129a75e815879e69586ff75de5e86`
- Displayed/model-reported OID matched New HEAD: PASS
- `refs/heads/master` pointed to New HEAD: PASS
- New HEAD parent equaled Old HEAD: PASS
- Commit subject equaled `RAH Task 151 live commit`: PASS
- Commit count changed from 1 to 2: PASS
- `HEAD:tracked.txt` contained exactly `RAH_TASK151_EDIT_OK\n`: PASS
- Worktree `tracked.txt` bytes were the same 20 LF-terminated bytes: PASS
- `git status --porcelain=v1` was empty: PASS
- `git diff` was empty: PASS
- `git diff --cached` was empty: PASS
- The only loose branch ref was `refs/heads/master`: PASS
- Reflog contained exactly the initial commit and the single live commit: PASS

The verified commit result triggered the existing Task 149 repository refresh
path. Independent post-commit observation confirms the committed change no
longer exists in the staged or worktree diff. The one-shot authorization was
consumed by the successful commit; commit output remains presentation/history,
not renewed authority.

## Authority conclusions

- Model request is not authorization.
- Execute permission is not commit authorization.
- Human Stage remains a host action.
- Human Authorize remains the reviewed-snapshot authorization event.
- Frontend presentation does not own authorization.
- `repo.commit` used the existing one-shot Rust-owned authorization.
- `repo.commit` did not auto-stage.
- No generic Git authority was introduced.
- No generic shell authority was introduced.
- No new authority, ADR, dependency edge, version bump, tag, or release is part
  of Task 151.
- ADR 0010 and ADR 0016 remain authoritative.
- RAH v0.11.0 remains released.

## Final validation

All final gates passed on Windows:

- `cargo fmt --check`
- `cargo check --workspace -j 1`
- `cargo test --workspace -j 1`
- `cargo clippy --workspace --all-targets --all-features -j 1 -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`
- `node --check crates/rah-desktop/frontend/status.js`
- `scripts/test-codex-baseline.ps1` against the certified 0.149.0 native
  executable, including fail-closed missing-helper, modified-helper, unknown
  manifest-field, hash, version, malformed-manifest, missing-binary, and missing
  baseline probes
- `scripts/codex-baseline.ps1 verify 0.149.0`

Metadata remained 12 packages, all version 0.11.0 and edition 2024. There was
no dependency delta, ADR impact, or authority impact.

## Limitations

This task validates only the bounded Windows Desktop path described by Task
151. It does not claim Task 152 milestone completion or transport confinement.
The exact live activity-event counts and precise edit-tool label were not
durably persisted and remain explicit evidence gaps.
