# Task 510B — Codex 0.160.0 runtime certification

Reference-only follow-up: [Task 510B1 harness prerequisite audit](2026-10-04-task-510b1-codex-certification-only-desktop-harness.md).
This reference does not change this report's classification or resume certification.

Starting HEAD and local origin/master: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Initial worktree clean. Starting exact-head CI `37189461776` is the supplied
Task 509 historical checkpoint; no new CI verification was performed here.

## Plan and stop condition

1. Read ADR 0030 and relevant architecture/security material.
2. Measure baseline and exact candidate, then inspect whether an existing
   bounded certification-only Desktop path can run the real artifact.
3. Only if that prerequisite exists, perform schema, deterministic, direct,
   Tool, cancellation, diagnostic and Desktop certification gates serially.
4. Preserve production admission and preferred baseline throughout.

**B — DESKTOP CERTIFICATION REQUIRES WEAKENING PRODUCTION ADMISSION**

Stopped at the task section 21 prerequisite. No existing real-artifact
certification-only admission mechanism was found. Do not substitute fake
transport tests or historical direct success for Desktop certification.

## Exact artifact audit

| Field | Certified baseline | Candidate |
| --- | --- | --- |
| Reported version, measured with --version | codex-cli 0.157.1 | codex-cli 0.160.0 |
| Bytes | 322515248 | 326872368 |
| SHA-256 | `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574` | `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d` |
| LastWriteTimeUtc, descriptive only | 2026-09-26T08:20:41.5289688Z | 2026-10-02T11:18:58.4901097Z |
| PE machine, measured | 0x8664, Windows x86_64 | 0x8664, Windows x86_64 |

Baseline path:
`C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe`.

Candidate path:
`C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe`.

Both executable hashes were measured twice, before and after --version checks,
and matched the identities above. No live/direct certification probes ran;
these measurements are not a pre/post live-probe immutability certification.

Candidate provenance: existing global npm package; local @openai/codex
package.json reports 0.160.0. No acquisition, installation or external supply
chain verification occurred. Adjacent bin inventory contains codex.exe and
codex-code-mode-host.exe (74697520 bytes). Actual companion requirements were
not exercised. Baseline historical certification remains unchanged.

## Desktop prerequisite and admission evidence

Source inspected at the starting HEAD:

- `crates/rah-runtime-codex/src/lib.rs`: current admission contains only exact
  codex-cli 0.157.1; preferred version is unchanged.
- `src/process.rs`, ProcessTransport::start: resolve executable, verify_version,
  verify_schema, then spawn app-server. check_version returns VersionMismatch
  for a successful --version output outside that exact set.
- `src/experimental.rs`, CodexFactory::create: real executable creation uses
  ProcessTransport::start. Its cfg(test) fixture seam supplies a fake transport,
  not a candidate process with narrowly authorized admission.
- `crates/rah-desktop/src/codex_composition.rs`: test resolver/factory seams do
  not supply a real candidate admission policy. Replacing the runtime factory
  with a fake would not certify the real neutral Desktop/runtime composition.
- Public legacy CodexRuntime connect paths also call ProcessTransport::start.

Thus source inspection establishes rejection before app-server startup and
therefore before inference or Tool execution. This is not a newly executed
production rejection probe. No production admission changes or bypass occurred.
Unknown 0.160.1/0.161.x and other unlisted versions remain rejected by the same
exact-set policy.

## Certification evidence ledger

| Required gate | Task 510B outcome |
| --- | --- |
| Protocol/schema audit: initialize, catalog, config, thread, turn, streaming, Tool, completion, cancellation, shutdown, errors | Not run after prerequisite stop; no compatibility conclusion |
| Deterministic adapter/model/catalog/neutral/preflight/admission/Tool/lifecycle suites | Not run; no current counts |
| Runtime-global catalog count and gpt-6.1-sol advertisement | Not measured here |
| Fresh direct gpt-6.1-sol turn | Not run |
| Authorized repo.status Tool round trip | Not run |
| Candidate cancellation/shutdown/process cleanup | Not run; no app-server started by this task |
| Task 498 diagnostic/typed-cause/sanitization regression | Not run |
| Real candidate Desktop Connect/catalog/preflight/turn/Disconnect | Blocked by missing certification-only path |
| Desktop repo authority/currentness/stale-handle/lifecycle checks | Not run |
| Workspace/canonical Desktop/frontend/static validation | Not run; documentation-only stop record |
| Tauri 47/47/47/47/47 | Starting Task 509 evidence only; no command-surface changes |
| 14 packages/members, all 0.33.0 | Starting checkpoint requirement; no manifests/dependencies changed; not re-executed |
| HostExplicit exactly 11, static/executable | Historical Task 509 evidence only; not re-executed |

