# Task 301 — RAH v0.25 Reviewed Rename/Move Windows Live Certification

## Scope

This plan records the Windows connected-current Desktop HostExplicit live gate
for the reviewed `repo.rename-file` route. It is certification-only: it adds
one ignored Windows test and feature-gated test observability, and does not
change release status, create a tag, create a GitHub Release, or begin release
preparation.

## Authoritative checkpoint

| Check | Value |
| --- | --- |
| Current master before Task 301 | `51fbd8250221cd7dcd09202b1297570e05f325c3` |
| Direct parent | `0f84c8ad6bd5101e235865011c559ea7121173f4` |
| Exact-head CI before Task 301 | `34579301495` — PASS |
| ADR | `0026-hostexplicit-reviewed-file-rename-move-boundary.md`, Accepted |
| Branch | `master` |

## Authorized changed-file scope

- `crates/rah-tools/src/repository_rename_file.rs`
- `crates/rah-tools/src/lib.rs`
- `crates/rah-desktop/src/main.rs`
- `docs/plans/2026-09-11-task-301-reviewed-rename-windows-live-certification.md`

The two `rah-tools` changes are limited to the existing opt-in
`live-test-support` feature. They record repository-keyed Tool execution and
native-attempt counts and have no production behavior or API effect without
that feature.

## Certified environment

Observed before the live gate:

| Item | Value |
| --- | --- |
| Windows edition/version/build | Microsoft Windows 10 Professional, `10.0.19045`, build `19045` |
| Architecture | x64 / `x86_64-pc-windows-msvc` |
| Rust toolchain | `rustc 1.96.0 (ac68faa20 2026-05-25)`, Cargo `1.96.0 (30a34c682 2026-05-25)` |
| Git | `git version 2.54.0.windows.1` |
| Git executable | `C:\Program Files\Git\cmd\git.exe` |
| Codex source | certified baseline, `npm-isolated` manifest |
| Codex version | `codex-cli 0.149.0` |
| Codex executable | `C:\Users\spider.tp\AppData\Local\codex-baselines\0.149.0\codex.exe` |
| Codex executable SHA-256 | `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00` |

The live test fails closed unless the repository resolver selects the certified
`0.149.0` baseline and the executable reports exactly `codex-cli 0.149.0`.

## Fresh fixture

The ignored test creates a disposable real Git repository with an attached
committed `master` HEAD, an ordinary mode-`100644` UTF-8 source file at
`src/rah-hostexplicit-live-rename.txt`, and an existing ordinary destination
parent at `safe-destination`. The destination is absent from the worktree,
HEAD, all index stages, and ignore rules. The source contains multiple lines,
the non-ASCII `λ` sentinel, no NUL, and remains byte-identical and clean.

An unrelated committed file is modified and staged so that the reviewed Commit
authorization is valid while the rename source remains clean. The fixture
rejects sparse or linked worktrees, nested repositories, reparse ancestry, and
active merge/rebase/cherry-pick/revert/sequencer/bisect state.

## Live evidence contract

The test must prove, from the production Desktop route:

1. `ConnectedCurrent` composition has exactly 11 eligible Tools, with one
   `repo.rename-file` entry bound to `RepositoryHost`, repository mutation,
   repository file rename, `Execute`, and `RepoRenameFile`; no MCP, Process
   Plugin, provider activation, or Trusted Profile provider participates.
2. Prepare accepts only typed source/destination paths, emits a bounded opaque
   ticket, and performs zero Tool executions, zero native attempts, zero Git or
   filesystem effects, zero model/provider activity, and no Commit review
   invalidation.
3. The direct review contains exact paths, length, SHA-256, complete escaped
   UTF-8 content, mode, expected unstaged move consequence, and all explicit
   non-effects. Generic Prepared activity remains status/provenance-only and
   excludes private ticket, content, hashes, paths, native identities, Tool
   input, Git evidence, and review data.
4. Confirm accepts only the ticket, revalidates retained preparation and
   currentness, performs D2 before Started, invalidates the pending Commit
   authorization before the effect boundary, and reaches one Started event.
