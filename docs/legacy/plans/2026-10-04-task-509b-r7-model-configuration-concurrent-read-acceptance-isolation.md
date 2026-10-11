# Task 509B-R7 — Concurrent model_configuration read acceptance isolation

Starting HEAD: `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`.
Evidence: `F:/temp/task509br7-evidence/`. R6 evidence remains unchanged in
`F:/temp/task509br6-evidence/`; before/after SHA-256 manifests compare equal.
All R2–R6 implementation files match the R6 source freeze. R7 makes no product
source edit. The only R6 report change is an appended reference to this report.

## Scope and plan

Preserve the dirty, unstaged, uncommitted, unpushed Task 509 WIP. Inspect the
existing request lifecycle, perform actual-app single-reader Advertised Apply,
then exactly one bounded diagnostic reader if Phase A passes. Classify before
product correction. Use normal DOM observations and bounded observational
frontend tracing; do not poll model_configuration during normal acceptance.
Resume full acceptance only after isolation; run fresh local deterministic gates
before A. Only A permits commit, normal GitHub master push and natural exact-head CI.
No Task 510B, inference, live native OpenAI or 0.160.0 certification.

The R6 L2 command remains `async fn model_configuration(app: AppHandle) ->
ModelConfigurationPresentation`. No command lifetime redesign was needed.
Relevant README, architecture, guardrails, security and accepted ADRs 0030/0033
were inspected. R6 deterministic evidence remains established, not invalidated.

## Original R6 failure and diagnostic-reader ambiguity

R6 selected advertised gpt-6-astra and clicked Apply while its harness repeatedly
invoked model_configuration. UI remained generation 2/request 2 Loading with
Connect disabled; a later command returned advertised_catalog and
advertised_unverified for that same generation/request. All original logs remain.

R7 reused the actual R6 Desktop binary (both provider features), its isolated
application profile, explicit certified-baseline executable and a new evidence-
local WebView profile. CLI verification reports exactly `codex-cli 0.157.1`;
measured artifact is `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
CDP accesses the actual `http://tauri.localhost/` page through local port 9509.
The saved R6 Advertised preference was restored to Inherit through normal Reset.
No inference or repository Tool invocation was sent.

## Phase A — single-reader PASS

`phase-a.js/json`: no auxiliary model_configuration call during Apply, no
parallel snapshot polling. Only production provider/picker/Apply handlers ran;
the harness inspected frontend globals and DOM while waiting.

Inherit was coherent at generation 2/request 2 with nine actual runtime-advertised
IDs and Connect enabled; gpt-6.1-sol was absent. Apply of gpt-6-astra reached
**generation 3/request 3, Advertised, advertised_catalog, advertised_unverified,
compatibility unverified, Loading false, Connect enabled**. Selected ID and
truthful runtime advertisement provenance were retained. The consumed DTO and
rendered source agreed. A subsequent stable command inspection (`post-a.json`)
returned the same resolved authoritative snapshot. H3 was not reproduced.

## Phase B — exactly one diagnostic reader reproduces

`phase-b-prepare.js/json`, `phase-b.js/json`, `phase-b-post.js/json`.
After normal Reset reached generation 4/request 4, the harness introduced one
bounded diagnostic read immediately before the normal Apply refresh read.
No diagnostic polling loop was used. Observational wrappers recorded invocation
start/end and normal render calls; no source state was changed or completion
held. All instrumentation disappeared when the app closed.

| Operation | Relative time from diagnostic start | Result |
| --- | --- | --- |
| diagnostic reader B starts, invocation 4 | 0 ms | owns discovery |
| normal refresh reader A starts, invocation 5 | 0.2 ms | observes owned Loading |
| A returns and UI consumes | 1.7 ms | generation 5/request 5 Loading |
| normal trailing status read, invocation 7 | 2.6–3.3 ms | same Loading snapshot consumed |
| B completes | 11031.3 ms | generation 5/request 5 advertised_catalog / advertised_unverified |
| DOM deadline | 30 seconds | UI still Loading, Connect disabled |

B's correct completion was never a production render/publish operation. A later
stable command inspection returned the same ready generation 5/request 5 while
UI still held Loading for generation 5/request 5. There was **one source request**,
not competing request IDs or newest-request replacement. This distinguishes the
harness-owned response from a newer ready DTO actually consumed by the UI.

