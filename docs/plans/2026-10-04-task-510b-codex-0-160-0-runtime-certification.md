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