Task 510A previously recorded 11 candidate advertised models, gpt-6.1-sol
present, provider-unbound model/list, a successful direct turn, and limited
schema checks. Those are historical research facts, not Task 510B certification.
Task 509's runtime-advertised/provider-unverified picker policy is unchanged;
no model-string special case, provider validation or hot-switch is introduced.
Disconnect remains the recomposition boundary.

## Authority, ADR and conclusion

Only this documentation file changed. ToolRegistry, repository authority and
switching, leases, permissions, Trusted Profiles, mutation uncertainty,
remembered workspaces, frontend, public APIs, dependencies, versions and
admission tables are untouched. No model/provider/version/thread metadata
authority was added. This is preservation by unchanged source, not new live
authority certification.

ADR result: **ADR-B — existing ADR sufficient**. ADR 0030 requires live Desktop
certification in addition to direct/schema/deterministic evidence. No new ADR.

The exact candidate matches the requested hash, but remains uncertified and
unadmitted. Recommend a separate bounded certification-harness task: an
explicit certification-only path bound to exact artifact identity, unavailable
to normal production admission and retaining all host authority checks. Then
repeat Task 510B's full evidence gates. Do not change the preferred baseline.

Task 510C — authorize and apply Codex 0.160.0 preferred/admission baseline update
is appropriate only after classification A and separate explicit authorization.
It is not the next executable step after this B disposition.

No commit, push, tag, release or new exact-head CI: the task authorizes publication
for a completed certification record, and certification stopped at B. This stop
record remains an uncommitted new file for review; final worktree is consequently
not clean. No production source or artifact was modified.

Reference-only follow-up: the construction seam implementation and deterministic
failure stop are recorded in [Task 510B2](2026-10-04-task-510b2-codex-cross-crate-certification-construction-seam.md).
Task 510B remains stopped; its historical classification and evidence are unchanged.


## Task 510B-R4L - implement 14-second C2 policy (2026-10-05)

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Evidence: `F:\Temp\rah-task510br4l-evidence`; starting binary patch, WIP hashes
and original observer/host test retained. All historical stops preserved.

Policy source: `crates/rah-tools/src/repository_observer.rs:33`, STATUS_TIMEOUT,
10s -> 14s. Existing authoritative constant supplies constructor, diagnostic
status scope and final status allowance; three diagnostic-only labels now use
that same constant. No new timeout abstraction or arithmetic change.
Fresh validation + final status retain one elapsed envelope; final child gets
actual positive 14s minus elapsed, with no reset, reservation or aggregate
change to layout children. Lease waiting remains outside; synchronous validation
is non-preemptible. C2 is not a hard end-to-end dispatch deadline.
Layout PROBE_TIMEOUT stays 5s. TERMINATION_GRACE stays 2s, used independently
for child reap and each pipe-reader join. Other observer policies unchanged.
Deterministic controlled times assert 14s exactly, elapsed 0/6/13s and 14s-1ns,
and refusal at 14/15s; forced-child fixture backdates from the policy constant
to retain its one-second control. Host sample no longer imposes an obsolete
10s full-dispatch target; it still requires successful valid dirty status and
normal requested/started/finished lifecycle.

Focused serial tests: observer 8/0/1 ignored; layout 20/0; boundary 19/0;
status 4/0; process-policy 13/0; sandbox 12/0/1 ignored; provenance 1/0.
All eight semantic probes and boundary walk retained byte-identically;
linked-worktree, dirty-fixture and fail-closed layout tests passed.
Scoped check rah-tools/rah-runtime/rah-sandbox with live-test-support PASS.
Scoped Clippy same crates, all-targets/all-features, -D warnings PASS.
cargo fmt --check and git diff --check PASS. Existing external R4 target reused.
Pre-host diff guard: only observer and host test changed from starting hashes;
no probes, optimization, ToolRegistry or authority edits.
ADR-B: numeric host policy within existing semantics, no new ADR/dependencies.
Task 058 / 43c33f2 earliest supported 10s policy, Task 060 implementation;
numeric historical rationale unknown, no ADR mandates exactly 10s.
14s is the approved empirical bounded allowance, not a reliability guarantee.
R4K boundary 6.1044s / markers 0.4188s / remainder ~5.6857s remains future
performance research only; no boundary optimization in R4L.
Host validation and final disposition recorded below after execution.

Host lane: exactly one designated control, six additional serial samples, then
one final control. All PASS, no retries; each authorizes through the ordinary
HostToolScope -> registry -> Execute permission -> fresh repository validation
-> final status route, returns 19 dirty entries, eight successful layout probes
plus final status (nine captured PIDs), no child timeout/termination. Each
captured PID absent from CIM checks. Scope drained and normal lifecycle asserted.
Initial residual regex omitted Option(Some(pid)); corrected read-only extraction
verified all nine captured PIDs for every sample; no host dispatch was replayed.

