# Desktop durable conversation persistence

Status: Task 111 research/design complete. This document changes no production behavior.

## 1. Current Task 110 behavior

Task 110 keeps a process-local, host-owned `DesktopConversationState`. Its active replay history contains only completed provider-neutral `User` and final `Assistant` pairs, bounded to 8 messages and 32 KiB. A pair is committed atomically only at `AgentEvent::Completed`; failed, cancelled, incomplete, streamed-delta, and `ToolFinished` records are not replayed. Repository/model context changes and New Conversation clear that replay history while leaving visible transcript records separated by a safe context-boundary label. Each operation still starts a fresh Codex thread; neither `AgentRuntime` nor the Generic Tool Bridge changed.

## 2. Product capability definition

Two capabilities are distinct:

| Capability | Meaning | Decision for first durable version |
| --- | --- | --- |
| Durable transcript recovery | Completed prior discussion remains readable after the application restarts. | Adopt. |
| Cross-restart conversation continuation | Recovered messages are supplied to a new model operation. | Do not adopt automatically. |

Restored content is data, not authorization, repository selection, tool availability, active model proof, completed-side-effect proof, or provider-native thread state.

## 3. Durable transcript vs active replay context

Task 112 should model two private host structures:

```text
DurableTranscript       = bounded completed display records across context epochs
ActiveReplayContext     = current-process, bounded messages eligible for next AgentInput
```

On restart, `DurableTranscript` is restored for display and `ActiveReplayContext` is empty. The fresh process creates its own repository/model/profile/tool state before every operation. Loading a transcript sends no model request and does not connect Codex.

## 4. Process-generation instability across restart

`repository_generation` and `model_generation` in Task 110 are counters meaningful only inside their creating process. A later process can recreate zero (or another equal number) while targeting a different repository or effective model. Persisting them and comparing equality is therefore unsafe and is rejected as a durable identity mechanism.

## 5. Recovery semantic options

Option A, never auto-resume replay, persists readable transcript but starts an empty replay epoch. It has the clearest authority semantics and requires no durable repository identity.

Option B, automatic continuation after a stable identity match, would need a durable repository identity and model/configuration fingerprint. A canonical path is mutable and a path hash adds no identity; Git metadata can change or be absent; a provider/model identifier does not prove live credentials, endpoint behavior, or tool/profile authority. Even a match could only permit fresh host reconstruction, never restore authority. This is too much cross-restart policy for the first durable feature.

Option C, explicit user re-bind/Resume Conversation, improves intent but confirmation alone cannot establish that repository-A discussion is appropriate for repository-B. A stable identity remains useful, and the host would still need to rebuild all authority. Defer until the product has a deliberately specified rebind identity and warning UX.

Option D, persist process generations, is rejected for the reason above.

## 6. Recovery decision matrix

| Option | Product continuity | Authority clarity | Repository/model correctness | Privacy requirements | Implementation / UX complexity | Stale-state risk | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Transcript-only restore + fresh replay epoch | Readable history only | High | High: no cross-process claim | Transcript only | Low / low | Low | **Adopt for Task 112** |
| Automatic replay after stable identity match | High | Medium | Identity is difficult to make durable and meaningful | Requires durable identity metadata | High / low | Medium/high | Defer |
| Explicit user re-bind/resume | Medium/high | Medium | Confirmation does not prove context suitability | Requires identity and warning design | Medium/high | Medium | Defer |
| Persist process generations | Apparent only | Low | Invalid across processes | Adds misleading metadata | Low / low | High | Reject |

## 7. Persisted-data classification

Persist only a strict, bounded transcript snapshot: format version, ordered records, completed `user`/`assistant` message pairs, and safe separators. A private durable conversation identifier and timestamps are unnecessary in version 1 and must not be added merely for speculation. Model/provider display text is also excluded: it risks misleading users about current selection and does not make history more readable.

Never persist model deltas, tool activity, tool arguments/results, `ToolCallId`, runtime `SessionId`, Codex thread/turn IDs, request IDs, absolute repository paths, environment, credentials, tokens, headers, endpoints, stderr, cwd, executable/profile paths, permission snapshots, or ToolRegistry inventories. Future durable audit history, if desired, needs a separate privacy and schema task.

The first durable contract persists only fully completed pairs and explicit safe separators. It deliberately omits a submitted prompt with a failed/cancelled/crashed operation and all partial assistant text. This preserves the Task 110 completed-only, no-uncertain-effects rule; transient failure presentation remains process-local.