## Normal product concurrency and classification

`normal-pair-prepare.js/json`, `normal-pair.js/json`, `normal-progress.json`:
normal Reset returned Inherit at generation 6/request 6; after draining trailing
reads, two clicks on the enabled production Apply button invoked its real
handlers. No diagnostic source read ran during this test.

The first normal refresh (11) owned generation 7/request 7. The second (12)
returned Loading for the same owner. First refresh completed with the catalog
at approximately 10.071 seconds; its superseded frontend refresh token did not
render over the newer token. Its normal trailing loadStatus read (16) then
returned and consumed the resolved catalog at approximately 19.559 seconds.
UI converged to Advertised/unverified, Loading false and Connect enabled within
the original 30-second criterion. Normal reads did not allocate competing source
requests. No product command/event subscription or source freshness rule changed.

**H1 — acceptance harness observation race** is the causal classification for
the R6 reproduction. H2's competing request ownership was not observed; H3 did
not occur in Phase A. H4's failure to consume a ready normal frontend response
was not observed: the diagnostic response had no UI consumer, and paired normal
handlers recovered through their existing trailing status read. These are bounded
observations, not a claim that every possible UI interleaving has been tested.

## Exact read/Apply ownership semantics

`model_configuration_for_state` calls `model_source::refresh(state, false)`.
A presentation read **can initiate discovery**, allocate a request ID and install
Loading when the context has no owned source. Context rebinding increments source
generation. With source already Some(Loading), `!force && owner.source.is_some()`
returns that same snapshot immediately; another read does not begin a new request.
Stable cached Codex reads also measure artifact freshness; changed artifacts can
revoke source evidence. A read is therefore not universally a passive snapshot.

Apply calls configuration_changed/bind; it does not itself begin discovery. The
subsequent normal presentation read creates the source ticket. Resolve publishes
only for the current generation/request. Connect's existing explicit forced
refresh is separate from presentation reads. No ownership correction is needed
for the measured H1 cause; a general pure-read lifecycle redesign is outside R7.

## Harness correction and remaining actual acceptance

Added `scripts/task509b-r7-acceptance.cjs`: actual-app single-reader harness that
waits for production boot, selects an advertised model, clicks Apply and observes
consumed/rendered state and DOM Connect eligibility. It never invokes
model_configuration. Its final run passed (`corrected-harness.json/.exit`, exit 0),
with a fresh Inherit-to-Advertised transition. This corrects the perturbing polling
used in R6. R6 evidence-local harness/logs are not overwritten.

Actual acceptance results:

- Advertised gpt-6-astra selection/provenance/Loading clearance/Connect eligibility
  PASS; existing local Connect establishes `connected`; disconnect PASS.
- Explicit Custom input, structurally valid gpt-6-astra, exact `Custom · unverified`
  provenance and local Connect PASS. Catalog membership did not convert Custom.
- Custom persistence PASS across disconnect/reconnect and actual process reopen:
  Custom mode, ID, provenance, picker choice and Connect eligibility restored.
- Provider openai → ollama PASS with local/model:tag: generation 1/request 1 →
  generation 2/request 2 on the reopened process, source rebinding and unverified
  compatibility; no previous rendered selection persisted. Stale success/error
  rejection remains proven by the freshly rerun deterministic source tests;
  no live old-result completion was artificially injected in this switching case.
- Malformed Custom draft refused; Connect disabled and no invalid selection published.
- Inherit Reset PASS: runtime_default, no explicit model/mode, inherited_unverified,
  Connect enabled. Explicit model override with provider inherit refused with
  model_configuration_invalid.
- Native OpenAI configured-only actual-app PASS: RAH_OPENAI_MODEL=gpt-6.1-sol,
  configured source, exactly one configured picker entry, no Codex catalog,
  no Custom/presets, read-only picker and hidden custom input. Child had no API key;
  no native Connect/API request or inference was sent. The configured source path
  resolves from host configuration without invoking a network client.

Evidence: advertised-custom.js/json (initial harness interruption),
custom-continuation-corrected.js/json, reopen.js/json,
persistence-switch-inherit.js/json and native.js/json. The first disconnect wait
used the wrong literal `disconnected` instead of production `not connected`;
that bounded harness timeout did not represent a product failure. Continuation
used observed state and did not replay Connect/disconnect. An initial continuation
referenced r7Select before the interrupted script had installed it; correction
installed that DOM helper and proceeded. Outputs remain separate. The first
attempt to wrap the immutable Tauri internal invoke property had no effect;
effective tracing wrapped frontend refresh/status helpers instead. No product
file was edited for any harness/instrumentation correction.

