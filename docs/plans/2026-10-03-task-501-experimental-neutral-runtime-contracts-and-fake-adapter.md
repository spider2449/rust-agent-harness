# Task 501 — experimental neutral runtime contracts and fake-adapter conformance

## Checkpoint and ADR determination

Starting HEAD: `58fa9f69b8202e7e7214457920ff1643ba475b08`.
Both remote master refs matched before work. The original main checkout was clean
at an older SHA and was preserved; this task uses an isolated worktree.

**ADR-B — existing ADRs suffice for an experimental contract.** ADRs 0001 and
0002 already establish the RAH-owned neutral runtime and adapter separation;
ADR 0003 owns the Tool boundary; ADR 0032 owns the typed local failure envelope.
The new module is explicitly experimental, unused by production composition,
and does not amend those accepted decisions. A later promotion into Desktop or
a change to ADR 0006's bridge needs its own architecture review. This choice
follows the guardrails' distinction between durable boundary decisions and
implementation details.

## Contract and implementation

Changed source: `crates/rah-runtime/src/lib.rs` and
`crates/rah-runtime/src/experimental.rs`. The module is workspace-public because
`pub(crate)` cannot be used by future adapter crates, but its name and module
documentation explicitly deny stable API status. No new crate or dependency.
No Desktop or Codex source changed.

The closed `Capabilities` shape describes discovery, native continuation, text
replay, Tool calls, cancellation and streaming. `ModelDescriptor` has only an
identifier and optional label; `ModelSelection` is explicit or runtime default.
`ModelDiscovery::Unsupported` and catalog completeness preserve honest absence.
The factory validates adapter configuration and creates an instance without
repository or registry authority. The instance exposes liveness, discovery,
conversation opening and shutdown. The conversation owns RAH `ConversationId`,
model context and a scoped Tool port; it sends an owned replay or native turn.
`TurnHandle` owns one operation `SessionId`, an existing `RuntimeEventStream`,
and separate cancellation control. Provider identity remains adapter-private.

The host gives adapters a read-only Tool definition snapshot and a request-only
`HostToolPort`. The adapter can submit an untrusted name and arguments and
receive an output plus host-produced Tool lifecycle events. The interface has
no Tool registration, permission mutation, repository selection, or
HostExplicit operation. A future production port must
retain existing registry authorization, generation/currentness, lifecycle event
production, and uncertain-effect ownership. The fake port proves mediation and
denial, not production dispatch. Failures reuse `RuntimeFailure` and
`RuntimeDiagnostic`; provider causes remain process-local and downcastable.

## Fake-adapter conformance

Four deterministic tests use no network, external process, or Codex import.
They cover complete and unsupported catalog discovery; explicit and default
model selection; distinct RAH conversation/provider/operation identities;
multiple native turns with retained private continuation count; separate replay
mode and unsupported native continuation; text delta and terminal completion;
host-authorized Tool request/start/finish events, result consumption and host
denial; supported/unsupported cancellation; typed failure source and safe
serialization; shutdown and later
operation rejection. The negative authority test toggles only host-private
authorization and observes the adapter fail through the port. The port trait's
shape exposes no authority setters or registry handle.

## Neutrality re-check

A hypothetical native OpenAI adapter can privately own an HTTP client, scope
model discovery, translate events and Tool requests, retain private native
continuation if available, and close its transport. No neutral field requires an
executable, PID, thread ID, JSON-RPC, or Codex provider choice. A local adapter
can privately own a process, report no catalog or native continuation, stream
or buffer output, and still use the same Tool port. Optional capability absence
is represented explicitly. Neither adapter is implemented here.

## Validation and disposition

Focused `cargo test -p rah-runtime`: 15 passed, 0 failed, 0 ignored, including
four new fake-adapter tests and three Task 498 failure-envelope tests.

Final-source `cargo fmt --check`, `cargo check --workspace`,
`cargo test --workspace`, warnings-denied full-workspace Clippy and
`git diff --check` passed. The final workspace suite recorded 1,033 passed,
0 failed, 24 ignored across 56 test-result summaries. The first bare workspace
attempt failed only because two CLI fixture executables were absent from the
new isolated target. They were built with the documented fixture command;
setting `RAH_TEST_TARGET_DIR` to that target made the final workspace run pass.
The failed log remains at `F:/temp/rah-task501-workspace-test.log`; the final
passing log is `F:/temp/rah-task501-workspace-test-final.log`.

The canonical Windows Desktop gate passed on final source: 329 passed,
0 failed, 20 ignored; no watchdog timeout. Its evidence directory is
`F:/temp/rah-task501-desktop-gate/20261003-081333-953-867d2702cef841c0a676c77e3f131204`.
Frontend/static Node tests passed 6/0, including the Tauri inventory of 47
runtime, manifest, generated, default-allow and frontend commands.
`node --check` for `status.js` passed. Cargo metadata reported 13 packages,
all version 0.33.0. Static and executable HostExplicit checks confirmed
exactly the established 11; the two executable allowlist/composition tests
passed in the canonical Desktop gate.

Authority/security impact: none in production. No registry, policy, repository,
permission, Trusted Profile, lease, uncertainty, Codex certification, Desktop,
version, release or dependency change. The experimental fake host proves only
the shape and denial path; later production binding needs separate review and
validation. The current interface has no Codex-shaped prerequisite for native
OpenAI or a local runtime.

**Final classification: A — EXPERIMENTAL NEUTRAL RUNTIME CONTRACTS VALIDATED.**
The recommended next task is a narrow adapter-binding design/review for the
production host Tool port and lifecycle ownership before any Desktop migration.