| Control | Dispatch | Boundary validation | Cumulative before status | Remaining | Status allowance | Status envelope |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| designated | 6.6779835s | 6.2351921s | 6.6163534s | 7.3836466s | 7.3835086s | 57.4449ms |
| sample-1 | 6.5432627s | 6.1057641s | 6.4829305s | 7.5170695s | 7.5169112s | 56.2586ms |
| sample-2 | 6.5515418s | 6.1069046s | 6.4895551s | 7.5104449s | 7.5103461s | 57.6735ms |
| sample-3 | 6.587103s | 6.1225141s | 6.5269541s | 7.4730459s | 7.4728799s | 56.0691ms |
| sample-4 | 6.5713217s | 6.1454645s | 6.509586s | 7.490414s | 7.4903121s | 56.7758ms |
| sample-5 | 6.5200452s | 6.0894607s | 6.4589704s | 7.5410296s | 7.5408957s | 57.0293ms |
| sample-6 | 6.7224218s | 6.2457255s | 6.6597969s | 7.3402031s | 7.3400987s | 58.3248ms |
| final | 6.4015965s | 5.9714091s | 6.340758s | 7.659242s | 7.6591325s | 56.6881ms |

Designated walk: 6.2350519s, marker checks 434.6208ms, 6,580 directories /
111,287 entries. Probe order/durations and cumulative elapsed/remaining:

| Probe | Duration | Cumulative | Remaining C2 |
| --- | ---: | ---: | ---: |
| ["rev-parse", "--show-toplevel"] | 45.6936ms | 49.9673ms | 13.9500327s |
| ["rev-parse", "--path-format=absolute", "--absolute-git-dir"] | 42.988ms | 93.038ms | 13.906962s |
| ["rev-parse", "--path-format=absolute", "--git-common-dir"] | 41.6931ms | 134.8253ms | 13.8651747s |
| ["rev-parse", "--is-bare-repository"] | 42.0137ms | 176.9612ms | 13.8230388s |
| ["rev-parse", "--show-superproject-working-tree"] | 53.8851ms | 230.9286ms | 13.7690714s |
| ["rev-parse", "--path-format=absolute", "--git-path", "index"] | 42.2815ms | 273.3091ms | 13.7266909s |
| ["rev-parse", "--path-format=absolute", "--git-path", "HEAD"] | 41.9157ms | 315.3086ms | 13.6846914s |
| ["worktree", "list", "--porcelain", "-z"] | 62.118ms | 377.509ms | 13.622491s |

Raw .stdout/.stderr/.exit, host-results.json and per-control residual-verified.json
retain complete evidence. Status envelope includes policy/process overhead;
allowance captured at execute-process entry differs slightly from immediately
preceding boundary remainder because intervening work consumes the same clock.
Optional R4H wall-marker env was not enabled; full dispatch and phase timings
and process-local R4I/R4J argv/PID/outcome provenance remain captured.
No claim of an independently measured authorization interval or hard deadline.

Frozen bundle read-only SHA-256/length/fsutil file-ID verification PASS:
- codex.exe: fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d;
  326872368 bytes; 0x000000000000000000110000000c1861.
- codex-code-mode-host.exe: 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6;
  74697520 bytes; 0x000000000000000000180000000c1862.
No bundle executable launched. HostExplicit exact eleven-name host_kind mapping
and host_allowlist_is_exact assertion inspected, unchanged; no Desktop test run.
ToolRegistry, HostToolScope, authorization, active repository authority, linked
gitfile/registration, submodule exclusion, private/common Git identity, index/HEAD,
production Codex admission and preferred baseline unchanged. Every other starting
WIP file byte-identical; no new dependencies, public APIs or authority semantics.

Final classification: **A — 14-SECOND C2 POLICY IMPLEMENTED AND HOST LANE VALIDATED**.
Implementation eligible for isolated publication. Source policy/test can be
separated via audited index content from the diagnostic hunks without changing
working files. Only policy constant plus self-contained deterministic budget test
and this R4L report are eligible; historical unpublished certification stays WIP.
Publication/CI disposition and exact R5 decision follow below. R5 not begun.

Isolated publication candidate: HEAD source plus only STATUS_TIMEOUT 10 -> 14
and the self-contained deterministic remainder test; no diagnostics/features or
new dependency edges. Observer 8/0, scoped rah-tools check and all-targets /
all-features Clippy -D warnings, fmt/diff checks PASS. Candidate preserves
child_timeout and all repository validation code. Working files were not
replaced or reverted to prepare it. Audited two-file staged scope is source
policy/test plus R4L-only report appended to the checkpoint report; unpublished
R1-R4K historical additions and instrumentation remain in the original WIP.
No Codex certification, admission change, tag or release. Normal GitHub master
push and exact-head CI required for publication and R5 authorization. Until
exact-head CI PASS, R5 remains blocked; R5 will not start automatically.
