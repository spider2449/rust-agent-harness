# Desktop cross-restart resume and safe context rebinding

Status: Task 114 research/design complete. This document changes no production behavior.

## 1. Current Tasks 110-113 contract

Task 110 keeps a process-local, host-owned `ActiveReplayContext`: completed provider-neutral `User`/final `Assistant` pairs only, at most eight messages and 32 KiB, committed only after `AgentEvent::Completed`. It is bound to current-process repository and model generations. A changed repository or effective model starts a new context; a same-context disconnect/reconnect may retain it. Every operation still uses a fresh Codex thread.

Task 112 persists a bounded, strict JSON v1 *display transcript* only: completed pairs and closed separators. On recovery it restores that transcript, appends `application_restarted`, creates an empty `ActiveReplayContext`, leaves Codex disconnected, selects no repository, and restores no tools, permissions, profile, model, or authority. Task 113 can explicitly delete that fixed private snapshot and clears the in-memory context.

This restart behavior remains the default. There is no automatic cross-restart continuation.

## 2. Product definition

**Resume Previous Conversation** is an explicit user action that imports one selected recovered provider-neutral message sequence into a new current-process `ActiveReplayContext`. It performs no model request and restores no execution environment.

It is different from automatic transcript recovery (display only), authority restoration (never), Codex-native thread resume (not used), New Conversation (an explicit fresh start), and ordinary same-process disconnect/reconnect continuity.

## 3. Transcript vs execution context vs replay context

| Context | Ownership and purpose | Must not be treated as |
| --- | --- | --- |
| Recovered durable transcript | Read-only historical/display records loaded from Task 112 storage | current authority or automatic model input |
| Current fresh host execution context | Connected repository/model generations, fresh ToolRegistry, permissions, and trusted host state | reconstructed from transcript text |
| ActiveReplayContext | Provider-neutral completed messages sent in the next `AgentInput.messages` | a provider session, authority grant, or durable identity |

The only permitted cross-restart flow is:

```text
selected recovered messages + explicit Resume + fresh connected context
    -> new ActiveReplayContext
```

The transcript is model input data only. It never proves repository identity, provider configuration, tool availability, permission, trusted-profile state, prior external effects, or a Codex thread/session.

## 4. Explicit-vs-automatic resume

Automatic resume after a purported identity match is rejected. A match cannot restore authority, and it would make stale text silently influence a newly configured repository/model/tool environment. Automatic recovery remains display-only.

Explicit rebind is preferred: the user deliberately chooses to place historical text into the model request for the *currently connected* context, without RAH claiming that the contexts are identical. The control should be visible with concise supporting text:

> Resume the previous conversation in the current context?
>
> Previous messages will be sent to the currently connected model context. Repository, model, and tool state are not restored from history.

One explicit action plus this persistent explanatory text is sufficient for the first version. A second modal adds friction without creating additional evidence or authority. If UX testing later shows accidental activation, a confirmation modal can be added without changing the authority contract.

## 5. Repository/model identity analysis

No listed candidate is a trustworthy proof that the fresh execution context is the old one.

| Candidate | Stability and privacy | Substitution / semantic weakness | Decision |
| --- | --- | --- | --- |
| Canonical repository path or its hash | Moves and case/symlink behavior change it; path/hash can disclose local topology | A hash is not identity; the path can be reused for another repository | Reject |
| Git remote URL | May be absent, mutable, shared, or sensitive | Does not identify a worktree, branch, local-only repo, or authority | Reject |
| `HEAD` commit | Changes on normal work; no privacy benefit if persisted | Identifies a revision, not repository identity; forks may share it | Reject |
| Git common-dir identity / working-tree fingerprint | Repository moves/worktree changes make it unstable; fingerprints add data and complexity | Still cannot establish present authority or conversational suitability | Reject |
| Provider enum/model identifier/configuration fingerprint | May drift and may expose endpoint/configuration information | Does not prove credentials, endpoint behavior, runtime, registry, or permissions | Reject |
| Process generation counters | Meaningful only within the process that created them | Equal values after restart are coincidental | Reject |
| Host-generated persistent repository ID | Would need lifecycle, storage, move, cloning, and privacy rules | Creates an authority-adjacent identity system but still cannot prove current suitability | Reject |

RAH must not create a fake proof of “same context.” It must not persist repository/model metadata solely to enable automatic matching.

## 6. Rebind semantics

On successful Resume, Rust chooses the eligible recovered sequence, validates it, snapshots the **currently connected** repository and model generations, and replaces the empty `ActiveReplayContext` with those messages bound to that snapshot. It marks the context resumed for local state/UX only and issues no model request. The next normal Send constructs:

```text
resumed completed pairs + new User message -> AgentInput.messages
```

The import makes no identity claim about the old execution context. Future tool calls use only the freshly created current ToolRegistry and all current host policy/sandbox checks.

## 7. Connected-runtime requirement

Resume should require `ConnectionState::Connected`, chat idle, recovered resumable history, and an empty active replay context. The connection has captured the actual repository/model generation snapshot that will receive the next request; without it, an import cannot be safely bound.

If repository or model desired state reports reconnect-required, Resume must fail closed with `ConversationResumeReconnectRequired`. It must not bind history to a known-stale runtime merely because a connection object still exists. Resume does not connect Codex, select a repository, or repair configuration.

## 8. Resumable epoch definition

For the first implementation, the source is exactly the logically resumable conversation immediately preceding the latest `application_restarted` boundary. It contains complete `user`/`assistant` pairs only.

It must not cross `new_conversation`, `repository_changed`, `model_configuration_changed`, `repository_and_model_changed`, or `history_trimmed`. It must not combine arbitrary older epochs. `application_restarted` is the one boundary Resume deliberately crosses:

```text
eligible completed epoch
application_restarted
empty current process context
    -- explicit Resume --> imported eligible pairs
```

For transitive resume, “immediately preceding” means the reconstructed logical chain selected by durable lineage, rather than merely the last physically written records; see sections 13-16.

## 9. Replay bounds

Task 110 limits remain authoritative: no more than eight replay messages and no more than 32 KiB aggregate UTF-8 content. The full selected logical conversation must fit before import. Count exact string bytes, preserve pairs, and do not use token estimates or tokenizer dependencies.

Fail closed if it does not fit. Do not silently keep newest or oldest pairs, summarize, or offer an unbounded selection UI. Recommended sanitized error text is: **“Previous conversation exceeds the replay limit. Start a new conversation context.”** This explains the outcome without byte counts or token estimates.

## 10. Tool/effect handling

Only durable completed User/Assistant pairs are importable. Never import tool requests, activity records, tool arguments, IDs, outputs, or an activity log. Assistant prose about a tool effect is untrusted text, not evidence that the effect occurred.

Every subsequent tool call is evaluated through the fresh ToolRegistry, current permissions/policy, sandbox/executor, and current external state. No old external effect is replayed or trusted.

## 11. Active-context rules

Resume is allowed only when `ActiveReplayContext` is empty. It must reject a live completed context rather than merge independent histories or silently discard it. The user may continue current context or use New Conversation; neither choice merges it with recovered history.

After success, normal Task 110 reconciliation is authoritative. A repository/model change followed by reconnect clears the resumed replay history exactly as it clears ordinary same-process history. Conversely, disconnect/reconnect to the same connected R1/M1 inside the same process preserves the resumed context under existing Task 110 semantics.

## 12. New Conversation / Clear History interaction

New Conversation before Resume intentionally abandons the immediate recovered-resume opportunity for that process. It is a user-owned decision to start fresh, so Resume becomes unavailable until a later recovery boundary creates a new eligible source.

Clear Conversation History removes the persisted source and any in-memory resume candidate. Successful clear makes Resume unavailable; no hidden resume-only copy may survive.

## 13. Durable lineage problem

Without lineage, a resumed old epoch and later newly completed pairs become a linear archive:

```text
Epoch A pairs -> application_restarted -> post-resume Epoch B pairs
```

After the next restart, v1 can identify only B as the physically adjacent sequence. It cannot know that B was semantically built on imported A. One-generation behavior would therefore lose A on the next Resume, despite the product wording “Resume Conversation.” This is not an authority problem, but it is a user-visible continuity limitation that must not be hidden.

## 14. One-generation vs transitive resume

One-generation Resume with no lineage is technically safe, cheap, and v1-compatible, but it delivers a surprising degraded experience after a second restart. It should be rejected as the Task 115 product behavior unless explicitly renamed/documented as a limited “resume recent segment” feature.

Transitive Resume reconstructs the old chain plus post-resume pairs, applies the same fail-closed replay limits, and still begins each restart with empty active replay/current fresh authority. This better matches user expectation while preserving the text-versus-authority separation.

## 15. Strict v1 compatibility issue

v1 deliberately rejects unknown record fields, versions, and separator reasons. Adding a `conversation_resumed` separator/link meaning to v1 would make old v1 readers reject new files or, worse, force a meaning change into a closed format. A resume-specific durable record therefore requires a new version, not an additive reinterpretation of v1.

