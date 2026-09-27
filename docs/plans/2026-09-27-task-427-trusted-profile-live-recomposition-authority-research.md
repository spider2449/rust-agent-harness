# Task 427 — Trusted Profile Live Recomposition Authority Research

**Task type:** research, authority, and lifecycle design only
**Baseline:** `1e23fe76c9b349b7e4650e4f3d1c01eb9ce1ac0b`
**Verdict:** `NO FEATURE — DISCONNECT / CONNECT REMAINS THE CLOSED TRUSTED PROFILE RECOMPOSITION BOUNDARY`

## 1. Starting checkpoint and Task 426 context

The repository started at the required `HEAD == origin/master == 1e23fe76c9b349b7e4650e4f3d1c01eb9ce1ac0b`, subject `docs: define RAH v0.33 scope and authority roadmap`, with a clean worktree. Task 426 exact-head CI run `36287557042` was reported PASS for formatting, workspace check, workspace tests, and lint. Task 426 selected no v0.33 product capability and sent Trusted Profile live recomposition to this research task; that was a research pointer, not feature authorization.

This task changes no production behavior. The existing authority boundary is the host-selected Trusted Profile and its effective composition lifecycle. This report does not select live publication replacement.

## 2. Current Trusted Profile lifecycle

Desktop's `DesktopTrustedProfileSelection` stores the exact absolute source path privately, the profile ID, MCP and Process Plugin counts, expected tool count, and sanitized external tool descriptors. Its serialized presentation exposes selected/remembered state, profile ID, and counts; it does not serialize the source path. Selection calls `TrustedStaticProfile::load`, applies the Desktop provider-only rule (rejecting any first-party capability declarations), calculates descriptors, then drops the loaded static profile without launching providers.

The retained selection is configured intent, not the effective provider set. `DesktopProviderActivation::activate` calls `load_for_activation()` against the retained path on every Connect attempt, reapplies provider-only validation, derives permission levels and descriptors from host profile declarations, and invokes the effective composer. Provider metadata does not assign RAH permission. The resulting activation wrapper owns one `EffectiveProfileComposition`; current code associates that owner with the connection being established.

Relevant implementation: `crates/rah-desktop/src/trusted_profile_selection.rs`, `crates/rah-desktop/src/provider_composition.rs`, `crates/rah-tools/src/trusted_profile.rs`, and `crates/rah-profile-composition/src/lib.rs`.

## 3. Connect and Disconnect semantics

The frontend has a Connect Codex / Disconnect Codex control. The profile panel says Restore validates without spawning providers and Connect activates providers. Profile selection and restore are allowed only while chat is idle and connection state is `NotConnected` or `Error`; selecting a different profile while connected therefore requires Disconnect first. No profile-file watcher or automatic activation exists.

Connect captures repository, model, profile, connection, and commit-identity generations; builds the first-party registry; activates the selected profile; merges both into a fresh registry; computes the host allowed-permission set; creates the effective authority composition; and starts a new Codex app-server runtime with the registry and model/workspace configuration. It publishes only after generation/currentness checks pass. Duplicate names or a failed activation/merge fail closed; a rejected pending publication shuts down the new runtime and activation.

Disconnect requires `ChatState::Idle`, no active repository index effect, and no model-turn or prepared HostInvocation state. It withdraws the connected publication, revokes repository Commit context, shuts down the Codex runtime, then consumes and shuts down the provider activation. It does not clear the selected profile, selected repository, selected model, or conversation text. A successful reconnect uses the current selected profile source and creates a fresh Codex runtime. If repository/model context is unchanged, the stored conversation remains usable and is replayed as input to a subsequent new thread. A failed runtime shutdown leaves an error connection state; provider shutdown is attempted afterward.

Disconnect is therefore the existing safe provider retirement boundary. It is an explicit operation, and it already enables the desired profile reread without restarting the Desktop application.

## 4. Profile source loading and currentness

### Selection and preview

The exact source path is retained privately. No file identity, byte length, modification time, or content digest is retained. Static selection loads bytes and constructs sanitized counts/descriptors at that time, but those counts are not a byte-bound approval token. A profile with the same profile ID can change provider IDs, executables, tools, schemas, or permission assignments while retaining that ID.

### Activation reread

