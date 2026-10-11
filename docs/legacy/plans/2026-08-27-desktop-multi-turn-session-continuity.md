# Desktop multi-turn conversation continuity and context boundaries

Status: Task 109 research/design complete. No production behavior is changed by this document.

## Decision

Task 110 should add **bounded, process-local, host-owned provider-neutral message replay above `AgentRuntime`**. The Desktop host will retain only completed `User`/final `Assistant` pairs for the active conversation epoch, prepend them to the next `AgentInput.messages`, and commit the new pair only after `AgentEvent::Completed`.

It should not reuse Codex-native threads, add a generic continuation API, or persist sessions to disk. This is the smallest implementation that makes the model context match the transcript segment while preserving RAH's provider-neutral runtime boundary.

## Current observed behavior and semantic problem

The frontend keeps visible chat messages, but visibility is not model context:

1. `rah-desktop::run_chat` constructs `AgentInput.messages` with only the new `User` prompt.
2. `CodexRuntime::start` rejects an empty input, creates a fresh Codex `thread/start`, then starts one `turn/start` using a translation of every supplied RAH message.
3. `AgentEvent::Completed` already carries the final provider-neutral `AgentOutput.message`.

Consequently, a visible sequence of user, assistant, user messages does not imply that the second runtime request contains the first pair. The first implementation must correct that false implication without claiming a durable or provider-native continuation.

The existing `AgentRuntime::resume(SessionId)` does not solve this product need. It issues `thread/resume` for a private Codex mapping and returns a passive stream; Codex still needs a subsequent `turn/start` to submit new input. It cannot safely invent that input because the RAH API supplies none.

## Existing foundations

`rah-protocol` already provides the required neutral request/output path:

```text
completed User + Assistant pairs, then current User
    -> AgentInput.messages
    -> AgentRuntime::start
    -> AgentEvent::Completed { AgentOutput { message } }
```

`Message` has neutral `System`, `User`, `Assistant`, and `Tool` roles. No provider request type is needed. The Codex adapter currently translates each message into a role-prefixed input item, so replay is observable at the adapter boundary even though each operation starts a new Codex thread.

`rah-session` already provides `AgentContext { messages, tool_results, metadata }`, `Session`, `SessionStatus`, `SessionStore`, and deterministic process-local `MemorySessionStore`. It does **not** yet establish that one `SessionId` means a long-lived Desktop conversation: runtime events currently use a fresh `SessionId` per `start`, while `SessionStatus` describes the operation lifecycle (`Running`, `Completed`, `Cancelled`, and so on). Task 110 should reuse the neutral `Message`/`AgentContext` concepts, but should not force `MemorySessionStore` into the Desktop until a small, explicit host-conversation identity/lifecycle design removes that ambiguity.

## Terms that must remain distinct

| Term | Meaning | Must not be mistaken for |
| --- | --- | --- |
| Visible transcript | UI records, including pending, failed, cancelled, activity, and previous epochs | model input |
| Provider-neutral conversation context | committed `User` + final `Assistant` messages replayed in the active epoch | authority or provider state |
| Codex app-server thread | adapter-private provider-native thread and turn state | a Desktop conversation or public RAH identity |
| Authority/tool-registry snapshot | host-created selected repository, trusted profile, permissions, and registered-tool snapshot for an operation | a fact granted by transcript text |
| Durable persisted session | future on-disk recovery record with schema and privacy rules | process-local replay state |

Three identifiers follow from those terms: a private, provider-neutral Desktop conversation ID for host state; the operation-scoped RAH `SessionId` emitted by `AgentRuntime::start`; and the private Codex thread ID. Task 110 should not expose any of them to the frontend. Reusing the operation `SessionId` for a long-lived Desktop conversation would conflate established runtime semantics with a different lifecycle. A later session-design task can decide whether a dedicated public conversation identifier is warranted.

## Option analysis and decision matrix

| Option | Product continuity | Provider neutrality / portability | Architecture and authority clarity | Complexity | Token/context cost | Reconnect, provider, repository behavior | Failure/cancel semantics | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1. Full provider-neutral message replay | Real continuity for committed transcript segment | High; every runtime receives `AgentInput.messages` | State remains host-owned above runtime; tool registry still snapshots per operation | Low | Linear bounded replay cost | Same-context reconnect can retain context; explicit epoch on model/repository change | Simple atomic host commit | **Adopt for Task 110** |
| 2. Codex-native thread reuse | Efficient native continuity | Low; behavior is Codex-specific | Couples Desktop identity, private thread state, bridge lifetime, and authority snapshot decisions | High | Potentially lower repeated prompt cost | Must define reconnect recovery, changed model/repository thread disposition, and non-Codex fallback | Provider state may contain uncertain partial turns | Defer |
| 3. New generic continuation API | Potentially useful later | Only if all runtimes can define equivalent semantics | Changes stable extension point without a capability current input cannot express | Medium/high | Provider-dependent | Requires normative behavior for every implementation | Requires new cross-runtime terminal/continuation rules | Do not add |
| 4. Durable SessionStore first | Survives restart, but does not itself fix current-turn continuity | Potentially high, once designed | Must bind durable history to fresh authority validation | High | Same replay cost plus storage limits | Requires recovery, migration, stale snapshot, and privacy policy | Must represent interrupted/uncertain operations | Defer |

