# Task 510B1 — certification-only Desktop harness prerequisite audit

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Starting status contained only the untracked Task 510B stop report. It is
preserved, with a reference-only follow-up note.

## Plan and disposition

1. Inspect ADRs 0005, 0030 and 0033, architecture guardrails, security policy,
   and the stopped Task 510B report.
2. Trace H1/H2/H3 through the real adapter and Desktop construction paths.
3. Stop at classification B if crossing the private adapter boundary requires
   a new externally callable construction surface. Do not replace the adapter,
   duplicate its implementation into Desktop, or expose a release bypass.

**B — REAL DESKTOP COMPOSITION CANNOT BE REUSED WITHOUT PUBLIC PRODUCTION API EXPANSION**

No harness mechanism selected or implemented. This is a source-boundary stop,
not a candidate compatibility failure or a failed validation run.

## Construction evidence

- `rah-runtime-codex/src/lib.rs` keeps process, connection and transport modules
  private. Its admission table contains only `codex-cli 0.157.1`; the preferred
  version is also `codex-cli 0.157.1`.
- `process.rs`, `ProcessTransport::start`, resolves the executable, verifies
  production version admission, verifies the real schema, then spawns stdio
  app-server. `ProcessTransport` and `start` are crate-private.
- `experimental.rs`, `CodexFactory::create`, calls that strict start path.
  `Instance` and `Instance::from_transport` are private. The only test transport
  injection is a fake transport, which cannot satisfy this task.
- Desktop's `codex_composition::configured_codex_factory` uses the public
  `CodexFactory::new` and therefore retains strict admission.
- Desktop's private `production_composition::connect_with_configuration`
  can accept a configured neutral factory and shares registry/permission
  composition, preflight, conversation binding, currentness and publication.
  This seam is usable, but no real candidate-eligible Codex factory can currently
  be supplied from Desktop.
- That injected path also omits `model_source::refresh` and its retained source
  snapshot. Merely supplying a factory would not establish that the sole changed
  decision is candidate eligibility; the certification design must retain exact
  artifact currentness and runtime preparation semantics explicitly.

H1 inside the adapter can access process internals but cannot access Desktop's
private backend composition. H1 inside Desktop cannot access the adapter's
private internals: Cargo compiles a dependency without the consuming test
target's `cfg(test)`. H2 examples/integration binaries face the same privacy
boundary. H3 needs a new cross-crate constructor or policy surface on the
adapter. A feature-gated public constructor would still be a new cross-crate
surface; no such feature or constructor was added under this stop condition.

Embedding copies of private adapter modules in Desktop would change the crate
composition being exercised and undermine the required production-adapter
proof. Reversing the production dependency direction is also inappropriate.

## Exact candidate data and required gate

The descriptor remains supplied task data, not measured or admitted here:

- Exact path: `C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe`.
- Expected version: `0.160.0` (readback `codex-cli 0.160.0`).
- Expected bytes: `326872368`.
- Expected SHA-256: `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.

The future gate must require an absolute exact executable path, nonempty exact
version and complete SHA-256; reject missing identity, wrong hash and wrong
version before schema/app-server execution; avoid PATH fallback; retain real
schema inspection; and compare whole-file hashes before launch and after
shutdown. Eligibility applies to that test invocation only. No version or hash
grants Tool, model, provider or repository authority.

Baseline remains `0.157.1`, with supplied SHA-256
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.

## Evidence ledger

| Gate | Result |
| --- | --- |
| Production rejection | Source trace only: strict version rejection precedes schema and app-server startup; executable regression not run |
| Descriptor / missing hash / wrong hash / wrong version / exact path tests | Not implemented or run |
| Real candidate startup / handshake / adapter / neutral runtime / Desktop Connect / Disconnect | Not run |
| Candidate pre/post execution hash | Not measured in this task |
| Inference / Tool execution | None |
| Focused deterministic counts | No tests run; no passing count claimed |
| Workspace fmt/check/test/clippy | Not run: documentation-only prerequisite stop |
| Canonical Windows Desktop suite | Not run |
| Tauri 47/47/47/47/47 | Required invariant; source untouched, inventory not executed |
| 14 packages/members, all 0.33.0 | Manifests untouched; metadata not executed |
| HostExplicit exactly 11 | Authority source untouched; static/executable gates not executed |

## Security and next scope

No production/test source, admission table, baseline, dependency, version,
frontend command, Tauri permission or authority implementation changed. No
environment override, CLI flag, preference, certification Tool or release-build
bypass was introduced. Unknown-version fail-closed behavior remains unchanged
by source inspection. This is not new executable authority certification.

ADR result: **ADR-B — existing ADR sufficient** for the intended exact-artifact
certification process. No ADR was created or altered.

Recommend a narrower internal composition seam task to resolve the cross-crate
test construction boundary before implementing certification execution. It
must specify how the test artifact gets adapter-private exact admission without
making that constructor available to normal production Desktop builds, and how
Desktop retains preparation/source-currentness checks. Preserve neutral APIs,
strict production construction, schema validation and host authority.

Only after that seam and this harness achieve classification A may Task 510B
resume its full exact-artifact schema/deterministic/direct gpt-6.1-sol/Tool/
cancellation/diagnostic/Desktop certification gates, with pre/post hashes.
No model, Tool, cancellation, diagnostics or Desktop turn certification is
claimed here. Task 510B was not resumed. Admission/preferred changes remain
separate work requiring separate authorization.

No commit, push, tag, release or new exact-head CI. Classification B requires
stopping; only classification A authorizes completion/publication. The two
reports remain uncommitted and the worktree is not clean.

Reference-only follow-up: implementation and focused deterministic failure stop
are recorded in [Task 510B2](2026-10-04-task-510b2-codex-cross-crate-certification-construction-seam.md).
The Task 510B1 classification and evidence above remain unchanged.