`TrustedStaticProfile::load` delegates source reading to `trusted_profile_source::read`. The reader requires an absolute path, checks path topology for links/reparse points and non-directory ancestors, opens the file, checks regular-file type and a 1 MiB limit, rechecks topology, reads bounded UTF-8, and parses/validates strict profile data. On Unix it compares opened device/inode to the current pathname before reading. On non-Unix targets, including Windows, `verify_opened_identity` is a no-op. The pre/open/post topology checks reduce exposure but do not prove race-free identity against concurrent replacement.

There is no captured hash or expected profile identity passed from selection into activation. Connect authorizes rereading the explicitly selected path under the current host profile semantics, not exact bytes previously displayed. A changed file is freshly interpreted. A missing/unreadable source, invalid schema, link/reparse source, or unsupported profile fails activation; provider launch does not proceed after a failed profile load. A change after bytes have been read does not change the already parsed candidate, but the current API does not bind human review to those bytes.

No byte-exact approval claim is made. If a future feature presents a change review, it would need to bind the confirmation to a candidate identity/content digest and verify that same candidate through activation, or state explicitly that confirmation authorizes the selected source path's current contents. The latter would still be path-level consent, not byte-level consent.

## 5. Provider composition ownership, publication, and cleanup

`EffectiveProfileComposition` owns a fresh `Arc<ToolRegistry>`, redacted effective inventory, MCP adapters, Process Plugin adapters, and optional host controls. `registry_handle()` clones the registry `Arc`. Its consuming `shutdown(self)` processes Process Plugin adapters in reverse order, then MCP adapters in reverse order. Desktop activation consumes that composition through `shutdown(self)`.

Composition builds privately. It starts MCP providers and then Process Plugin providers, requires each provider's exact configured discovery/schema and permission admission, registers immutable RAH proxy Tools into a fresh registry, and returns no composition until all complete. Duplicate public tool names fail closed. For a provider connect or duplicate-registration failure, already admitted adapters are explicitly shut down in reverse order before returning an error. This establishes no partial registry publication for those handled paths.

The cleanup guarantee is narrower than “every possible failed or cancelled candidate is reaped.” Some fallible resource/configuration steps in the provider loops use early `?` returns after earlier adapters may have started; those paths do not route through the explicit `fail` cleanup helper. Future-source mutation between static validation and composition makes those paths relevant to any candidate-startup contract. Cancellation of the composition future also has no explicit awaited cleanup transaction in this function.

Adapter shutdown methods return errors, but `EffectiveProfileComposition::shutdown` discards each result. Consequently Desktop cannot report whether every child actor completed termination/reaping. Adapter supervisors implement bounded shutdown/child-wait paths; an error or timeout is possible. MCP `Drop` sends a stop control without awaiting its actor. Process Plugin's public shutdown documents graceful request followed by termination/reaping, but a poisoned owner or bounded actor wait can still return an error, which the composition currently hides. There is no proof here that a future old-generation registry has no retained proxy references at the instant retirement begins.

### Publication ordering alternatives

**Build new before withdrawing old** keeps A published while B starts. A and B processes can overlap and can both retain external side-effect capability. B's private registry is not model-visible until handoff, but a handoff would have to retire every A dispatch path before B becomes usable. Existing runtime/thread references and provider-call tasks can retain A. The current composition owner has no publication swap protocol, quiescence count, or old-owner retirement result. This contract is not closed.

**Withdraw old before building new** creates an unavailable interval, retires A, then builds B. If B fails, the session is disconnected. This avoids overlap only if A calls and proxies are quiescent and shutdown is verified before B starts. Existing `Disconnect` already provides this sequence while also replacing the Codex runtime/thread. This is not a materially different product operation.

No atomicity beyond existing Connect's pending-publication transaction is claimed. That transaction publishes the first complete composition from a disconnected state; it does not implement replacement of an already connected composition.

## 6. Pending calls, cancellation, and old-provider retirement

The Codex bridge snapshots a `ToolDefinition` and private alias map for each runtime start/thread, then admits a dynamic call only when thread ID, turn ID, alias, and snapshot match. It authorizes against the bridge's captured `Arc<ToolRegistry>` and allowed permissions, clones that registry into the asynchronous tool-dispatch task, and delivers completion back to the original thread/turn. The lifecycle can include queued request, admitted provider call, request sent, side effect possible, pending response, runtime delivery, cancellation, timeout/disconnect, and terminal result.