### Why not native Codex reuse now

Codex thread reuse would require a new way to supply a fresh `turn/start` for an existing private mapping. It would also make the Generic Tool Bridge snapshot lifetime span multiple Desktop turns. A repository/profile/tool-registry change would then require either invalidating the thread, proving its tool set is unchanged, or allowing old context to coexist with new authority. Cancellation and disconnect would additionally leave provider-native state whose exact terminal status must be reconciled before reuse. None of those concerns exists for a new bounded request with a fresh host-built registry snapshot.

The future rule remains: a persisted Codex thread, if later used, never restores authority by itself. Current trusted host state must recompute repository, profile, permissions, and tool inventory before any operation.

### Why no continuation API

The existing input already represents the product capability: a runtime is given the complete provider-neutral conversation messages for an operation. A `continue_session` API would either duplicate this explicit replay capability or introduce provider-native hidden-state semantics that other runtimes cannot reproduce. Add one only after a separately specified cross-runtime capability cannot safely be represented by `AgentInput.messages`.

### Why no durable store first

Disk persistence is separable from continuity and would magnify an unresolved semantic mismatch. A future durable task must separately specify storage location, versioned schema, atomic replace/write, corruption and startup recovery, size limits, deletion/privacy, repository and provider metadata redaction, migration, failed/cancelled records, and stale authority/context snapshots. It must not serialize paths, credentials, provider-native IDs, or old authority as current authority by accident.

## Conversation epoch and execution-context boundary

Introduce a host-only **ConversationEpoch** (also acceptable as `ConversationContextGeneration`) in Task 110. An epoch owns one replayable provider-neutral message list and an execution-context identity captured when that epoch becomes active.

The identity must at least include the existing repository generation and model generation. It should reserve a future trusted-profile/tool-registry generation, but Task 110 must not invent profile behavior before it exists. Generation numbers are internal only; they never appear in IPC or UI.

The rules are:

1. Disconnect alone does not create an epoch. If the desired repository and model generations are unchanged, a reconnect may continue the same process-local replay context.
2. A repository generation change creates a new epoch before the next operation. Old transcript remains readable but is not replayed. This prevents repository-A facts from being injected into a request whose fresh tools target repository B.
3. An effective model configuration generation change creates a new epoch before the next operation. This includes `llama.cpp -> reconnect required -> Inherit -> disconnect -> reconnect` when the effective model selection changed. A reconnect without a changed effective selection does not create an epoch.
4. A future trusted-profile/tool-registry generation change should use the same new-epoch rule, because it changes the execution context presented to the model. This is not implementation scope for Task 110.

These are correctness and UX context-isolation boundaries, **not** authorization boundaries. A model/provider change does not grant or revoke tool permission by itself. Separately, the host continues to require reconnect for the existing authority/model snapshots and creates the registry from host-selected state.

The minimal UI representation is a non-sensitive separator before the new segment:

```text
──────── New conversation context ────────
Repository changed
```

or:

```text
──────── New conversation context ────────
Model configuration changed
```

Do not show generation counts, `SessionId`, Codex thread IDs, absolute repository paths, or provider configuration details. Old messages remain visible above the separator, but they are explicitly outside the new replayable context.

## Atomic turn-commit contract

The Desktop host prepares an immutable candidate request from the active epoch's committed pairs plus the new prompt. Presentation state is separate from that candidate.

| Outcome | Visible transcript | Replayable active-epoch context |
| --- | --- | --- |
| `Completed` | Keep submitted user message and final assistant output | Atomically append exactly `User(prompt)`, then `Assistant(output.message)` |
| Runtime/model/tool failure | May keep submitted user message and a failure indicator | Append neither |
| Cancellation | May keep submitted user message, streamed partial text, and cancelled indicator | Append neither; never use deltas as an assistant message |
| Connection failure before start | Show normal command failure if desired | Append neither |
| Stream ends without a terminal event | Show runtime failure | Append neither |

