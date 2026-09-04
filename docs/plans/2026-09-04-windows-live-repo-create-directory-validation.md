# Task 186 — Windows Live `repo.create-directory` Validation

## Task scope

Windows Desktop plus real native Codex live certification for
`repo.create-directory`.

## Source baseline

- Task 185 commit: `473fab2d44f0638679f01241b2e59c4b7f269b23`
- Task 185 exact-head CI: `33826631224`
- Desktop binary: `target/release/rah-desktop.exe`, built from the source
  baseline above.

## Certified Codex baseline

- Version: `codex-cli 0.149.0`
- `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`
- Certified side-by-side directory: `D:\spider\tools\codex\0.149.0`

## Fixture and baseline

The new disposable fixture was `D:\rah-task186-live`. Its tracked baseline
contained `sentinel.txt` and `parent/anchor.txt`; `parent/new-directory` was
absent during preflight.

- Baseline HEAD: `ef01f5cb86f827b10af01aa25bc1d8d402b0f220`
- Baseline branch: `master`
- Baseline refs: `ef01f5cb86f827b10af01aa25bc1d8d402b0f220 refs/heads/master`
- `sentinel.txt` SHA-256: `6a3ee6bda08a144897d5717b2dd166c575601b19f98aca20956a5f1952287003`
- `sentinel.txt` length: `16`
- `parent/anchor.txt` SHA-256: `76941c1ce43944361925b0fd435d40c54d9e2744eaed34b077664b03295055a5`
- `parent/anchor.txt` length: `21`

## Advertisement and request evidence

- Public tool: `repo.create-directory`
- `dynamic_definition_emitted=true`
- Observed private alias: `rah_tool_3`
- The request used the same alias as the advertisement. The alias is private
  and non-contractual.
- Exact non-null request:

  ```json
  {"path":"parent/new-directory"}
  ```

No recursive, parent-creation, force, mode, or other mutation parameter was
present.

## Lifecycle and structured result

The exact lifecycle was:

- Requested: `1`
- Started: `1`
- Finished: `1`

The single finished result had `is_error=false` and:

```json
{
  "git_metadata_changed": false,
  "path": "parent/new-directory",
  "status": "directory_created_verified",
  "uncertain": false
}
```

## Completion and raw evidence

- Completion marker: `RAH_REPO_CREATE_DIRECTORY_LIVE_OK`
- `marker_observed=true`
- Raw JSONL validation: `RAH_JSONL_PARSE_OK`
- JSONL parse errors: `0`
- Failure records: `0`
- Desktop was closed normally after turn completion.

The completion evidence recorded the final model completion as marker-observed
by the Desktop completion gate.

## Repository context

The selected repository and execution context remained internally consistent:

- Repository fingerprint:
  `repo-context:d689cbef210e1c80802160a87837f531400cd6ee852fee8ad78121cb9d9c2993`
- Repository generation: `1`
- Connection generation: `1`
- Runtime generation: `1`
- Session generation: `1`

The tool lifecycle correlated to `D:\rah-task186-live`; no stale runtime
publication or repository mismatch was observed.

## Filesystem postcondition

- `parent/new-directory` exists.
- It is an ordinary directory.
- It is empty: child count `0`.
- `.gitkeep` and `.keep` are absent.
- The preexisting `parent` remains intact and contains only `anchor.txt` and
  `new-directory`.
- Sentinel and anchor SHA-256 values and lengths equal their baselines.

## Git postcondition

- `git status --short` was empty.
- Cached diff was empty.
- Worktree diff was empty.
- HEAD remained `ef01f5cb86f827b10af01aa25bc1d8d402b0f220`.
- Branch remained `master`.
- Refs remained unchanged.
- No staging or commit occurred.

The clean Git status is expected because Git does not track empty directories;
the filesystem target is the authoritative creation proof.

## No unintended authority and no replay

Evidence showed no requests for `repo.create-file`, `repo.delete-file`,
`repo.rename-file`, `repo.edit-files`, `repo.patch`, or `repo.commit`. No shell
or process execution was used. The operation had one effect attempt, with no
retry, replay, compensation, delete, or rollback claim.

## Platform classification

**Windows `repo.create-directory` live certification: PASS**

This certifies Windows only. Linux remains deterministic-validation only.

## Historical release state

The v0.14.0 release remains untouched:

- Release commit: `52506521bdf838784dd45bb54df2d6bcff8bcd08`
- Tag: `v0.14.0`
- Tag object: `9193423e96dd0cda2fd8f5ed5619ab2b58483acc`

No version, release, Rust, frontend, Cargo, dependency, or authority-boundary
changes were made for this validation.