Desktop does not allow its Disconnect handler while its chat state is running or cancel-requested. This is a meaningful barrier for ordinary lifecycle use, but adapter call timeout/disconnect can leave an external effect uncertain even after the RAH turn reaches a terminal error. MCP cancellation is a protocol notification; Process Plugin cancellation depends on the peer's advertised support. Neither cancellation nor a timeout proves that the external side effect did not occur. No call may be automatically replayed against B.

During true live replacement, an A call may hold an A registry/proxy while publication switches to B. A late A result remains routed to A's original Codex thread and turn, and without an explicit generation gate could arrive after B publication. Aborting the Rust task is not evidence of external rollback. Preventing this requires a quiescence/cancellation/currentness contract across runtime, bridge, adapter, and provider layers; none is implemented for profile replacement.

Required default for any future design: no in-flight or uncertain call crosses generations, no automatic retry/replay, and no claim of rollback. If the only provable gate is fully idle and runtime-disconnected, that reduces to the current Disconnect → Connect path.

## 7. Codex Tool snapshot, session, and conversation semantics

`rah-runtime-codex::CodexRuntime::start` calls `snapshot_tools` for the runtime's captured bridge registry and includes the resulting dynamic definitions in `thread/start`. It stores per-SessionId `SessionRecord` data containing the Codex thread ID, active turn ID, and bridge tool/alias snapshots. Dynamic requests route through those stored definitions and the runtime's captured registry. There is no API to replace that runtime registry or update definitions in an existing Codex thread.

Desktop invokes runtime start for a chat turn. Codex thread/turn identity and RAH `SessionId` are per started turn; the Desktop conversation is separately held as display/history text and replayed into a new request. Replacing a registry under an existing runtime/thread would not update an already started thread's advertised definitions or snapshot. Removed tools could remain requestable under stale definitions; changed same-name schema/provider tools could disagree with the old definition and new proxy; new tools would not be in the existing snapshot. A new thread alone might refresh the snapshot only if the runtime itself held the new registry, but no safe registry mutation API exists. A fresh runtime connection is the proven path.

Conversation text is distinct from Codex-native thread identity, RAH SessionId, and ToolRegistry authority. Repository/model generations participate in Desktop conversation context identity; Trusted Profile generation does not. Therefore a profile-only Disconnect → Connect does not clear the displayed/replayed conversation. The new Codex runtime starts a new thread on the next turn from that conversation input. Previous provider results remain historical conversation text and do not grant the replacement profile authority. Profile-only thread replacement should be described as a new model execution context, not continuity of the old provider authority.

There is no source evidence that a profile-only change invalidates persisted resume lineage. A future operation that changes only profile composition would need an explicit transcript policy; the existing reconnect workflow currently preserves the host conversation and its independent repo/model context rules.

## 8. Generations and currentness matrix

| State/generation | Current role and checked objects | Live replacement implication |
| --- | --- | --- |
| Repository generation | Changes with repository selection/membership/active identity transitions. Captured by Connect publication and turn-start checks; repository tools and HostInvocation use repository identity/currentness. | Candidate must freeze the one active repository, generation, and fingerprint and rebuild a fresh merged registry from that member. Concurrent switch/close/admission must reject publication. No union registry is allowed. |
| Model generation | Advances when selected model/provider configuration changes. Captured by connection and checked before turn/current publication; affects runtime's immutable model config and conversation identity. | Must remain unchanged. Profile replacement cannot silently bundle a model change; changed model requires the existing broader reconnect transition. |
| Profile generation | Advances on host profile selection/clear. Captured in Connected and checked for publication, authority snapshots, and turn admission; status reports `reconnect required` when stale. | Existing generation tracks selection intent, not file bytes. A separate candidate/source identity would be needed to bind review and publication to contents. |
| Connection generation | Advances at Connect start and is captured/checked at publication and currentness boundaries. | Each fresh connected publication must supersede older preparation and reject stale confirmation/transition work. Reusing a generation for a different registry would be ambiguous. |
| Identity generation | Commit identity generation is captured with connection publication and constrains the Commit capability; currentness publication checks compare it. | Recomposition must preserve or freshly rebuild the Commit-bound first-party composition under current identity; it cannot retain an old Commit tool accidentally. |
| Chat/runtime turn generation | Desktop chat generation and terminal ownership arbitrate one turn's completion; Codex uses thread ID and turn ID; HostInvocation records model-turn/currentness state. | Replacement must require terminal/idle state and reject late old-turn/provider results. Cancellation requested is not terminal. |
| HostInvocation currentness | Prepared tickets capture repository/model/profile/connection context, registry identity, definitions, permissions, and relevant preparer identity; confirm checks for staleness. | Any provider registry/permission change must invalidate outstanding preparations before publication; current lifecycle's selection/Disconnect gates do not create a swap protocol. |
| Provider/composition generation | No standalone generation currently exists. Owner identity is represented indirectly by connection publication and registry `Arc` identity. | No new generation is recommended because no live feature is selected. A future replacement contract would need to decide whether connection generation plus registry identity is sufficient or whether explicit composition generation is required. |
| Repository fingerprint | Captured in connected publication as repository context evidence; not itself the only generation check. | Freeze it with repository generation and rebuild from the same host-selected member; a fingerprint is not a substitute for lifecycle exclusion. |

