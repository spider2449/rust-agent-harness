# RAH v0.30 Linked Worktree Windows Live Certification

## Verdict

**PASS WITH EXPLICIT NONCLAIMS**

The certified Windows host-driven gate passed on the exact pushed source and
emitted:

```text
RAH_V030_LINKED_WORKTREE_LIVE_OK
```

No production behavior correction was required. The certification source is
`93a522c2a42b438d539d296ba92cbb9dd1eea297`; its exact-head CI run was
`35420903926` (PASS). The source contains one test-only Windows certification
harness addition in `crates/rah-desktop/src/main.rs` and no production,
dependency, schema, ADR, or permission changes.

Production behavior base: `a684405ecd0143093fba669b9b52e84bc32f7590`.
Task 360 deterministic/audit tree: `3771a52227d2ef85944e971a2a0dc9c0e5d5856f`.
Task 360 exact-head CI: `35352376409` (PASS).

## Environment

| Item | Recorded value |
| --- | --- |
| Windows | Windows 11 IoT Enterprise LTSC, build 26100, x64 |
| rustc | `1.98.1 (48a229cea 2026-09-01)` |
| Cargo | `1.98.1 (797e8a9bc 2026-08-05)` |
| Native Git | `2.55.0.windows.5` |
| `rah-desktop` | `0.29.0` |
| Workspace | 13 packages, all `0.29.0`, Rust edition 2024 |
| HostExplicit | exactly 11; no `repo.create-directory` addition |
| Codex installed | `codex-cli 0.155.1`, SHA-256 of `codex.cmd`: `00743f8084cbc1594683b33cfb8bf14d2ce40d46ca6ba9f7142de6ba31502a84` |
| Certified Codex baseline | `0.149.0` unavailable; Codex was not run |

Pre-live formatting, workspace check/test/lint, metadata, release build, and
focused linked-worktree suites passed. The final source was pushed, matched
`origin/master`, and was clean before the live run.

## Fixture and topology

The harness created a unique disposable native-Git fixture outside the RAH
repository:

```text
fixture/
  main/       branch main
  linked-a/   branch worktree-a, native default linked form
  linked-b/   branch worktree-b, native relative linked form
```

The three worktrees shared one Git common directory while retaining distinct
private worktree Git directories, HEAD files, and indexes. Native Git created
the topology; RAH did not execute worktree lifecycle commands. Private paths,
registration paths, and sentinel strings were used only as private evidence.

## Live claims established

The exact-source run established all mandatory Windows claims:

- main, linked A, and linked B admitted through production semantic validation;
  A used the ordinary linked form and B used Git's supported relative form;
  all received distinct process-local member IDs;
- common Git identity was equal while private Git identity was distinct;
  duplicate, canonical alias, copied/fabricated/malformed gitfiles were
  rejected before membership or effects;
- local submodule roots, `--separate-git-dir` roots, selected-root/private/
  common/worktrees/registration reparse relations were rejected fail-closed;
- only one member was active at a time; switch A→B, Close, reactivation, and
  inactive removal preserved lifecycle semantics; removal left filesystem and
  native Git registration intact and performed no prune/remove;
- selected-root observation read each member's own HEAD and index; staged
  state was deliberately different across members;
- production Stage and Unstage changed only A's index and preserved B's staged
  state and the main index; no sibling `index.lock` or content effect remained;
- production file creation, patch/edit, rename/delete, and nested-boundary
  checks stayed within A; A's `.git` metadata remained protected;
- reviewed A Commit used A's selected branch, HEAD, and index. An unrelated B
  Commit did not stale the still-current A review and did not alter B's index
  during A Commit. Moving A's selected branch ref made the old review stale;
  native Commit spawn was zero and no retry or rebinding occurred;
- local branch creation used selected A HEAD, left A on its original branch,
  created no worktree, and rejected a target-ref conflict. Detached linked
  observation worked while Commit remained unavailable under the attached
  branch rule;
- external native worktree move/removal and registration/root `.git`, backlink,
  or commondir mutation made retained identity stale before activation/effect;
  RAH did not migrate, prune, or repair it;
- deterministic Task 360 activation-publication barrier evidence passed on the
  certified source;
- Effective Authority contained only the selected active member's repository
  authority, and zero-active state contained none. No private Git directory,
  common directory, registration directory, gitfile bytes, or filesystem
  identity appeared in authority or other tested product DTO surfaces;
- HostExplicit remained exactly 11 and preparations became stale across
  lifecycle currentness changes;
- remembered linked A persisted only as a descriptive location candidate. It
  contained no private/common Git path, registration ID, gitfile content,
  filesystem identity, or `RepositoryMemberId`;
- fresh Desktop state restored zero admitted members, no active member, no
  repository, no repository ToolRegistry, no index reservation, no Commit
  authorization, idle HostExplicit, no executable conversation state, and no
  provider connection. Fresh admission reran semantic validation;
- fixture child processes were owned, reaped, and bounded; the unique fixture
  root and its Git metadata were removed. No global process-name killing was
  used.

## Effect accounting and privacy

Admission, activation, switch, Close, and removal performed zero Git mutation.
Stage/Unstage touched only the selected index. Content authoring touched only
the selected worktree. Commit changed the selected index/HEAD/branch plus
normal shared Git objects. Branch creation changed one intended local ref.
No production path executed `git worktree add/remove/prune/repair/move/lock/`
`unlock`, fetch, pull, push, clone, or network credential access.

The harness kept fixture-controller commands separate from RAH production
effect counts. Product-observable admission errors, membership, authority,
workflow/activity snapshots, Tool schemas/inputs, and remembered-workspace
serialization were searched for distinctive private topology strings and did
not leak them.

## Nonclaims

`GUI automation was NOT EXECUTED.` Model-selected worktree routing, Codex
inference, Linux or macOS live certification, submodule support,
separate-git-dir support, worktree lifecycle authority, multiple active
repositories, persistent executable membership, OS sandboxing, network
isolation, race-free TOCTOU, a machine-wide external-Git lock, and rollback,
replay, or compensation for uncertain native effects are not claimed.

## Git closure

The certified source remained `93a522c2a42b438d539d296ba92cbb9dd1eea297`.
This document and the execution record below are a later docs-only descendant;
their SHA and exact-head CI result are reported separately from the certified
source.
