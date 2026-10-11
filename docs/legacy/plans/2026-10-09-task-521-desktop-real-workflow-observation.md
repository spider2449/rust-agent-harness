# Task 521 - Desktop real-workflow UX observation

Date: 2026-10-09. Observation only.

## Decision

**C - MORE HUMAN OBSERVATION REQUIRED**

Actual human repository work occurred and the human confirmed the individual
results below, then ended the test. This is partial acceptance evidence, not
automated acceptance. Repeated file-list failure is a concrete impediment, but
its failing layer and regression status are not established. No v0.35 capability
or frontend improvement is selected.

## Plan and checkpoint

1. Verify repository identity, starting master, CI and immutable release.
2. Ask the human to operate the existing Desktop and report actual experience;
   observe supplied screenshots without synthetic activity or agent inference.
3. Record only supported observations, preserve private evidence locally and
   publish this documentation file with exact-head CI verification.

Local HEAD, origin/master, live GitHub master and advertised origin master
matched `8d8455933c059e90ea991fc3b2d4a09eec76fd7c` before edits.
Origin identifies `spider2449/rust-agent-harness`.
[Starting CI 37892046771](https://github.com/spider2449/rust-agent-harness/actions/runs/37892046771)
was completed/success for that exact SHA.
The v0.34.0 annotated tag object remains
`504c298947f81f531364d99c1189b8281ddf5a6f`, targeting
`7b972988f2f116b87b3770d66be1ef970e20b726` locally and through GitHub.
The existing release remains published at `2026-10-09T05:55:25Z` with zero
uploaded assets. No release or historical record was changed.

## Actual session and human assessment

The human operated Desktop, reported successful activation and Git checking,
and supplied two screenshots showing native llama.cpp, a connected runtime,
Chat and Inspector Activity side by side. The normal-width screenshots are
approximately 1790px wide; no 476px observation was supplied. Exact executable
provenance, model identifier and server context configuration were not captured.
An early process census found llama-server running but no rah-desktop process;
that census predates the screenshots and does not establish session health.

| Goal and performed action | Expected behavior | Actual evidence and completion | Friction and limits |
| --- | --- | --- | --- |
| Activate repository and check Git | Active repository; useful status answer | Human reports activation without error and Git check normal | No material friction reported for these actions. Selection sequence, staged/modified/untracked explanation and Status/Diff/Review inspection were not captured. |
| Review codebase and list files through Chat | Repository inspection followed by useful response | Human reports both requests fail with `Chat failed runtime operation failed at chat turn.` | Actual workflow failure reported; codebase-review Tool identity and failing layer unknown. |
| Continue a short conversation | Send a new request | Screenshot S1 shows `Chat ready` with `Conversation context limit reached; start a new conversation context`. Human says context seemed small and sending was no longer possible. | User-observed confusion and interruption. Source confirms a Desktop replay limit, not a measured llama.cpp token limit. |
| Recover using New Conversation | New request accepted | Human explicitly confirms sending and receiving an answer works after New Conversation; S2 shows the new-context separator | Workaround succeeded. This does not prove file listing recovered. |
| Search repository content in the new context | Tool completes and answer uses results | S2 shows repo.search Requested, Running and Completed, plus a search answer; human confirms search works | No material friction reported for search. Underlying matching content was not independently validated. |
| List files after successful search | Tool completes and returns a list | S2 shows the list request, empty assistant entry, chat runtime failure and visible repo.list Requested/Running rows; human confirms listing fails | Repeated workflow interruption across reported attempts, with no captured root cause. No terminal repo.list row is visible in the bounded screenshot; unseen rows and actual backend execution state are unknown. |
| Create a sample untracked file | Authorized creation with clear result | Human reports successful sample-file creation | Completion is human-reported only. Chat-versus-manual-Host route, terminal Tool state, review/authorization and disposable-repository status were asked about but not confirmed before testing ended. No safety violation is established or dismissed. |

The human confirmed outcomes, including recovery and the search/list contrast;
they did not provide an overall navigation/attribution acceptance assessment.
No unintended action, drawer obstruction, repeated scrolling or extra-click
count was reported. Their absence from reports does not establish absence in use.
No agent-directed mutation, cleanup or live model retry was performed.

## Attribution and source comparison

S2 makes a plausible visual association between search/list requests and nearby
Tool rows, but the human did not confirm reliable per-turn attribution. S1 shows
multiple repo.list Requested/Running rows. Their number is not a proven count of
distinct Tool calls: these are lifecycle event rows. Failed or uncertain Tool
outcomes and model-versus-manual Host distinction were not human-accepted.
Completed search rows are visibly distinguishable from Requested/Running rows;
the failure banner and visible nonterminal list rows leave an unresolved
presentation question, not proof that a Tool remained running.

Current source inspection found Desktop replay checks at more than 8 history
messages or more than 32 KiB of message-content UTF-8 bytes. Normal successful
user/assistant pairs therefore reach the message guard before a sixth request
after five completed pairs. Resumed history or long content can affect this.
Actual history size was not measured. The context-limit message is distinct
from the generic turn-operation failure; S2 demonstrates listing failure after
new-context recovery, so the earlier replay limit cannot explain that failure
without additional evidence.

Before selecting any remedy, compare the existing Inspector tabs, sticky section
navigation, collapse/restore, Focus Chat and opt-in narrow drawers. Those controls
already exist in layout.js. Activity has separate model and Host event rendering;
Host rows use `Host action - not Model` presentation. Source availability does
not establish human comprehension. No speculative redesign or backend change
is recommended from this session.

## Bounded private evidence

Original human screenshots remain outside the repository and are not published.
They contain private paths and repository content; only necessary UI observations
are transcribed here. SHA-256 identifies the supplied artifacts:

- S1: `EA78562E1EC3A8F7B5B39DB47B1B9D061308D600850145585AE905ECB913C6F1`.
- S2: `F79F4D6D28F9A7A671C4E151F2A0D2832B92DA966955C42CEE1AB29E9BD3D1F5`.

No screenshot editing, copying into public reports, credentials collection or
unrelated repository-content inspection was performed. Desktop/provider logs
for the failed list turn were not supplied or collected. Generic diagnostics
do not identify a root cause.

## What remains unobserved

- Human-confirmed attribution of Tool activity to individual conversation turns,
  and differentiation of manual Host actions from model-requested execution.
- Terminal Failed/uncertain Tool presentation during the failed list operation,
  with bounded diagnostic evidence identifying the failure layer; no D regression
  claim is justified yet.
- A completed Status/Diff/Review inspection and return-to-Chat workflow, including
  staged/modified/untracked comprehension.
- Navigation at approximately 476px, drawer obstruction, existing-controls
  workarounds and the human's overall friction assessment.
- Mutation test provenance, disposable-repository status and authorization route.
  Further mutation acceptance requires those prerequisites; prefer read-only work.

Further observation should address these gaps without artificial Tool calls,
retries-to-green or reopening historical staged-diff investigations. A concrete
list failure may warrant a separate bounded diagnostic task; this report does
not authorize implementation or assume a frontend remedy.

## Validation and preservation

Only this Markdown report changes. No dependency, ADR, runtime, provider, Tool
permission or authority impact; HostExplicit remains 11. No version bump, tag,
release, reset, clean or historical rewrite. Rust/frontend suites and production
builds are not rerun for this documentation-only change.

The three historical untracked reports remain excluded, with SHA-256:

- Task 514: `DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43`.
- Task 514C: `A6C834178A8428F3EBD3F3F99A4B254B639DFA1A9A6BEEA3E3B326AD8CAE277F`.
- Task 518: `2A5A78D7E5D9C06784BB7FDC14EFFC7675F7F616793C1406489AF00D7E6B0206`.

Publication requires diff integrity, an explicit single-file commit, normal push,
local/origin/live master equality and exact-head CI success. Resulting commit
identity and CI are post-commit facts reported at closure, not predicted here.