## 8. Storage location

Use the Rust-side Tauri application-local-data directory (`app.path().app_local_data_dir()`), with a fixed private filename below it. Tauri exposes `AppLocalData` as an application-specific base directory; on Windows this follows the current user's local application-data convention rather than a hardcoded user path. Do not write selected-repository `.rah` data, use cwd, Documents/home, or expose an arbitrary chooser. See the [Tauri path API](https://v2.tauri.app/reference/javascript/api/namespacepath/).

## 9. Storage-format options

1. A versioned JSON snapshot is inspectable, uses existing `serde`/`serde_json`, is naturally bounded, and can be atomically replaced.
2. An NDJSON journal tolerates a partial trailing line only with a more complex replay contract; it needs compaction, deletion rules, and a growth policy.
3. SQLite supplies transactions and migrations but adds a dependency, deployment surface, and database-shaped lifecycle for one bounded transcript.
4. `rah-session::SessionStore` is operation/session-oriented (`SessionId` and `SessionStatus`) and is not a Desktop durable-conversation abstraction.

## 10. Storage-format decision matrix

| Format | Implementation complexity | Dependency impact | Crash safety / atomic update | Corruption recovery | Schema migration | Size bounding / deletion | Privacy exposure | Portability | Semantic fit | Future multiple conversations | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Versioned JSON snapshot | Low | Existing serde only; Windows replacement binding likely needed | Strong replacement pattern | Reject whole file safely | Explicit version | Simple bounded replacement/reset | One bounded local file | Good | High | Adequate later migration | **Adopt** |
| NDJSON journal | Medium | None | Partial-tail policy needed | Complex replay | Per-event evolution | Compaction required | Persistent append history | Good | Medium | Good | Reject for v1 |
| SQLite | High | New dependency | Strong transactions | Good | Good | Good | Database file | Good | Low for one transcript | Good | Reject for v1 |
| `SessionStore` reuse | Medium/high | May spread session semantics | Undefined for this purpose | Undefined | Not a storage format | Undefined | Undefined | RAH-wide | Low | Premature | Reject |

## 11. Atomic replacement / crash safety

Task 112 should serialize and validate the complete bounded candidate before touching disk, create a uniquely named temporary file in the **same destination directory**, write all bytes, call `File::sync_all`, close it, then replace the existing destination with Windows `ReplaceFileW(destination, temporary, NULL, 0, NULL, NULL)`. `ReplaceFileW` combines replacement steps and requires the files to be on the same volume; it documents defined error states, including cases where the old destination is absent and the replacement remains under its temporary name. See [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew).

