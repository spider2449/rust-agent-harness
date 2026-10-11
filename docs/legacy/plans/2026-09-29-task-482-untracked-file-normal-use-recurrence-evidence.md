# Task 482 — Untracked-file normal-use recurrence evidence

Date: 2026-09-29. RAH version: 0.32.0. Certified Codex baseline: 0.157.1. HostExplicit: exactly 11 (checkpoint, not recounted). v0.33 capability: NONE SELECTED.

## RAH checkpoint and fixture

The only RAH checkout used was `F:\coding\otherPrj\rah-task-480`. Before work, `git rev-parse HEAD` and `git rev-parse origin/master` both returned `ae69e5e29f35c4cccfab9328f2b322154dbefb4b`; `git status --short` was empty. The dirty primary checkout was not touched. The production `rah-desktop.exe` from `F:\Temp\rah-task481-target\release` was launched with the research checkout as its working directory. Its Desktop child used the certified Codex 0.157.1 app-server. No RAH source, test, contract, permission, dependency, or version file was changed.

The fresh disposable repository was `F:\Temp\rah-task482-untracked-normal-use`, on `main` at initial commit `d2bfb68bac02c4dffc12ebe4d6e32943b7f6b516`. The clean initial tracked tree was `README.md`, `docs/workflow.txt`, `src/app.txt`, and `src/config.txt`. The target file did not exist before the Desktop workflow.

## Production workflow and exact result

A human used the production Desktop with this fixture. RAH created `docs/local-setup.txt` through its existing `repo.create-file` Tool. The creation result was `{"length":47,"path":"docs/local-setup.txt","sha256":"5c7f920034c482569660c84fbb45262755912c5d45e7145039770cca02b1a90f","status":"ok"}`. RAH read back:

```text
mode=development
worker_count=44
cache=enabled
```

The pre-correction `repo.status` result was `status=ok` with one entry, `docs/local-setup.txt`, `tracked=false`, `index_state=untracked`, and `worktree_state=untracked`. Independent Git status was `?? docs/local-setup.txt`; the file existed and held the same 47 bytes. The Desktop authority view showed 16 effective Tools and zero unavailable capabilities. That inventory alone does not authorize a particular target.

On the first natural correction request, the connected Codex runtime chose its own `apply_patch`, not a RAH Tool. Its exact error was `patch rejected: writing is blocked by read-only sandbox; rejected by user approval settings`. The file was unchanged. This is a test/runtime path limitation and is **not** a `repo.patch` result. To obtain the required RAH evidence without retrying variants, the human then explicitly requested one `repo.patch` call. Before it, `repo.file-info` showed `head.present=false`, `index.tracked=false`, `worktree.present=true`, `worktree.kind=regular_file`, `worktree.size_bytes=47`, and SHA-256 `5c7f920034c482569660c84fbb45262755912c5d45e7145039770cca02b1a90f`.

The sole RAH editing Tool attempt was `repo.patch` with safely recordable arguments:

```json
{"path":"docs/local-setup.txt","expected_file_byte_length":47,"expected_file_sha256":"5c7f920034c482569660c84fbb45262755912c5d45e7145039770cca02b1a90f","expected_old_text":"worker_count=44","replacement_text":"worker_count=4"}
```

Its **complete Tool result** was:

```json
{"changed":false,"reason":"repository_state","status":"precondition_failed","uncertain":false}
```

Subsequent `fs.read` returned the original content, and `repo.status` again reported the path as untracked. Independent Git status remained `?? docs/local-setup.txt`; SHA-256 remained `5c7f920034c482569660c84fbb45262755912c5d45e7145039770cca02b1a90f`. No partial content change, index change, or `.rah-*.tmp` file was observed. No alternative write attempt was made.

## Contract, authority, and coverage

Accepted ADR 0012 explicitly requires `repo.patch` to target an existing clean HEAD-tracked file and rejects untracked targets. ADR 0014 likewise restricts `repo.edit-files` to clean HEAD-tracked files. ADR 0013 authorizes `repo.create-file` only when the target is absent from HEAD, index, and worktree; it never overwrites or appends. The observed `repository_state` refusal is consistent with those contracts, rather than a broken promise. The private host repository mutation policies own these boundaries; Execute and an effective Tool inventory do not widen them. No model-selected repository, generic filesystem write, shell write, or authority bypass is part of the supported RAH workflow.

No existing bounded content Tool explicitly supports replacing bytes in a newly created, still-untracked file. Staging alone does not make it HEAD-tracked. Committing the mistaken file first would change history and defeat the ordinary before-commit correction workflow. Direct host editing or Codex `apply_patch` does not prove RAH Tool coverage. Therefore no supported alternative was attempted. The restriction applies to the target state, so it would recur for other newly created untracked files meeting the same conditions.

Creating a file and correcting it before staging or committing is a normal development action. The inability to finish this small correction inside the current RAH Tool model is material; requiring an erroneous commit or an external editor is an unreasonable workaround for this scenario. This is broader than one spelling of `worker_count`, but narrower than all repository editing: existing clean HEAD-tracked files remain covered. No authority expansion is implied by this evidence report.

## Classification and next action

| Observation | Class | Evidence boundary |
| --- | --- | --- |
| Repository creation, read, file-info, and status worked | N | Production Desktop Tool results and independent fixture checks. |
| `repo.patch` refused the untracked target | G | Second normal-use instance; exact `precondition_failed` result and accepted tracked-only contract. |
| Codex's own `apply_patch` was blocked by its read-only sandbox | U | A separate runtime path; it is not the RAH Tool result. |
| Temporary files | N | None appeared in this workflow; Task 481's uncertain batch-edit incident was not reproduced or investigated. |

**C — UNTRACKED-FILE CAPABILITY GAP RECURRED; REASSESSMENT NOW JUSTIFIED.** Task 481's G candidate is strengthened by a second disposable-repository workflow and a precise RAH Tool refusal. It is not a reproducible maintenance defect: the accepted contracts intentionally exclude this target state. The recommended next task is a focused v0.33 scope reassessment of whether bounded correction of newly created untracked files should be selected, including authority research and product tradeoffs. This task selects and implements no capability.