5. The current ToolRegistry executes exactly one ordinary `repo.rename-file`
   Tool and exactly one native no-replace attempt. There is no retry, replay,
   reverse rename, compensation, rollback, or second mutation path.
6. The terminal activity is status-only `renamed_verified`; the retained
   preparer independently classifies the result as `ReviewedSuccess` and the
   post-effect proof verifies source absence, destination regular-file shape,
   exact bytes/length/SHA-256, identity correspondence, parent/root/.git/Git
   validity, and protected Git state preservation.
7. The worktree consequence is observed semantically as source deletion plus
   untracked destination when Git does not heuristically report an `R`; HEAD,
   branch, refs, raw index, cached diff, and source HEAD/index representation
   remain unchanged.
8. The ticket cannot be reused, the activity ID cannot authorize Confirm or
   Cancel, the coordinator and chat return to Idle, no prepared ticket remains,
   refresh is emitted, model/MCP/Process Plugin counts remain zero, and the
   Codex child is shut down through the existing awaited lifecycle path.

## Validation record

| Validation | Result |
| --- | --- |
| `cargo fmt --check` | PASS during implementation validation |
| Focused reviewed rename tests | PASS: `rah-tools` rename suite, 36 passed |
| Windows Desktop deterministic suite | PASS: 219 passed, 10 ignored |
| Frontend syntax/permission suites | PASS: `node --check status.js`, frontend authority tests, Tauri rename permission tests |
| Desktop release build | PASS: `cargo build -p rah-desktop --release` |
| Workspace check/test/clippy/diff | PASS: workspace check, test, clippy, and diff check |
| Exact ignored live command | PASS on `ba1049c575da2a5bcbde613b34152361d8e56f76` using the repository Codex live-gate wrapper |
| Live marker | `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK` |
| Final commit / exact-head CI | pending final candidate |

## Final evidence

The successful live gate on `ba1049c575da2a5bcbde613b34152361d8e56f76` recorded:

- Windows 10 Professional `10.0.19045` build `19045`, x64;
- Rust `1.96.0`, Git `2.54.0.windows.1` from `C:\Program Files\Git\cmd\git.exe`;
- certified Codex baseline source `npm-isolated`, `codex-cli 0.149.0`, executable SHA-256 `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
- fresh real Git fixture with committed attached HEAD, dedicated clean source, existing empty safe destination parent, and unrelated staged change;
- Prepare Tool/native attempts `0/0`, unchanged source/destination absence, identities, directory entries, raw index, HEAD, branch, refs, Git state, generations, conversation, and pending Commit authorization;
- complete typed review with exact source/destination, length, SHA-256, complete escaped UTF-8 content, mode, effect/consequence, and explicit non-effects;
- status-only generic activity without ticket, source content, hash, paths, native paths, ToolInput, filesystem identity, Git evidence, repository identity, or preparer identity;
- Confirm ticket-only ordering with D2 before Started and Commit authorization invalidation before the effect;
- exactly one HostExplicit Started, one authorized Tool dispatch, one `repo.rename-file` execution, and one native no-replace attempt, with no replay/retry/reverse/compensation/rollback;
- independent post-effect proof classified `ReviewedSuccess`, exact destination bytes/length/SHA-256 and identity correspondence, source absence, ordinary non-reparse destination, and unchanged protected Git state;
- worktree semantics of source deletion plus untracked destination, with raw index and cached diff unchanged;
- Commit authorization pending before Prepare and after Prepare, then not pending after Started/effect;
- zero model lifecycle, MCP, and Process Plugin activity; coordinator/chat Idle; ticket reuse and activity-ID authorization rejected; one repository refresh;
- existing Desktop shutdown/reaping path completed with `RAH_RENAME_FILE_HOSTEXPLICIT_CLEANUP_REAPED=1`;
- final marker `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`.

The final exact candidate live gate and exact-head CI remain required after this
evidence-recording plan update.
