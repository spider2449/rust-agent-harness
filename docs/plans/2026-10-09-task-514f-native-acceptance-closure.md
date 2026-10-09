# Task 514F - Native provider acceptance closure and release decision

Date: 2026-10-09. Scope: documentation and decision only.

**A — NATIVE DESKTOP ACCEPTANCE COMPLETE; RELEASE PREPARATION AUTHORIZED**

All required local native Desktop production acceptance is complete. Task 515
may prepare v0.34.0 release artifacts. Final public release remains separately
blocked by unresolved OpenAI live acceptance; this report grants no exception.

## Plan and verified checkpoint

1. Verify local HEAD, origin/master, live GitHub master and exact-head CI.
2. Consolidate existing Windows acceptance and retained quality evidence.
3. Inspect release conventions and distinguish preparation from publication.
4. Record the decision, review/stage only this report, check whitespace, commit
   and push normally, then verify master equality and new exact-head CI.

Starting authoritative source: `ba53e58218a27d9ba9968bf143869b1e5bb10e7b`.
Local HEAD, origin/master and the live GitHub master branch API all returned
this SHA. [Exact-head CI 37871957289](https://github.com/spider2449/rust-agent-harness/actions/runs/37871957289)
was completed/success. Tracked checkout was clean. The two untracked Task 514
and 514C reports and all ignored historical test evidence are preserved.

No implementation, version, lockfile, tag or GitHub Release change belongs to
this task. No accepted live UI test, broad Cargo validation, Codex compatibility
work or VCTIP investigation was repeated.

## Consolidated acceptance

| Evidence | Accepted behavior | Exact source / successful CI |
| --- | --- | --- |
| [Task 513](2026-10-08-task-513-provider-connect-restoration.md) | Native OpenAI deterministic production Connect/text/Tool/cancel/reconnect; native llama.cpp Connect and actual streamed chat/host Tool continuation | `4f6315224c2d813dad6503a890e565de52723188`; 37804317343 and 37804313253 |
| [Task 514B](2026-10-09-task-514b-native-desktop-lifecycle.md) | Actual Windows multi-turn history, reconnect, OpenAI missing-key rejection and return to llama.cpp; four rendered responses matched backend completions | `5afc73abcda5b8b8fa6f35ee219b7eddfbad8d4b`; 37859204714 |
| [Task 514D](2026-10-09-task-514d-authority-presentation.md) | Actual llama.cpp/native identity, 17 named Tool details, fs.read Read, repo.status Read-only effect/Execute permission; model-selected repo.status Requested/Running/Completed and unseen filename in final continuation | `44a039c02ff5486decc86603f7fe210efcbf1943`; 37867290438 |
| [Task 514E](2026-10-09-task-514e-native-cancellation.md) | Actual active-stream cancellation renders Chat was cancelled, exactly one cancelled terminal, no generic failure or late delta; subsequent prompt and disconnect/reconnect complete | `ba53e58218a27d9ba9968bf143869b1e5bb10e7b`; 37871957289 |

Task 514D's original permission assertion conflict was explicitly resolved by
the human without changing Tool definitions. Its cancellation presentation
failure remains historical evidence; Task 514E supplies the deterministic
before/after control and actual correction acceptance. Task 514B retains its
original second-turn provider rejection and pre-correction fixture failure.
Task 513 retains its original Tool-normalization failures. The initial Task
514/514C stopped dispositions remain unchanged rather than being overwritten.

Current actual cancellation evidence was reviewed in
`target/task514e/actual-ui/adjudication.json` and `ui-evidence.json`, with backend
events and ownership records identified by the Task 514E report. Its production
executable SHA-256 is
`940A8B89E55A074296ECED1262D28C3F99CC42A2F8EB8C1D80FD978D3E2DF2F3`.
Task 514D's accepted-contract adjudication separately records presentation and
Tool PASS and cancellation FAIL; it is not treated as full cancellation PASS.
No captured raw wire call-ID claim is made. Tool support is accepted for the
tested llama.cpp model/template, not all models. Windows acceptance does not
establish Linux/macOS or installer acceptance.

**Remaining demonstrated llama.cpp/Desktop production defects: none.** This
closes the demonstrated defects in this acceptance scope, not every possible
provider, platform or deployment combination. Task 514E changes terminal
translation/ownership; its report explicitly retains Task 514D's unchanged
presentation and Tool acceptance. No new source change warrants live replay.

## OpenAI boundary and release-policy decision

Status remains **OPENAI_LIVE_NOT_VERIFIED**, because no API credential was
available. Native OpenAI deterministic production tests: **PASS**. Actual
OpenAI missing-key UI handling and recovery: **PASS** (Task 514B). A real
connection to the official Responses API: **UNVERIFIED**, not PASS.

Reviewed sources: repository instructions; README, ARCHITECTURE,
ARCHITECTURE_GUARDRAILS and SECURITY; accepted ADRs 0032/0033; Task 511/513/514
acceptance records; Task 486 and the v0.33 release gate. No explicit existing
rule was found requiring real OpenAI API acceptance at the release-document
preparation stage. Therefore decision B does not apply.

The Task 514 acceptance record's credential-conditional step requires retaining
OPENAI_LIVE_NOT_VERIFIED and obtaining an explicit release decision; its release
record requirements call for an OpenAI live result or approved decision. Prior
release conventions also separate preparation, exact-head certification and
publication. There is no basis to infer an automatic OpenAI exemption from
deterministic tests or earlier Codex release records.

Accordingly the unresolved OpenAI gate is a **publication blocker**: obtain
authorized actual API acceptance, or explicit human authorization of a waiver
with its scope and nonclaims recorded in the release gate. This closure
authorizes preparation only and does not waive that gate. It does not assert
a universal ADR requiring every provider's live test for every release.

## Retained committed quality evidence

Task 514E's committed report and `target/task514e/publication-closure.json`
record final production-source gates:

| Gate | Accepted result |
| --- | --- |
| Native adapters | 23 PASS |
| Neutral runtime | 18 PASS |
| Canonical Windows Desktop | 339 PASS, 13 ignored; no watchdog timeout |
| Workspace | 1,078 PASS, 18 ignored, zero failed across 60 suites |
| Frontend | All 9 suites PASS |
| Tauri permission inventory | PASS, 49 commands |
| Formatting / workspace check / strict all-target all-feature Clippy | PASS |
| Normal production Desktop release build / diff-check | PASS |
| Starting exact-head CI | PASS, 37871957289 |

These are retained executed results, not commands newly run in Task 514F.
Original fixture, setup, assertion and lint failure evidence remains intact.
HostExplicit remains exactly 11. This task changes no ToolRegistry, lease,
permission, repository authority, provider credential, dependency edge, public
API or accepted ADR.

## Task 515 requirements - preparation only

v0.34.0 follows the existing workspace minor-release convention after released
v0.33.0 and covers native production provider composition and the demonstrated
restoration/recovery corrections. Root Cargo version is currently 0.33.0;
all 14 current packages inherit it, including rah-runtime-openai. Previous
v0.33 preparation had 13 packages; do not reuse that old package count.
The live origin v0.34.0 tag query returned no tag.

- Update the workspace/package versions and the corresponding 14 internal
  Cargo.lock records to 0.34.0, verifying no unrelated dependency/checksum drift.
- Reconcile `crates/rah-desktop/tauri.conf.json` bundle version, currently
  0.9.0, with Cargo 0.34.0. Align it to 0.34.0 for this release unless an
  explicitly documented independent bundle-version policy is approved. Audit
  other emitted version metadata and the actual release artifact. The current
  divergence is known preparation work, not an unsafe scope/version blocker.
- Add CHANGELOG native provider/lifecycle corrections; preserve immutable
  v0.33 and earlier publication history.
- Review/update README connection, backend-only key, model/endpoint and recovery
  instructions; ARCHITECTURE native/optional legacy composition; SECURITY fixed
  OpenAI origin, loopback llama.cpp, separate keys and unchanged host authority.
- Create a native provider release acceptance record, including source/artifact
  hashes, exact-head CI, Windows evidence, tested model/template, platform and
  installer nonclaims, the retained failure history and explicit OpenAI gate.
- Document cancellation, next-turn, disconnect/reconnect, missing-key and local
  server recovery. Cancellation does not imply remote/Tool rollback; uncertain
  effects must be inspected before new action and must not be automatically
  replayed. Describe restoring a retained known-good executable/configuration
  as operational recovery, without promising reversal of external effects or
  destructive repository reset.
- Establish and execute the release build/validation matrix: fmt, locked
  workspace check/tests, strict all-target/all-feature Clippy, Cargo metadata
  and lock/version audit, canonical Windows Desktop with fixture helpers,
  frontend suites, Tauri permissions, normal production release build and
  artifact/version checks, diff integrity, normal push/master equality and
  exact-head CI. Record bundle/installer validation separately if claimed.
  Carry forward accepted UI evidence unless relevant source changes justify
  repeating it. Preserve the separate OpenAI live/publication gate.

None of those release-preparation changes is performed by Task 514F. No tag or
GitHub Release is authorized. Task 515 is authorized to prepare artifacts;
final public release is separately blocked as described above.

## Documentation closure

Only this report is eligible for staging and commit. Historical untracked
report SHA-256 values before the task:

- Task 514: `DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43`.
- Task 514C: `A6C834178A8428F3EBD3F3F99A4B254B639DFA1A9A6BEEA3E3B326AD8CAE277F`.

Post-commit SHA, push, final master equality, whitespace checks and exact-head
CI are recorded in the completion response and ignored Task 514F closure
evidence; this report cannot contain its own immutable commit identity.