## Fresh deterministic closure

Both target variables: `F:/temp/rah-task509a2-target-run2`. Product hashes verified
against R6 before gates; no source edits occurred while build/tests ran. No linker
corruption or target recovery occurred. Gates executed serially and all returned 0:

- presentation `model_configuration_presentation_is_closed_and_sanitized`: 1/0/0;
- focused `model_source::tests`: 17/0/0;
- all nine existing node --check checks and seven frontend suites PASS;
  corrected harness syntax also PASS;
- Tauri inventory **47/47/47/47/47**;
- HostExplicit executable `host_allowlist_is_exact`: 1/0/0; closed static match
  remains exactly **11**;
- cargo metadata: **14** packages, all **0.33.0**; Cargo.lock unchanged;
- cargo fmt --check, cargo check --workspace, cargo test --workspace,
  cargo clippy --workspace --all-targets --all-features -- -D warnings,
  git diff --check: PASS;
- workspace **1085 passed / 0 failed / 24 ignored**, Desktop segment **358/0/20**;
- canonical Windows Desktop **358/0/20**, helper/test/overall exit 0,
  watchdog false, run `20261004-162027-595-f9b08972a8584d68add7e45e879f49f8`.

Logs and exit files live under the R7 evidence directory. The initial PowerShell
gate runner treated Cargo's ordinary stderr progress as a terminating native
error; runner stderr policy was corrected before serial gates, with explicit
exit-code checks retained. No Rust/test failure triggered this correction.

## ADR, dependency and authority/security impact

**ADR-B**, no new ADR. No R7 product/dependency/command/capability changes.
Task 509 includes the established R3 direct production sha2 dependency in
rah-runtime-codex; no new RAH crate edge or lockfile change. ToolRegistry,
repository authorization/selection/switching, leases, permissions, Trusted
Profiles, mutation uncertainty and remembered-workspace semantics are unchanged.
HostExplicit exactly 11. Codex current admission/preferred baseline remains
0.157.1. No 0.160.0 certification, version bump, tag, release or live OpenAI work.

## Task 509 disposition and final staged-check stop

**F — LATER VALIDATION OR AUTHORITY REGRESSION**

After the successful gates, staging the complete Task 509 WIP exposed a final
`git diff --cached --check` failure (exit 2) in the previously untracked R6 report:

- line 199: whitespace-only compiler-diagnostic line;
- lines 206, 212, 218, 248 and 249: trailing whitespace in preserved compiler output.

This is a report/patch hygiene failure, not a concurrency, Rust, frontend or
authority regression. The executed unstaged `git diff --check` gates passed;
they did not include the untracked R6 report. Final staged validation includes
that report and therefore prevents A. Do not reinterpret the earlier gate as
proof that the complete candidate patch passed whitespace validation.

Stopped immediately before commit/push. No diagnostic text was trimmed or
rewritten, consistent with preserving R6 and the reference-only R6 change.
`final-staged-diff-check.log/.exit`, `failed-staged.patch` and
`failed-staged-stat.txt` preserve the failed staged candidate. After retaining
that evidence, `git restore --staged` on the exact Task 509 candidate paths
restored the original unstaged index state and preserved all worktree content.
The R7 report is completed with this stop classification; no source correction
or further validation retry followed it.

HEAD remains `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`. No new commit, push,
exact-head CI, tag, release or version bump. Worktree remains dirty and unstaged;
Task 509 is not complete. Publication nonresults and index/worktree state are
recorded in `F:/temp/task509br7-evidence/publication.json`.

Next bounded recovery must resolve the preserved compiler-excerpt whitespace
contract without losing the original R6 evidence, then recheck the complete
candidate patch before publication. R7 does not authorize that report correction.

Task 510B remains deferred and was not started. Its exact scope, contingent on
Task 509 A and publication closure, is Codex runtime certification under ADR 0030:
exact artifact/schema audit, deterministic regressions, direct and Desktop
certification evidence, and explicitly reviewed admission/preferred-baseline
update. No general UI redesign or native OpenAI work is included.
