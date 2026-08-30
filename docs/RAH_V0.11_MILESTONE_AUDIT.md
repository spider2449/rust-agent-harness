# RAH v0.11 Milestone Audit

## Status

**V0.11 MILESTONE AUDIT PASS — RELEASE PREPARATION READY**

This is an audit checkpoint, not a release. Workspace packages remain at
version 0.10.0, no tag or GitHub Release exists for v0.11, and release
preparation is the next task.

## Audited checkpoint

- Starting checkpoint: `ad7db6d1067a05cb26d37198074375d300eb3e51`
  (`test: validate repository commit through live Codex`).
- At audit start, `HEAD` and `origin/master` both matched that checkpoint.
- The only local worktree item was untracked `.vscode/`; it was left untouched.
- Workspace baseline: 12 packages, all version `0.10.0`, Rust edition `2024`.

## v0.11 delivered capability

v0.11 delivers one bounded Tool: `repo.commit`. Its sole model-visible input
is the closed object `{ "message": "..." }`; no additional properties are
allowed. The result is bounded/redacted: a status and, for verified success, a
commit OID. It does not expose raw Git stderr, host paths, hooks paths, index
hashes, authorization data, host identity configuration, Git environment or
config, or credentials.

The capability completes only this local workflow:

```text
inspect -> edit/create -> stage/review -> host-reviewed bounded commit
```

It is not generic Git authority.

## Authority boundary

ADR 0016 is Accepted (`6b052ec070d492e58ae0de3eb49777c75324afd5`) and remains
authoritative; no later ADR supersedes or weakens it. The implementation aligns
with its material decisions:

- The trusted host owns authority. The model controls only a bounded UTF-8
  commit message.
- The host fixes the exact canonical repository, native Git executable,
  explicit identity, attached existing current branch, expected old HEAD, and
  compound reviewed index snapshot.
- Admission requires attached non-unborn HEAD and rejects detached/unborn,
  special/sequencer states, linked worktrees, and staged gitlinks/submodules.
- Snapshot binding combines raw index SHA-256, canonical staged-entry digest,
  and `git write-tree` OID. Authorization is policy-generation and
  repository/snapshot bound.
- The fixed normal Git commit command has no arbitrary argv, staging, amend,
  merge, rebase, cherry-pick, signing, or model-selected identity/configuration.
  Host-owned empty `core.hooksPath`, `--no-verify`, `commit.gpgSign=false`,
  minimized environment, and explicit host identity are enforced.
- Exactly one mutating spawn attempt is permitted. Postcondition verification,
  not exit status, decides `committed_verified`; the taxonomy is
  `invalid_input`, `precondition_failed`, `known_no_effect`,
  `committed_verified`, and `uncertain`.
- An uncertain possible external effect is never retried, replayed, reset,
  compensated, or rolled back.

No material ADR 0016 deviation was found. The Task 133 research conclusions
remain reflected in the implementation: a normal commit advances the attached
branch, `write-tree` may create unreachable tree objects, hooksPath is required
in addition to `--no-verify`, local config remains untrusted ambient input with
critical behavior overridden, and the shared RAH mutation lease is not a lock
against external actors.

## Task / commit evidence

| Task | Commit | Subject | Audit result |
| --- | --- | --- | --- |
| 132 | `f8c2e4da835f0167a3ad35440fa825d501ba1bde` | docs: define RAH v0.11 scope and authority roadmap | PASS |
| 133 | `982537d5203dd807627bbe6717066dff5fb52452` | docs: research bounded repository commit authority | PASS |
| 134 | `6b052ec070d492e58ae0de3eb49777c75324afd5` | docs: define bounded repository commit authority | PASS |
| 135 | `8497a1d55395b2f6bbe5cc0d6c1319b7e84114fc` | feat: add bounded repository commit foundation | PASS |
| 136 | `a128a53214cf35538ae2f57622e7a7d7b7597fb9` | test: harden bounded repository commit policy | PASS after lint recovery |
| 136A | `243abd5d7a6f5d3a504e956d8c365919609cd430` | test: fix repository commit Clippy lint | PASS; test-only recovery |
| 137 | `e02cd3b6ebef789531c47b856e841a9df8e8b05f` | feat: compose repository commit through trusted profiles | PASS |
| 138 | `c1a77bdc1a2a20afc677c2292fe4ed5a69e7100f` | test: verify repository commit through generic tool bridge | PASS |
| 139 | `ad7db6d1067a05cb26d37198074375d300eb3e51` | test: validate repository commit through live Codex | PASS |

## Deterministic hardening evidence

`RepositoryCommitPolicy` remains private and narrow; no public generic Git
executor, arbitrary argv surface, or public hash-based authorization constructor
exists. Fixed-command host execution revalidates executable and hooks identity,
and the shared mutation lease covers repository mutators.

The deterministic suite covers policy-generation and cross-policy rejection,
single-attempt accounting, executable/hooks identity revalidation, HEAD/index
races, special states, hostile configuration/hooks/signing/identity/editor,
parser hardening, spawn failure, lease contention, known-no-effect and
uncertain classification, post-observer failure, stale authorization, and no
replay. Task 136 initial CI `33283001044` failed only strict Clippy on a
test-only `chunks_exact_to_as_chunks` portability lint; Task 136A recovery
`33284211437` passed and did not change production authority.

## Trusted Profile and Generic Tool Bridge evidence

The trusted-profile `repo.commit` schema is closed and uses symbolic repository
and executable resources plus trusted host identity. Its public ToolName is
exactly `repo.commit`, its outer permission is Execute, and profile/static or
effective validation does not authorize a snapshot. Composition is atomic and
uses a fresh registry; its effective inventory is redacted.