Stale scenarios must fail closed: a candidate prepared under A cannot publish after profile generation moves to B; an A turn result cannot authorize or be delivered as a B result; and simultaneous repository/model/profile/connection/identity changes must cause candidate publication rejection and candidate cleanup. Existing activation checks already reject stale Connect publication; no live candidate handoff exists.

## 9. Repository and model interaction

The connected registry is not just external providers. Connect builds first-party tools for the selected repository and merges the external profile registry into a fresh registry. Recomposition must therefore rebuild the entire merged registry, not swap only an external subregistry. It must retain zero or one active repository, preserve its current member identity/generation and canonical context, and reject a concurrent repository transition. Provider configuration cannot select a repository, and a union registry is out of scope.

The runtime captures model configuration and canonical workspace context at creation. A safe replacement would hold model generation/config unchanged and create a fresh runtime under that exact state. Combining profile, model, repository, and runtime identity changes into one operation would make authority attribution and conversation semantics harder to review and is not warranted by the evidenced user problem.

## 10. Tool and permission delta review

The host derives external permission levels from the selected profile and merges them with repository-dependent first-party permissions. Provider-authored metadata cannot assign or escalate permission. A profile delta can nevertheless change the effective host-selected authority:

| Delta | Effect |
| --- | --- |
| Same authority shape | Same tool names, provider identities, schemas, and permission levels; provider executable/configuration may still differ and can change behavior/effects. |
| Expansion | New tool, stronger permission, provider, or schema capability becomes model-visible. Requires a fresh explicit host decision; mere file-change detection cannot authorize it. |
| Reduction | Tool or permission removed. A fresh thread is still needed so stale definitions are not retained. |
| Replacement | Same public name with changed provider identity, executable, schema, or permission. Name equality does not make this equivalent authority. |

An honest sanitized change review, if a future disconnected UX needs one, should cover profile identity, provider-kind/count changes, tool-name additions/removals, schema-change indication, and permission-level changes. It may say that provider configuration changed without disclosing raw executable paths, environment values, credentials, private metadata, or host paths. Same-name schema/provider replacement must not be shown as “unchanged.”

The current UI does not show a file-change delta; it presents the last selected static metadata. Explicit Connect freshly rereads the selected path. Detecting a changed file does not itself authorize activation.

## 11. Human authorization and terminology

Terminology for this report:

* **Profile reread:** read and validate current bytes from the selected source; no provider effect.
* **Provider recomposition:** privately build a fresh effective provider composition; may start local provider processes.
* **Publication replacement:** replace the active merged ToolRegistry, permissions, and composition while the Desktop remains connected.
* **Runtime reconnection:** stop/replace the Codex app-server runtime under a current model/workspace configuration.
* **Thread replacement:** create a new Codex `thread/start` whose dynamic Tool snapshot reflects its captured registry.

A standalone profile reread is not sufficient to change connected authority. True connected composition replacement would require at least publication replacement and, based on current Codex behavior, a fresh runtime and thread if tool membership/schema changes. If it also requires idle + disconnected runtime, it is ordinary Connect behavior.

