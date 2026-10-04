# Task 509A2 — LNK1103 isolated-target recovery

Starting HEAD and independently verified GitHub master:
`44666d4de3e514339918fab4143f440d35892c8e`.
Task 509/509A/509A1 dirty WIP was preserved. No product source, test, Cargo,
dependency, toolchain, or host configuration correction was made by this task.

## Original failure and isolation

Task 509A1's `cargo test --workspace` exited 101 before any tests ran while
linking `rah-tools` integration test `repository_file_info`:
`LNK1103: debugging information corrupt; recompile module`, in
`librah_tools-69e8fd44c9a1c889.rlib(...rcgu.o)`.
Original logs remain at `F:/temp/task509a1-evidence/`; the original target
`F:/temp/rah-task504-target` was neither deleted nor cleaned.

Evidence for this task: `F:/temp/task509a2-evidence/`.
The first previously nonexistent target was `F:/temp/rah-task509a2-target`.
Its execution session ended during compilation without an exit result; no test
build process remained. Its `workspace.log` and partial artifacts are retained.
This incomplete attempt supports no classification.

The conclusive run used the separately verified previously nonexistent target
`F:/temp/rah-task509a2-target-run2`, with the exact command:

```powershell
$env:CARGO_TARGET_DIR = 'F:/temp/rah-task509a2-target-run2'
cargo test --workspace
```

It initially waited for Cargo's package-cache lock; editor-owned metadata
processes were left untouched. Compilation, linking and all tests then passed.
`workspace-run2.log` and `workspace-run2-exit.txt` preserve the outcome.
Before/after SHA-256 checks of every tracked and untracked repository file
reported zero changes during isolation. Implementation/test hashes also match
the prior Task 509A1 source freeze; its older report hash predates the existing
reference-only note. No source was changed between failed and isolated builds.

**I1 — stale/corrupt target artifact.** Fresh isolated workspace validation
passed: **1065 passed / 0 failed / 24 ignored, exit 0**, summing all Cargo test
and doc-test summaries. I2 and I3 were not observed. No source fix is warranted.
Removing/rebuilding the original stale target is separate developer maintenance.

## Resumed deterministic gates

| Gate | Result |
| --- | --- |
| Isolated workspace tests | 1065 / 0 / 24; exit 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS; exit 0; isolated target |
| Canonical Windows Desktop | 338 / 0 / 20; exit 0 |
| Complete frontend/static | PASS; exit 0 |
| Tauri executable/full inventory | PASS; exit 0; 47 matching commands |
| Metadata | PASS; 14 members/packages, all `0.33.0` |
| HostExplicit static and executable | PASS; exactly 11 |

Canonical command:
`powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:/temp/rah-task509a2-target-run2 -OutputDirectory F:/temp/task509a2-evidence/desktop`.
Canonical evidence run: `20261004-103617-994-238051d5ba674ab5b6e9aeeada9be17f`.
The harness prepared echo fixtures and used ordinary parallel Desktop execution.

Complete frontend validation checked syntax of every frontend `.js` file and
executed all six `*test.js` files: model preflight, remembered workspace browser
layout, remembered workspace behavior, repository membership, runtime/model
state, and effective authority. `frontend.log` retains all results.
`node crates/rah-desktop/tauri_permission_test.js` certified 47 runtime handlers,
manifest commands, generated permissions, default allows, and frontend commands.
`metadata.json` records current package metadata. Manifests and lockfile have no
diff: no dependency or version drift. Prior unchanged-source fmt/check/diff and
focused adapter-feature evidence from Task 509A1 is reused, not claimed rerun.

Static `host_invocation::host_kind` retains 11 named arms and the closed fallback.
The canonical run passed `host_invocation::tests::host_allowlist_is_exact`.

## State separation and authority acceptance

Runtime adapter identity remains explicit and host-composed. Codex upstream
provider/model persistence is unchanged: schemas 1–3 `provider=openai` retain
historical Codex meaning. Native OpenAI uses its startup configured-model snapshot
and ignores saved Codex preferences. Returning to Codex restores preferences and
requires fresh scoped validation; old catalog observations cannot transfer
validity. No-provider composition fails closed. Model validity is adapter scoped.
Current full tests and preserved both-adapter/OpenAI-only/no-provider focused
evidence establish these semantics; no live inference or entitlement is claimed.

No authority expansion, new dependency edge, public/core API change, permission
mutation, repository-selection change, ToolRegistry bypass, or ADR change.
Existing host authority and lifecycle boundaries remain intact; HostExplicit 11.
No Task 509B picker implementation, runtime refresh, live OpenAI request, Task 510,
tag, release, or version bump was performed.

**A — RUNTIME ADAPTER AND MODEL-PROVIDER STATE ARE SAFELY SEPARATED**

Task 509A is complete and Task 509B is authorized but not begun. Publication uses
the authorized coherent commit `refactor: separate runtime and model provider state`,
normal GitHub master push, and natural exact-head CI; final publication evidence
is returned separately so the report does not require a self-referential commit.

## Exact Task 509B scope

Provider-aware Model picker: bounded Codex catalog discovery/freshness with owned
teardown and stale-result rejection; loading/empty/error UX; validated selection
and Connect eligibility; native OpenAI configured-model presentation and, if
explicitly selected, non-live supported presets; explicit custom-model policy;
fresh Connect validation and Disconnect boundaries; absent-model and runtime/source
switch tests. No runtime refresh, live OpenAI work, Task 510, layout redesign,
persistent native OpenAI preference, or authority expansion.