Duplicating imported pairs into a new v1-like epoch avoids links but is rejected: it visibly duplicates transcript content unless presentation becomes special, multiplies storage across restarts, complicates trimming, increases privacy exposure, and makes archive records no longer cleanly represent displayed events.

## 16. Schema-v2 analysis if needed

Correct transitive Resume warrants a minimal v2 schema. Prefer explicit private epoch lineage over a database or duplicated pairs. Conceptually, v2 has ordered epochs with a persistence-only opaque ID, optional parent epoch ID for a resume link, and the same completed-pair/closed-separator display records. A resumed epoch references its source; recovery can resolve the bounded chain, detect missing/cyclic/invalid links, and then apply Task 110 replay bounds.

The IDs are persistence structure only: not `SessionId`, Codex thread IDs, repository IDs, model IDs, or authority. Monotonic stored IDs are sufficient if their uniqueness is validated; UUIDs are acceptable only if a dependency-free existing facility is available. Do not expose IDs to frontend IPC.

V2 continues to store no repository path, remote, commit, provider/model identity, endpoint, credentials, registry, generations, or tool data. A `resume_previous_epoch` linkage is conversation structure, not authority metadata.

## 17. Migration analysis

Task 115 should support reading valid v1 and v2. For a valid v1 snapshot, parse with the existing strict v1 parser, deterministically map its linear records into v2 epochs, and write v2 only after constructing and fully validating the complete v2 candidate. Use the existing same-directory atomic replacement discipline; retain the old v1 bytes until successful replacement where the replacement API permits it. On migration failure, preserve the valid v1 display/recovery behavior and report a sanitized persistence incompatibility rather than delete or partially convert data.

Do not migrate automatically merely because the application starts if no resume feature needs v2 state. The first successful durable mutation under the Task 115 v2 implementation is a reasonable migration point, provided the migration itself is atomic and retry-safe. V1 should remain readable for the supported desktop migration horizon; future retirement needs an explicit compatibility decision.

Keep `conversation-transcript-v1.json` as the legacy input filename and write a neutral `conversation-transcript.json` as the v2 primary only if startup defines deterministic precedence (valid neutral v2 wins; otherwise valid v1 is migrated/read). This avoids calling a v2 payload “v1,” allows old binaries to continue reading their own v1 file, and makes migration rollback clearer. Exact temporary/quarantine/clear rules must target both fixed private names only.

## 18. Resume-model decision matrix

| Model | Authority clarity | Repository correctness | Provider neutrality / portability | Intent clarity | Complexity / stale risk | Privacy metadata | Reconnect behavior | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Automatic after identity match | Low/ambiguous | Apparent, not proven | Medium | Low | High / high | Requires misleading identity data | Complicated stale matching | Reject |
| Explicit rebind under current connected context | High | Does not claim sameness; current authority is fresh | High | High | Moderate / bounded | None beyond transcript structure | Uses existing reconciliation | **Adopt** |
| Codex-native thread/session resume | Low outside Codex | Native state/tool snapshot ambiguity | Low | Medium | High / high | Provider thread metadata | Complex reconnect recovery | Reject |
| No Resume capability | Highest | Highest | High | Clear | Low / low | None | N/A | Reject: insufficient continuity product |

## 19. Durable-lineage decision matrix

| Model | Transitive continuity | Schema complexity | Storage growth / transcript clarity | Migration impact | Provider neutrality | Bounds/corruption handling | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| One generation, no lineage | No | None | Low / clear | None | High | Existing v1 | Reject for named Resume product |
| Duplicate imported messages | Yes | Low/hidden | High / duplicate and ambiguous | V1 behavior distortion | High | Repeated growth/trimming complexity | Reject |
| V2 explicit epoch lineage | Yes | Moderate, bounded | Low / clear inherited structure | Explicit v1-to-v2 migration | High | Validate links, cycle/missing failure closed, then apply bounds | **Adopt** |
| Native provider thread persistence | Provider-dependent | High | Opaque / poor archive fidelity | High | Low | Provider partial-state recovery | Reject |

## 20. Security/authority analysis

The proposed design preserves all existing boundaries:

- Model input and persisted text are never authorization.
- Repository/model/profile/ToolRegistry/permission state is fresh host state at connection time.
- No current state is recreated from a transcript or lineage link.
- No old tool effect, shell/process authority, uncertain action, or native provider state is replayed.
- No automatic Codex connection, repository selection, or reconnect occurs.
- Fresh Codex threads plus explicit `AgentInput.messages` remain the provider-neutral execution model.