Current host authorization is explicit profile selection/restore while disconnected, followed by explicit Connect. Choosing another profile is not allowed while connected. File modification alone has no effect. A dedicated live “Recompose” control would be a new host action, but a frontend click is intent only; the backend would still need to bind exact currentness, validate the candidate, and publish through an owned transition.

## 12. Failure and cancellation model

| Candidate result | Meaning under a hypothetical live design |
| --- | --- |
| Known no effect | Candidate never published and every started candidate child is confirmed stopped/reaped; old publication remains authoritative. Current composition API does not prove this for all early-error/cancellation paths. |
| Replaced verified | B is current, A is no longer dispatchable, A calls/proxies are quiescent, and old child retirement result is verified. Current types do not establish this. |
| Disconnected safe | A was withdrawn and retired; B failed; runtime is disconnected and no current provider publication exists. Current Disconnect → Connect already yields this class of state on activation failure. |
| Uncertain | External provider side effects may have occurred, or child termination/reaping cannot be established after an error/timeout/cancel. Do not retry or claim rollback. |

Profile load and static validation are synchronous and return errors. Provider startup/discovery and runtime startup are async and may fail. Existing connection publication can reject stale work and shut down the unpublished activation. Candidate cancellation has no documented transactional cancellation contract: dropping the composition future may drop owner handles without awaiting all actors, and external effects are not rolled back. Cancellation during old-provider retirement cannot restore a provider that may have partially shut down. No retry or compensation is implied.

The current composition consumes adapters in reverse provider order by kind and ignores adapter shutdown results. Thus old retirement failure visibility and orphan exclusion are not closed for live replacement. Ordinary Disconnect remains a serialized lifecycle transition and is the existing behavior to certify; this research adds no stronger claim.

## 13. MCP and Process Plugin lifecycle differences

### MCP

The supported profile path is local stdio only. It uses a configured native executable, cleared environment and isolated working directory, bounded startup/request/result behavior, pinned MCP protocol, tools/list validation, expected tool/schema admission, cancellation notification, and an owned supervisor. Calls can have external server side effects. Timed-out or cancelled calls remain effect-uncertain. MCP shutdown requests protocol shutdown, signals stop, and awaits the actor under a bound; shutdown errors are returned at adapter level but swallowed by `EffectiveProfileComposition`.

Starting B before stopping A can run two local MCP server processes concurrently with their configured effects/resources. No current contract says those processes are idempotent or harmless in overlap. Network MCP is unsupported and excluded: no HTTP transport, URL, OAuth, remote credential, DNS, TLS, proxy, or endpoint selection enters this candidate.

### Process Plugin

The Process Plugin path is also local stdio but has its own protocol, fixed configured process identity and version, isolated cwd, environment allowlist, exact expected tool/schema handshake, request capacity and timeout, execution IDs, and optional peer-supported cancellation. Its proxy Tools are cloneable `Arc<dyn Tool>` values backed by a provider client. A timed-out plugin call can be externally uncertain; cancellation is meaningful only when the handshake advertised support. Shutdown requests graceful termination and awaits actor/process reaping under a bound, but adapter errors are discarded by the composition owner.

Process Plugin and MCP share high-level composition ownership but do not have identical protocol, timeout, cancellation, or shutdown guarantees. No process-generation reuse, cross-generation proxy safety, or overlap contract is established for either kind.

## 14. Security, privacy, persistence, and repository/Git impact

This task adds no Tool, permission, executable, provider kind, schema, transport, network route, repository selector, filesystem/Git operation, persistence field, or executable authority restoration. HostExplicit remains separate and its allowlist is unchanged. The provider-selected profile remains a host-owned configuration source; external permission assignment is host-derived.

No source path, executable path, environment value, credential, provider-private metadata, or raw configuration is needed in frontend presentation. A future sanitized delta can report symbolic profile/provider/tool/permission facts only if a separately scoped UX task establishes that value.

Restart continues to restore no active provider composition, provider PID, ToolRegistry, runtime, repository member authority, or executable repository authority. A remembered profile path is descriptive host preference only. No profile/process state should be persisted as authority.

Repository and Git authority remain as selected by the existing host repository lifecycle. No provider may choose or union repositories. No network authority or unsupported platform claim is added.

## 15. Product-benefit evidence