`RepositoryCommitControl` is host-only. It holds at most one pending
authorization: invalid messages retain it; stale/precondition failure,
known-no-effect, uncertain, and success consume it. It is in-memory only—no
SQLite persistence, profile serialization, restart reconstruction, or model
token exists. Therefore trusted capability enablement is not per-operation
commit authorization.

The Generic Tool Bridge remains neutral routing, not an authorization source.
Codex receives only the private alias `rah_tool_N`; the model-visible definition
remains `repo.commit` and message-only. Deterministic integration tests cover
unarmed refusal, explicit host arming, verified success, Execute denial,
malformed arguments, stale/consumed authorization, completed identical-call
replay, no second commit, and redacted result. The bridge neither conveys a
snapshot token nor a HEAD, branch, index hash, tree, repository path, Git
executable/argv, or identity input.

## Windows certified live evidence

Task 139 exact-head CI `33294858193` passed. Its complete chronological record
is retained in `docs/plans/2026-08-30-windows-live-repository-commit-codex-validation.md`.
Initially installed Codex was 0.150.1, not the certified 0.149.0 baseline.
The official 0.149.0 executable was provisioned side-by-side with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
The first dynamic-tool control then showed that standalone `codex.exe` lacked
its same-version `codex-code-mode-host.exe` companion. The official companion
was provisioned side-by-side with SHA-256
`3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.

The harmless control then passed with lifecycle `1 / 1 / 1`. A fresh disposable
repository subsequently produced the final Windows live `repo.commit` pass:
ToolRequested/ToolStarted/ToolFinished were `1 / 1 / 1`, ToolOutput was
`committed_verified`, and the independently verified fixture HEAD, ToolOutput
OID, and model final OID were all
`13c200c5c772b3e4a0eceb0a2364981c849313e0`. There was no automatic staging,
second commit, retry, replay, approval, or synthetic tool call. The app-server
was reaped, composition shut down, and fixture removed. The markers were
`RAH_REPOSITORY_COMMIT_LIVE_OK` and `LIVE_REPOSITORY_COMMIT_BRIDGE_PASS`.

This fixture commit is live evidence only; it is not a commit in RAH history.
The certified claim is Windows local native Git with the complete official
Codex 0.149.0 runtime. Installed 0.150.1 remained unchanged and was not used.
A bare standalone `codex.exe` is not claimed sufficient for all dynamic-tool
validation.

## Dependency and public API audit

Against the immutable `v0.10.0` baseline, v0.11 commit work introduced no
Cargo manifest or lockfile change: no production dependency, development
dependency, or dependency-version delta was found. The intentional product
surface is bounded `repo.commit` with message-only model input. The public Rust
host composition control is `RepositoryCommitControl` (or its equivalent), but
it is not model-visible and is not generic authority. No version bump occurred.

## Cross-platform status and known limitations

Windows is the only live-certified platform. Windows and Ubuntu CI/platform-
gated tests are deterministic evidence as available; Unix repository-commit
tests passing in CI means deterministically exercised on Ubuntu CI, not live-
certified on Linux. Linux and macOS live repo.commit parity are not established.

Explicitly deferred or excluded: branch create/switch and arbitrary refs;
detached/unborn commits; amend; merge/rebase/cherry-pick; reset/clean/stash;
tags; push/pull/fetch; credentials, remote/network Git; linked worktrees;
submodule/gitlink commit; generic repository delete/rename; generic fs.write;
generic shell/process; network MCP/Streamable HTTP; PluginManager install/update;
profile hot reload; dynamic authority restoration; multi-repository execution;
OS sandbox; network isolation; and rollback.

Task 120 remote/non-loopback llama.cpp successful generation remains
**NOT VALIDATED / DEFERRED**. Transport confinement remains **NOT CLAIMED**;
the v0.11 live Git evidence does not alter either statement.

## Documentation consistency

The roadmap recommendation remains bounded repository commit authority and did
not drift into branch/ref, delete/rename, Desktop workflow expansion, network
MCP, PluginManager, reload, multi-repository, generic filesystem write, generic
Git, or shell/process authority. This audit updates README, architecture, and
security documentation to state the actual v0.11 boundary. Historical v0.10
release records, including `docs/RAH_V0.10_RELEASE_GATE.md`, remain unchanged.
CHANGELOG retains its release-only convention; no unreleased v0.11 release entry
is made by this audit.

## Release-readiness checklist

| Category | Result |
| --- | --- |
| Authority contract / ADR 0016 | PASS |
| Deterministic hardening | PASS |
| Trusted Profile composition | PASS |
| Generic Tool Bridge | PASS |
| Windows live Codex | PASS |
| Dependency audit | PASS — no delta |
| Documentation consistency | PASS |
| Current workspace gates | PASS |
| Exact-head CI | PASS for Task 139; Task 140 requires its own new run |
| Version bump | NOT YET — release-preparation task |
| Tag | NOT YET |
| GitHub Release | NOT YET |

## Audit decision

**V0.11 MILESTONE AUDIT PASS — RELEASE PREPARATION READY**

ADR 0016 remains authoritative. v0.11 delivers bounded host-reviewed repository
commit authority only: `repo.commit` does not imply generic Git; model input is
message-only; Execute alone does not grant commit authority; host review is
required per commit; there is no automatic staging, branch/ref authority,
network Git, retry/replay of uncertain effects, version bump, tag, or release.

Recommended next task, after this audit commit has green exact-head CI:
**Task 141 — RAH v0.11.0 Release Preparation**.