`Completed` is the only commit point. Before it, prompt, model deltas, tool activity, and any partial provider state are presentation/runtime state only. This gives deterministic all-or-nothing replay and prevents uncertain external effects or partial model text from becoming a later instruction. It also means a completed turn's tool activity is not replayed merely because the final narrative mentions it.

## Tool-result handling

For Task 110, replay only successful user/final-assistant messages. Do not convert `AgentEvent::ToolFinished` outputs into `MessageRole::Tool` and do not replay `AgentContext.tool_results`.

Current `AgentInput.messages` has no provider-neutral record that associates a tool result with a particular tool call, its arguments, its call ID, or the assistant decision that requested it. A standalone tool-role message can therefore be semantically malformed for another runtime and could mislead a later model about state changed under an earlier repository or authority snapshot. Detailed tool results remain activity/audit evidence for their operation only. A future protocol proposal, if needed, must introduce structured conversation tool-call/result records as one coherent contract; it is not part of Task 110.

## Deterministic context bounds

Replay must be bounded without provider tokenizers. Use deterministic UTF-8 byte and message-count accounting over the committed replay history, with complete pairs as the only removable unit in future designs.

Recommended initial limits:

- at most **8 replay messages** (four completed `User`/`Assistant` pairs);
- at most **32 KiB** aggregate UTF-8 content in those replay messages;
- preserve the existing **32 KiB** submitted-prompt limit; no tokenizer or provider probing is added;
- count exact `String::len()` bytes, not characters or estimated tokens.

Before a new operation, construct the actual request candidate and reject it with the clear, sanitized error **"conversation context limit reached; start a new conversation context"** if its committed history would exceed either replay limit. On completion, append the entire pair atomically; if that makes the committed epoch reach or exceed the limit, it remains a valid completed record but the next operation is rejected until the user starts a new context. This does not silently discard a successful response or commit a malformed half-pair.

Fail-closed is the first-version recommendation. Oldest-pair truncation preserves syntactic pairs but silently removes potentially material repository constraints. Arbitrary message truncation can create an assistant-without-user history. Summarization changes model content, requires a new trust and failure model, and is out of scope. A later product decision may explicitly choose oldest-completed-pair eviction with a visible warning, but it must never remove only one member of a pair.

The byte cap is not a token guarantee. Codex already has a large request envelope; prior evidence observed approximately 10K provider-only initial tokens and approximately 18,411 Desktop request tokens, with llama.cpp `16,384` context insufficient even before meaningful history. Replay increases prompt pressure. The host must keep its history bounds independent of provider tokenizers, and llama.cpp operators remain responsible for selecting a sufficient host context; RAH must not choose `--ctx-size`, hardcode `262144`, or probe a provider in Task 110.

## Security and authority invariants

Conversation replay is model input only. It is never authorization, proof of repository identity, proof of a completed tool effect, or a replacement for a fresh tool-registry snapshot.

- The host owns repository/model/profile changes and verifies its active connection snapshots independently of transcript content.
- The Generic Tool Bridge remains generic and unchanged; `ToolRegistry` remains the sole tool extension boundary.
- Every model tool request still follows parsing, registry lookup, policy/permission, and sandbox/executor rules.
- No shell or process authority is added.
- Failed, cancelled, disconnected, or otherwise uncertain operations are not replayed, and no external effect is automatically replayed after reconnect.

## Recommended Task 110 scope

1. Add process-local Desktop host conversation state with a private conversation ID, active epoch identity, and committed neutral messages; do not expose it through IPC.
2. Build each `AgentRequest` from bounded committed active-epoch history plus the validated current user prompt.
3. Commit the submitted user and `Completed` final assistant message atomically; retain failed/cancelled UI records without adding them to replay history.
4. Start a new epoch, preserve visible old transcript, and emit only safe separator labels when repository or effective model generations change.
5. Add deterministic tests for two-turn request construction, atomic terminal handling, same-context reconnect retention, context-change separation, bounds, and no tool-output replay.
6. Keep the existing fresh `CodexRuntime::start` behavior, current registry creation, and runtime public APIs unchanged.

## Explicit non-goals

- Rust production behavior in Task 109;
- disk/JSON/SQLite persistence, migrations, or startup recovery;
- Codex thread reuse, provider-native hidden continuity, or Codex thread ID exposure;
- changes to `AgentRuntime`, `AgentInput`, `ToolRegistry`, Generic Tool Bridge, provider configuration, or repository authority;
- tool-result replay or structured tool conversation protocol work;
- summarization, tokenizer libraries, provider probing, or llama.cpp context-size selection;
- a new ADR. Existing boundaries are sufficient for the recommended host-level change.