For the first write, when the destination does not exist, use same-directory `MoveFileExW(temp, destination, MOVEFILE_WRITE_THROUGH)` without copy-across-volume flags. For an existing snapshot, do not implement delete-then-rename. Rust `std::fs::rename` currently maps to Windows APIs whose precise behavior may change, so it is not the specified persistence primitive; a narrow Windows binding is justified in the Windows-only desktop crate. See [Rust rename](https://doc.rust-lang.org/std/fs/fn.rename.html) and [MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).

This is best-effort durable replacement, not a power-loss-proof guarantee. It aims never to expose a partially written JSON document as the current snapshot; a failed replacement leaves either the previous complete snapshot or a safely ignorable temp file where the API documents that outcome. Windows does not provide a portable Rust directory `fsync` contract appropriate to claim directory-metadata durability here; Task 112 must not claim more than file flush plus Windows replacement semantics.

At startup, safely delete only temp names matching the application's fixed filename plus its private random suffix, in the already-resolved app-owned directory. Never glob/delete arbitrary files. A temp file is never recovery input.

## 12. Corruption handling

| Condition | Startup behavior |
| --- | --- |
| File absent | Start fresh; no warning. |
| Empty, invalid JSON, truncated, unsupported version, structurally invalid, oversized, malformed roles/separators, odd/non-paired messages | Fail closed: do not restore any content or replay context; preserve/quarantine the original under a fixed app-owned diagnostic name if a safe rename succeeds; show “Previous conversation could not be restored.” |
| Stale private temp | Ignore as content, then best-effort remove safely. |

No partial recovery occurs in v1, and corrupt data is never sent to a model. The frontend warning must not disclose a storage path, parser detail, provider data, or internal IDs.

## 13. Schema/versioning

Use a closed `version: 1` JSON document. Deserialize with explicit record enums and reject unknown enum values, unknown top-level/record fields, wrong roles, missing fields, and all versions other than exactly `1`. There is no migration machinery until a second supported version exists. A future version is rejected rather than interpreted as a compatible older record.

Conceptual shape:

```json
{"version":1,"records":[{"kind":"completed_pair","user":"...","assistant":"..."},{"kind":"context_separator","reason":"application_restarted"}]}
```

The actual implementation should use typed Rust structs/enums, not dynamic JSON values.

## 14. Size bounds

Persist a bounded visible archive, not only the active replay context: at most 16 context epochs, 64 completed pairs, 79 total records (64 pairs plus at most 15 separators), 16 KiB UTF-8 per user/assistant string, and 256 KiB serialized file bytes. These limits are independent of Task 110's 8-message/32-KiB active replay limit.

Before each durable commit, form the complete candidate and measure serialized UTF-8 bytes. To fit, evict the oldest **whole closed epoch** only; never split a completed pair or delete only a separator. If the current epoch alone cannot fit, retain it live but report a sanitized persistence failure and do not write a partial/newer-only document. Task 112 should make the archive truncation visible through a safe `history_trimmed` separator; this is display-only and non-replayable after restart. The implementation must reject an oversized incoming file before allocation proportional to its declared contents.

## 15. Persistence commit points

Write the snapshot synchronously after the host has atomically committed a successful completed User/Assistant pair, after New Conversation has added its separator, and after any repository/model context separator has been created. A graceful close may make a redundant attempt but is never the sole commit point. Failed, cancelled, incomplete, and partial turns create no durable content.

For v1, a disk-write failure returns a sanitized persistence warning/error after the in-memory completed result exists; it must not roll back the completed runtime result or manufacture an uncertain retry. Serialize/write under a host-owned persistence mutex so competing UI actions cannot overwrite each other's candidate.

## 16. Startup recovery sequence

1. Resolve the application-owned local-data directory.
2. Bound-read, parse, and validate the snapshot; safely handle stale private temps.
3. Restore only validated completed-pair/separator presentation records.
4. Create an empty fresh-process `ActiveReplayContext`.
5. Append/present `application_restarted` after restored records, and persist it when its bounded candidate can be written.
6. Leave Codex disconnected; do not select a repository, restore tools/profiles, or issue any model request.

## 17. Frontend recovery contract

Add one closed, deterministic Rust-owned command, `conversation_transcript`, returning only display records: `completed_message` with a `user` or `assistant` role and text, plus `context_separator` with a closed safe reason. Invoke it after the existing frontend boot/status sequence. Do not extend startup status with storage data and do not expose paths, generations, internal conversation IDs, repository paths, model identity, or credentials. A recovery-warning event or a sanitized field on this transcript response is acceptable; push-only recovery is not required.

## 18. Application-restarted context boundary

`application_restarted` is required in Task 112 when a valid stored transcript is recovered. The UI text is:

```text
New conversation context
Application restarted
```

It says that preceding content is readable but outside the current live replay context. It is not an error, provider-disconnect claim, authorization event, or proof that a prior provider thread exists.

## 19. Privacy / at-rest limitations

Conversation text can contain proprietary code, prompts, generated code, filenames, and repository facts. The first version relies on normal Windows user-account filesystem protection for application-local data; it provides no application-level at-rest encryption. This limitation must be documented plainly. A clear-history control is product-important, but encryption/key management and cloud synchronization are deferred.

## 20. Clear/delete semantics

New Conversation does not delete prior durable transcript; it records a new context boundary. A distinct “Clear Conversation History” is destructive and should be a later, separately authorized task unless Task 112 explicitly includes it: it must be Rust-owned, require explicit user action, target only the fixed app-owned snapshot/temp files, be serialized against active writes, and never touch a repository. This research task adds neither action.

## 21. Multiple-conversation deferral

Version 1 stores one bounded Desktop transcript/current history. Named chats, recents, project grouping, browser UI, open/delete lists, and database migration are deferred. The snapshot's versioned record envelope permits a later explicit migration without prematurely introducing session management.

## 22. rah-session ownership analysis

Persistence should remain Desktop-private initially. `rah-session`'s `Session`, `SessionId`, `SessionStatus`, `AgentContext`, and `SessionStore` model runtime operations and memory storage, not a durable visible Desktop conversation with restart/rebind semantics. Reusing them would conflate operation lifecycle with product history and could pull Desktop persistence policy into a provider-neutral crate. Promote a new abstraction only after another host demonstrates the same contract.

## 23. API / ADR impact

No public RAH API, `AgentRuntime`, protocol schema, `rah-session` API, ToolRegistry, Generic Tool Bridge, or provider configuration change is required. No ADR is required: this is private Desktop product state and adds no authority boundary. The Windows-only desktop crate may receive a narrowly scoped Windows replacement binding as an implementation detail in Task 112; its new dependency edge, if any, must be reported then.

## 24. Security/authority invariants

- A persisted transcript is untrusted model data, never authorization.
- Fresh host state recomputes repository, model, trusted profile, permissions, and registry for every new operation.
- No persisted grant, registry inventory, profile, Codex thread, or provider-native state is resumable authority.
- Transcript text never proves an external action happened; uncertain/partial activity is not persisted or replayed.
- Every future tool call still goes through parsing, ToolRegistry, permission/policy, sandbox/executor, and tool execution.
- No shell/process authority or automatic reconnection is introduced.

## 25. llama.cpp/provider implications

The format is provider-neutral. It stores neither llama.cpp context size/executable/endpoint nor provider session/thread IDs. Recovery never automatically replays merely because llama.cpp is selected. Provider context limits remain an operator concern; Task 112 neither chooses context size nor probes a provider.

## 26. Strong Task 112 recommendation

Implement one private, bounded `version: 1` durable Desktop transcript snapshot in the Tauri application-local-data directory. Persist only completed User/Assistant pairs and safe separators; exclude tools, partial/failed turns, authority, paths, credentials, runtime IDs, and provider metadata. Use strict validation, same-directory temp writing, `sync_all`, `ReplaceFileW` for existing snapshots, and `MoveFileExW(..., MOVEFILE_WRITE_THROUGH)` for first creation. On valid recovery, show the transcript plus `Application restarted`; create an empty active replay context, remain disconnected, and restore no authority. Fail closed with a sanitized warning on any invalid snapshot. Do not use SQLite, native Codex-thread resume, automatic rebind, or an `AgentRuntime` change.

Exact proposed Task 112 boundary:

- Likely files: `crates/rah-desktop/src/main.rs`, a new private `crates/rah-desktop/src/conversation_persistence.rs` if it keeps `main.rs` narrow, `crates/rah-desktop/frontend/app.js` and `index.html` only for transcript loading/separator/warning presentation, and Windows-only crate dependency/configuration only if required for the replacement binding.
- Schema: the typed v1 envelope above; limits are 16 epochs, 64 completed pairs, 79 records, 16 KiB/message, and 256 KiB serialized bytes.
- Algorithms: the replacement and startup sequences in sections 11 and 16, including strict whole-file validation and no partial recovery.
- Frontend: closed `conversation_transcript` display contract only; no storage details.
- Commit points: completed-pair and safe-context changes; close is redundant only.
- Non-goals: clear/delete UX, multi-conversation management, encryption, cross-restart resume/rebind, activity/audit persistence, and provider-native restoration.
- Deterministic tests: valid v1 round trip; all invalid/corrupt cases; limits/whole-epoch trimming; completed-only commit behavior; startup creates empty replay state; separator ordering; no forbidden fields; first-write/replacement error handling through injectable filesystem/Windows-replacement seam; and frontend payload closure.
- Windows live acceptance: start with a pre-seeded valid snapshot in the resolved app-local-data test location, verify restored visible pairs plus Application restarted, verify the first new request contains only its new prompt, verify no automatic Codex connection/repository/tool restoration, complete a new pair and restart to observe it, then test invalid snapshot yields only the sanitized warning and no model replay. Cleanup must target only the dedicated test app-data directory.

## 27. Explicit non-goals

This task and proposed Task 112 do not change production behavior in Task 111, add SQLite, persist credentials or repository authority, persist generation counters as durable identities, reuse Codex threads, automatically connect/reconnect, add a browser/history manager, cloud sync, encryption implementation, an ADR, or changes to `AgentRuntime`, `AgentInput`, `rah-session`, Generic Tool Bridge, ToolRegistry, or provider configuration.