| Question | Evidence classification | Finding |
| --- | --- | --- |
| Must users restart the application to change profiles? | Source-proven and documentation-proven | No. The UI has a Disconnect control and then permits selecting/restoring a profile and Connect. Connect rereads and activates it. |
| Does Disconnect → Connect preserve repository/model configuration? | Source-proven | Disconnect tears down runtime/provider publication without clearing selected repository or model state. Reconnect captures their current configuration. |
| Is displayed conversation preserved? | Source-proven | Disconnect does not clear Desktop conversation state. Profile generation is not part of its repository/model context identity. Same repo/model reconnect replays the stored history into a new Codex turn/thread. |
| Is the old Codex thread preserved? | Source-proven | No. A new runtime starts new Codex threads per Desktop turn; there is no registry replacement API for an existing runtime/thread. |
| Are provider changes frequent or costly to apply through reconnect? | Not established | No issue, telemetry, test, or documentation evidence measures frequency, lost work, or significant user impact. |
| Is current reload UX explicit? | Source-proven | Yes: selection/restore is disconnected-only and provider activation is explicitly attached to Connect. UI explains that Restore does not spawn providers and Connect activates them. |
| Would live recomposition save a user step? | Inferred | A successful true live operation might avoid one explicit disconnect/reconnect interaction, but safe thread/runtime replacement and permission review substantially reproduce the existing lifecycle. |

Task 426 and repository docs identify operational convenience as a plausible benefit, but provide no observed frequency or material harm. The demonstrated benefit beyond fewer explicit reconnect steps is absent.

## 16. Direct comparison

| Dimension | Existing Disconnect → Connect | Candidate Live Recompose |
| --- | --- | --- |
| User steps | Explicit disconnect, select/restore if needed, explicit connect | Could remove one visible disconnect step only if hidden teardown/reconnect is safe; otherwise same steps |
| Conversation impact | Host transcript retained for unchanged repo/model; next turn uses a fresh Codex thread | Must choose transcript policy; safe model context still needs a new thread/runtime |
| Codex thread impact | Fresh runtime; future turn snapshots fresh registry | Existing thread snapshot cannot be updated; fresh thread/runtime required for changed tools |
| Provider overlap | Old runtime shuts down before provider activation is consumed; no candidate startup overlaps old composition | Build-first overlaps external processes/effects; withdraw-first is the existing disconnected path |
| Pending-call handling | Disconnect rejects running/cancel-requested Desktop chat; timeouts can still be uncertain and are not replayed | Needs cross-layer quiescence and late-result rejection; not implemented |
| Tool snapshot freshness | New runtime snapshots current merged registry on new thread start | Stale existing snapshot/proxy remains possible without fresh runtime/thread |
| Permission review | Connect applies host-selected profile declarations and host permission computation | Must compare provider/tool/schema/permission delta and bind confirmation to exact candidate |
| Failure state | Activation failure leaves no new connected composition; explicit disconnected/error status | Build-first can preserve A but cleanup/handoff are open; withdraw-first may only leave disconnected |
| Child-process ownership | One activation owner per connection; shutdown attempted on Disconnect | Two generations, retained Arcs/tasks, and hidden shutdown errors make retirement unclear |
| Currentness complexity | Existing generation-gated Connect publication | Adds source-content identity, composition handoff, runtime/thread, call, repository/model race gates |
| Deterministic test burden | Existing lifecycle has bounded transition tests and composition tests | Requires extensive barriers, fault injection, late response and cancellation/reaping proofs |
| Windows live-cert burden | Existing Connect/disconnect provider path can be certified as a sequence | Must certify real A/B overlap or withdrawal, stale thread rejection, reaping, and no old reachability |
| New authority | No new category; explicit host-selected profile under ADR 0011 | Connected concurrent replacement changes provider/tool/permission lifecycle; existing ADR does not define it |
| Product benefit | One explicit lifecycle already rereads the selected profile | No demonstrated material benefit beyond fewer visible steps |

Live recomposition is not materially better on current evidence. Option B, “recompose while disconnected,” merely renames or shortens the already explicit Connect path and should not be added as a separate capability.

## 17. Deterministic test direction if reconsidered

No implementation or tests are authorized by this research. If a later task reopens the candidate, deterministic tests should use barriers and fake provider actors, never sleeps, and must prove at least:

* unchanged profile, source replaced between review and activation, invalid/missing/link source, and same ID with changed declarations;
* exact provider identity/tool/schema admission; startup failure and partial multi-provider startup failure; duplicate name; schema change; permission expansion/reduction; provider/tool removal;
* cancellation at source read, provider startup, discovery, merge, publication, and retirement; cleanup outcome and shutdown failure visibility;
* no start while turn/call is queued, admitted, sent, side-effect-possible, response-pending, delivery-pending, cancel-requested, timed out, or uncertain;
* late A result, stale name/schema/provider, no automatic replay, and no old registry/proxy reachability after retirement;
* repository, model, profile, connection, identity, HostInvocation, and source-content currentness races; stale confirmation rejection;
* restart, Disconnect during candidate work, and application shutdown during candidate work;
* old-first versus new-first behavior with a clear known-no-effect, replaced-verified, disconnected-safe, or uncertain result.

Tests should prove the core transaction and provider ownership, not treat cancellation as rollback. Independent review would need to verify the test path exercises actual Desktop publication/currentness code rather than helper-only predicates.

## 18. Windows live certification direction and platform nonclaims

If separately authorized later, Windows host-driven backend certification should use a selected Trusted Profile file and real MCP and Process Plugin fixture executables through production constructors. It should record start, exact publication, fresh thread/tool visibility, explicit transition, old provider retirement and child reaping, no duplicate child, shutdown, and whether any old proxy/thread can still dispatch. It should distinguish successful backend evidence from GUI automation; no GUI automation is claimed here.

Portable deterministic library tests do not prove Windows process identity/reparse/executable validation or live child cleanup. Windows backend evidence does not establish Linux/macOS behavior. No Linux/macOS live provider startup, shutdown, environment, cancellation, or reaping claim is made. Cross-platform parity requires separate platform evidence.

## 19. ADR conclusion and authority classification

**No ADR is needed for the selected no-feature outcome.** An explicit disconnected Connect that rereads an already selected profile fits the existing host-selected Trusted Profile, provider-composition, and connection boundaries.

True connected publication replacement is different: ADR 0011 states profile configuration is immutable for an effective-composition lifetime and defers automatic/hot reload; ADR 0006 snapshots Tools per Codex thread and keeps the runtime bridge's registry authority explicit. A concurrent A/B provider regime, dynamic permission/tool replacement, and old-owner retirement would alter those lifecycle assumptions. If a future proposal retains that true live behavior, authority/lifecycle research must decide whether a narrowly bounded superseding/new ADR is required before any implementation. This task authors none.

Authority classification for the selected outcome is `NONE` (no new operation or authority). A hypothetical explicit replacement that first disconnects and then Connects remains within the broader existing Trusted Profile authority semantics. A true connected handoff cannot be called a narrow extension merely because it invokes the same parser/composer: it changes when old and new external provider capabilities coexist and how already running threads/calls are retired. This is a reason to defer it, not a new authority granted here.

## 20. Final outcome and recommended next task

`NO FEATURE — DISCONNECT / CONNECT REMAINS THE CLOSED TRUSTED PROFILE RECOMPOSITION BOUNDARY`

The existing flow already rereads the selected profile, creates a fresh provider composition and registry, starts a fresh Codex runtime/thread context, and preserves repository/model settings and host conversation when their context is unchanged. No repository evidence demonstrates enough user pain to justify live transition complexity. A connected swap cannot safely update the existing Codex thread's Tool snapshot, has unresolved in-flight/late-call and old-provider retirement behavior, and would need stronger source-content/currentness and cleanup guarantees. A disconnected “recompose” action would duplicate Connect.

**Recommended next task:** return to v0.33 scope selection or research a different candidate with demonstrated user benefit. Do not start Task 428 for live recomposition on this evidence.

## 21. Scope and validation record

This is a research artifact only. No Rust, test source, manifest, lockfile, dependency, feature, workspace membership, version, README, changelog, release gate, ADR, profile schema, Tauri permission, tag, or GitHub Release is changed. No test suite is required or run.

Requested validation:

* `git diff --check`: PASS.
* `cargo metadata --no-deps --format-version 1`: PASS; 13 packages, 13 workspace members, all `0.32.0`, edition `2024`.
* Staged `git diff --name-only`: this artifact only.
* Commit/push and exact-head verification: to be recorded in task closeout.
