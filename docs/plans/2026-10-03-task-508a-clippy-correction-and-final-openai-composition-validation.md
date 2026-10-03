# Task 508A — Clippy correction and final OpenAI composition validation

Starting HEAD: `80a937806ef7a44ca2ccf624f5c616b75e3b04be`.
GitHub master independently matched this checkpoint; natural CI `37120001093`
was successful for the checkpoint only. Task 508 WIP was preserved, dirty and
uncommitted. No reset, recreation, provider redesign or Task 508L work occurred.

## Correction and equivalence

Preserved diagnostic: `error: this if statement can be collapsed`,
`clippy::collapsible_if`, `crates/rah-desktop/src/production_composition.rs:49:9`;
warnings-denied Clippy exited 101. Original evidence remains in
`F:/temp/task508-evidence/workspace-clippy.log`.

The sole implementation correction replaces:

```rust
if adapter == runtime_selection::ProductionAdapter::Codex {
    if let Err(error) = model.selection.codex_model_config() {
        // existing connection error assignment and return
    }
}
```

with the exact Clippy-equivalent condition:

```rust
if adapter == runtime_selection::ProductionAdapter::Codex
    && let Err(error) = model.selection.codex_model_config()
{
    // same connection error assignment and return
}
```

Enum equality has no side effects. Configuration validation reads the selection
and constructs/validates configuration without external effects; short-circuit
evaluation still invokes it exactly once only for Codex. The same lock/error
assignment and early return occur on error. The provider-codex cfg remains.
Enabled-feature checks, RAH_RUNTIME_PROVIDER interpretation, explicit selection,
ambiguity rejection, provider precedence and no-provider failure are unchanged.
No helper, suppression, refactor or additional source correction was introduced.

## Executed validation

Evidence root: `F:/temp/task508a-evidence/`. Cargo gates used
`CARGO_TARGET_DIR=RAH_TEST_TARGET_DIR=F:/temp/rah-task504-target`.
Source hashes were recorded and verified unchanged after canonical validation.

| Gate | Result |
| --- | --- |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | PASS, exit 0 |
| cargo fmt --check | PASS, exit 0 |
| cargo check --workspace | PASS, exit 0 |
| git diff --check | PASS, exit 0 |
| Default build / runtime_selection::tests | PASS; 2 passed / 0 failed / 0 ignored; Codex |
| Codex-only build / runtime_selection::tests | PASS; 2/0/0; Codex |
| OpenAI-only build / runtime_selection::tests | PASS; 2/0/0; OpenAI |
| Both build / runtime_selection::tests | PASS; 3/0/0; explicit selection required |
| None build / runtime_selection::tests | PASS; 2/0/0; runtime_adapter_unavailable |
| OpenAI-only task508 production fixture | PASS; 3/0/0; exit 0 |
| Canonical Windows Desktop gate | PASS; 335 passed / 0 failed / 20 ignored; exit 0 |
| Frontend/static | All seven JS syntax checks and six test files PASS, including browser layout and permission inventory |
| Tauri inventory | PASS; 47 matching runtime/manifest/generated/default allows/frontend commands |
| Cargo metadata | PASS; 14 workspace members, all 0.33.0 |
| HostExplicit | Static exactly 11; canonical executable host_allowlist_is_exact PASS |

Feature commands reuse Task 508:

```powershell
cargo build -p rah-desktop --bin rah-desktop
cargo build -p rah-desktop --bin rah-desktop --no-default-features --features provider-codex
cargo build -p rah-desktop --bin rah-desktop --no-default-features --features provider-openai
cargo build -p rah-desktop --bin rah-desktop --features provider-openai
cargo build -p rah-desktop --bin rah-desktop --no-default-features
```

Each mode also ran `cargo test -p rah-desktop` with the same feature arguments
and `runtime_selection::tests -- --test-threads=1`. The fixture ran
`cargo test -p rah-desktop --no-default-features --features openai-fixture task508 -- --test-threads=1`.
Some reduced-feature builds emit existing unused/dead-code warnings; all exited
0. The required all-feature warnings-denied Clippy passed. No further cleanup.

Canonical command:

```powershell
powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:/temp/rah-task504-target -OutputDirectory F:/temp/task508a-evidence/desktop -WatchdogMinutes 10
```

Canonical record:
`desktop/20261003-213645-823-6b6962e2e24e43a4882222204f69633c/status.json`.
Helper build exit 0; Desktop test exit 0; no watchdog timeout; test time 309.67s.
Preserved Task 508 workspace 1,062/0/24, Desktop 335/0/20, Task 499 4 passed,
Task 503 7 passed and Task 504 4 passed remain historical evidence. The entire
workspace test suite was not rerun for this source-equivalent conditional.

## Structural and deterministic proof

Cargo trees use `-p rah-desktop --target x86_64-pc-windows-msvc -e normal,build,dev`
and the corresponding feature arguments. Default: Codex present, OpenAI absent.
OpenAI-only: OpenAI present, Codex absent. None: both adapters absent.
Metadata retains default provider-codex and optional Desktop-to-OpenAI edge;
no version bump, package addition or new edge from Task 508A.

The newly built OpenAI-only executable was copied into the evidence root and
run with PATH empty, OPENAI_API_KEY removed and RAH_RUNTIME_PROVIDER removed:
`--runtime-composition-smoke`, exit 0:
`{"adapter":"openai","codex_compiled":false,"http_requests":0,"passed":true}`.
The smoke uses a fixed fake key and constructs/shuts down the native runtime;
it does not invoke inference. Codex discovery/admission/invocation paths are
feature-excluded; production fixture counters also confirm zero resolver and
Codex runtime construction. This is headless startup proof, not GUI automation.

The loopback-only production fixture passed Connect; ordinary streamed turn
with exactly one completion; one authorized repo.status through HostToolPort,
ToolRegistry/authorized_tool_dispatch and matching function_call_output/call_id;
and Desktop send_chat coordination. Four requests stay on the local fixture
with fixed fake credentials. Disconnect shuts down runtime and revokes Tool
authority. Retained runtime/port fail closed. History and repository generation
survive Disconnect; switching is refused while connected and permitted after
Disconnect; the old port remains rejected after switching and inactive repository
content is absent from Tool output. No public OpenAI request was executed.

## Authority, ADR and closure

Authority/security impact: NONE. ToolRegistry authorization, repository authority
and switching, leases, permissions, Trusted Profiles, mutation uncertainty and
remembered workspace semantics remain unchanged. API keys remain backend-only.
No provider gains Tool authority; no rollback/replay guarantee is added.
ADR-B: ADR 0033 remains sufficient; no new durable architecture decision.

**A — OPENAI PRODUCTION COMPOSITION IS CODEX-FREE AND LIVE-PROOF READY**

**LIVE_PROOF_READY = YES.** This completes Task 508 local validation and authorizes
the requested coherent commit and normal GitHub master push. No live inference,
real key use, paid request or live model-availability proof occurred. Task 508L
requires separate authorization. No tag, release or version bump.

Publication closure requires natural CI for the new exact commit, not checkpoint
CI. The commit SHA, push result, natural run ID/conclusion and final clean worktree
are recorded in the Task 508A final return and external evidence; this report is
included in that commit without a second documentation-only publication commit.
