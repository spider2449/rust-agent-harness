# Task 509A1 — Frontend state test harness recovery

Starting and final HEAD: `44666d4de3e514339918fab4143f440d35892c8e`.
GitHub master was independently verified with `git ls-remote origin refs/heads/master`
and matched this checkpoint. All Task 509/509A dirty and untracked WIP is preserved.

## Dependency audit and correction

The stopped failure was `ReferenceError: refreshModelConfiguration is not defined`
at `crates/rah-desktop/frontend/runtime_model_state_test.js:51`.
Classification: **H1 — missing harness import/load**.

`status.js` defines `refreshModelConfiguration(invoke)` immediately after
`renderModelConfiguration`. It invokes the existing `model_configuration` backend
command and passes its result to the production renderer. Its caller `loadStatus`
is defined in the same file. Production `index.html` loads that complete file with
`<script src="status.js"></script>`; this is a same-source dependency, not a
missing cross-script production global. The function has no explicit module export
and production does not inject this helper. Its `invoke` parameter is supplied by
production callers.

Existing frontend tests use Node VM/source extraction, including the model-preflight
test and repository tests; the membership tests also load the whole source into a VM.
The new state harness accidentally ended its renderer slice at the start of the
required helper. The only executable correction extends that slice's end from
`async function refreshModelConfiguration` to `function renderRepositorySnapshot`.
The VM now obtains the actual production renderer and refresh helper together.
No copied logic, fake helper, export, module system, production edit, or Rust edit
was introduced by Task 509A1.

## Executed validation

Evidence: `F:/temp/task509a1-evidence/`. Source hashes were captured in
`source-freeze.json`; the tested implementation remained frozen throughout builds.

- First rerun: `node crates/rah-desktop/frontend/runtime_model_state_test.js`,
  exit 0, zero failures. Adapter scoping, native OpenAI configured model, return to
  Codex, no-provider Connect, and all three stale-model fixtures passed.
- Focused frontend: `model_preflight_test.js` PASS, exit 0; syntax checks of
  `status.js` and `runtime_model_state_test.js` PASS; `git diff --check` PASS.
- Prior Rust evidence remains preserved, not rerun: both-adapter 4/0/0,
  presentation 1/0/0, no-provider 2/0/0, OpenAI-only 3/0/0. The both-adapter and
  OpenAI-only logs include the schemas 1–3 historical OpenAI compatibility test;
  stored `provider=openai` remains a Codex upstream preference. Scoped preference,
  identity, old-catalog rejection, and return/revalidation checks remain unchanged.
- `cargo fmt --check`: PASS, exit 0 (`fmt.log`).
- `cargo check --workspace`: PASS, exit 0 (`check.log`); default-feature build
  emits a dead-code warning for the OpenAI identity variant.
- Fixture helper build: PASS, exit 0 (`helpers.log`).
- `cargo test --workspace`: FAIL, exit 101 (`workspace-test.log`).

The workspace build failed before test execution at the Windows linker:

```text
error: linking with `link.exe` failed: exit code: 1103
librah_tools-69e8fd44c9a1c889.rlib(...rcgu.o) :
fatal error LNK1103: debugging information corrupt; recompile module
error: could not compile `rah-tools` (test "repository_file_info") due to 1 previous error
```

Both target environment variables were `F:/temp/rah-task504-target`.
This identifies a linker/debug-information failure, not a state assertion failure.
No fresh-target retry, source correction, or toolchain change followed. The failed
target and logs are retained. The stop rule prevents later gate execution.
Workspace passed/failed/ignored test counts are unavailable because Cargo did not
reach the test runners. This is not a claim that any runtime-state semantics failed.

Warnings-denied Clippy, canonical Windows Desktop, complete frontend/static suites,
Tauri executable inventory, and executable HostExplicit check were not run after
the failure. Desktop passed/failed/ignored counts and exit code are unavailable.
The expected 47-command inventory is therefore not newly certified.

Metadata was independently verified: 14 workspace members/packages, all `0.33.0`;
`metadata.json` records the result. Manifests, lockfile, permissions and ADRs have
no diff. There is no dependency expansion or version change.

## Authority and ADR

Static inspection of `host_invocation::host_kind` and `host_allowlist_is_exact`
shows exactly 11 eligible names, with the closed fallback intact. Executable
certification is pending, not inferred from static inspection or old evidence.
ToolRegistry, permission, repository, Trusted Profile, lifecycle and mutation
authority are untouched by this harness correction. No authority expansion.
ADR-B: no new ADR for frontend test loading; existing ADRs remain unchanged.

## Final disposition

**D — FULL VALIDATION REGRESSION**

The restored production dependency and focused frontend tests pass, but workspace
validation fails at linking. Task 509A remains incomplete. No commit, push, new
exact-head CI, tag, release or version bump; the worktree intentionally remains
dirty. Starting GitHub master remains authoritative. Task 509B and Task 510 were
not begun and are not authorized by this result.

The next prerequisite is a separately authorized investigation of this retained
linker failure, with deterministic validation resumed only under that scope.

Reference-only follow-up: [Task 509A2](2026-10-04-task-509a2-lnk1103-isolated-target-recovery.md)
records fresh-target I1 isolation, resumed full validation, and Task 509A closure.
This note preserves the original stopped evidence and historical classification.
Task 509B's scope remains the provider-aware Model picker: bounded Codex catalog
discovery/freshness and teardown, stale-result rejection, loading/empty/error UX,
validated selection and Connect eligibility, native OpenAI configured-model
presentation and explicitly selected non-live presets, explicit custom-model
policy, fresh Connect validation and Disconnect boundaries, and absent-model /
runtime-source-switch tests. No runtime refresh, live OpenAI work, Task 510,
layout redesign, persistent native OpenAI preference or authority expansion.