No `AgentRuntime`, `AgentInput`, `rah-session`, `rah-protocol`, Generic Tool Bridge, ToolRegistry, or CodexRuntime change is needed. `rah-session` remains operation-oriented; any general reusable durable conversation abstraction is a later separately justified design.

## 21. Privacy analysis

V2 lineage can remain provider- and repository-neutral. It persists only bounded conversation structure and eligible completed text, which already has known at-rest sensitivity. It needs no path, Git remote, commit, working-tree fingerprint, provider endpoint/model, credential, generation, registry, or tool metadata. Normal user-account filesystem protection and Clear Conversation History remain the current privacy controls; encryption, cloud sync, and multiple conversation management remain non-goals.

## 22. Proposed UX state machine

| State / event | Resume availability | Result |
| --- | --- | --- |
| Fresh startup, no valid recovered source | Unavailable | Empty active context |
| Valid recovery, disconnected | Unavailable | Display transcript only |
| Connected, idle, current state not stale, empty context, candidate exists | Available | User may explicitly import candidate |
| Connected but reconnect required | Unavailable | Show reconnect-required state |
| Chat running | Unavailable | No state mutation |
| Active replay has completed turns | Unavailable | Continue current context or choose New Conversation |
| New Conversation | Unavailable for this recovery | Abandons source in current process |
| Clear History | Unavailable | Deletes source and clears state |
| Successful Resume | Unavailable until another restart/recovery boundary | Active context is resumed and bound to current identity |

The frontend should explain availability at a product level but must not show paths, IDs, generations, thread IDs, configuration, credential, or registry internals.

## 23. Proposed Task 115 IPC/error contract

Add one closed Rust-owned IPC command: `resume_previous_conversation`. It accepts no transcript records, indices, IDs, paths, generations, provider/model values, or repository data. Rust selects and validates the exact eligible source.

On success it may return only `{ "status": "resumed" }`; it should not return message payloads or internal counts. Use closed sanitized errors such as `ConversationResumeUnavailable`, `ConversationResumeBusy`, `ConversationResumeReconnectRequired`, `ConversationResumeTooLarge`, and `ConversationResumePersistenceIncompatible`. Frontend maps them to user-facing text and receives no internal diagnostics.

## 24. Windows live acceptance design

Task 115 live acceptance should use a dedicated app-data test location and verify:

1. create a durable completed conversation containing a distinct marker;
2. close/restart and verify display recovery plus `Application restarted`;
3. verify Codex remains disconnected and no repository/tool authority was restored;
4. connect a fresh current context and explicitly Resume;
5. send a marker question and observe the expected marker as supplemental evidence;
6. deterministically verify the constructed request contains eligible replay pairs plus the new prompt and that the fresh registry/current authority is used;
7. restart again, explicitly Resume again, and verify the original marker remains available through the transitive chain;
8. verify oversize/malformed lineage fails closed, then clean up only the dedicated test data.

Deterministic Rust tests are authoritative for selection, link validation, replay construction, limits, error closure, and fresh-context binding. Model recall is supplemental only.

## 25. Strong Task 115 recommendation

Implement **explicit user-initiated transitive Resume Previous Conversation** as a Desktop-private host feature with a **schema v2 explicit epoch-lineage migration**.

- Require idle `ConnectionState::Connected`, current configuration not reconnect-stale, an empty `ActiveReplayContext`, and a recovered candidate.
- Bind selected completed User/Assistant pairs to the fresh current repository/model generation snapshot; make no same-context claim.
- Do not auto-match identity, persist repository/model metadata, restore authority, reuse native Codex threads, or expose internals.
- Require the entire reconstructed logical chain to fit eight messages and 32 KiB; otherwise fail closed with a sanitized error.
- Preserve Task 110 reconciliation: context changes clear resumed history; same-process same-context reconnect retains it.
- Migrate valid v1 snapshots deterministically and atomically to v2 only when needed by the Task 115 durable mutation path; keep v1 readable during the supported migration horizon.

Schema v1 is safe only for a consciously limited one-generation feature. Because the requested product is named Resume Conversation and users reasonably expect repeated-restart continuity, Task 115 should choose v2 lineage rather than silently ship that limitation.

## 26. Explicit non-goals

Task 114 and the recommended Task 115 design do not implement production behavior; change frontend or Rust now; add a Resume button; alter Cargo dependencies; modify AgentRuntime, AgentInput, rah-session, rah-protocol, CodexRuntime, Generic Tool Bridge, ToolRegistry, repository authority, or persistence files in this task; add repository/model identity metadata; add summarization/tokenizers; persist native Codex sessions; automatically reconnect/select repositories; add named/multiple conversation management; or add an ADR. No new dependency edge or public API change is required.
