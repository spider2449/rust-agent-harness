# Task 206 — External Effective Authority UX Hardening

Status: implementation complete pending the requested commit and exact-head CI.

## Scope

Task 206 closes the v0.16 Desktop Effective Authority presentation gap exposed by Task 205. Desktop now presents admitted local MCP and Process Plugin Tools using host-owned classifications and applies conservative repository workflow handling after an external Tool may have produced effects.

The task keeps the Task 201 and Task 202 contracts: no new authority class, no new ADR, provider-only Trusted Profile selection, explicit Connect-time composition, atomic publication, and provider ownership through the usable registry lifetime.

## False-Stale root cause

Task 205 merged external provider Tools into the final Desktop ToolRegistry, but effective_authority::compose() recognized only hardcoded first-party Tool names. Unknown names were filtered out. The snapshot then compared the number of classified Tools with the final registry definition count, so a valid external-provider connection could be reported as stale solely because its MCP or Process Plugin Tools had no authority metadata.

The fix classifies every final registry definition. First-party definitions use their existing closed host table. External definitions require an exact host-owned descriptor match for public name, source kind, and permission. Unknown or mismatched definitions fail closed; they are never silently dropped.

## Configured, Effective, Advertised, and Current

Configured state is the explicit in-memory Trusted Profile selection and its bounded provider/Tool intents. Selection remains static and non-spawning.

Effective state is the fresh final Desktop registry after successful explicit Connect-time provider admission. An external Tool is effective only after its provider composition and exact registry reconciliation succeed. Activation failure publishes no partial external inventory; selected intents can remain visible as not_effective with a bounded provider reason.

Advertised state is whether the current connection has published that effective inventory to the runtime. Public RAH Tool names remain the displayed identity; private Generic Codex Tool Bridge aliases are never exposed.

connected_current means the published inventory matches the captured Desktop host context and complete classification checks. It does not mean that a provider passed a fresh health check, will answer the next request, is bounded to the selected repository, or is protected by an RAH OS sandbox.

## External classification

Descriptors are created from the host-selected validated Trusted Profile and the admitted effective composition. MCP Tools use sourceKind = mcp; Process Plugin Tools use sourceKind = process_plugin. Both use effectClass = external, authorityCategory = external, the exact host-configured RAH PermissionLevel, and repositoryBound = false.

Provider descriptions, schemas, responses, stderr, and other provider metadata do not determine authority classification. repositoryBound = false only says that RAH did not construct the Tool as a selected-repository capability; it does not claim to constrain provider ambient OS access.

Provider identifiers are bounded and sanitized before they become source-label presentation. Native profile/executable paths, argv, cwd, environment, credentials, endpoints, stderr, correlation IDs, and private bridge aliases remain outside the DTO.

## Conservative external effects

An external ToolRequested event alone has no repository-effect implication. Once ToolStarted is observed with a selected repository, Desktop immediately invalidates outstanding reviewed-commit authorization. A matching finished event, whether success or error, requests repository/workflow observation.

If a started external call has no definite matching finish before turn failure, cancellation, or end-of-stream, Desktop conservatively requests the same refresh and keeps reviewed-commit authorization invalidated. The refresh is observation and review invalidation, not proof of a file change and not rollback. In a neutral no-repository context, external calls remain usable when their host permission allows them and do not manufacture repository state.

Existing first-party mutation and commit handling remains unchanged.

## Refresh Authority

Refresh Authority continues to read current in-memory state and serialize a fresh DTO. It does not reload profiles, spawn or stop providers, reconnect a provider or Codex, rebuild the registry, execute a Tool, mutate repository state, alter review authorization, or change generations.

## Deterministic verification

Focused Desktop tests cover mixed MCP and Process Plugin public names, exact permissions and classifications, bounded labels and redaction, provider-only disconnected selection, atomic activation failure, unknown registry fail-closed behavior, and external requested/started/finished/uncertain lifecycle handling with and without a selected repository. Existing first-party authority and review tests remain in the suite. The frontend static authority test checks that backend-supplied sanitized labels are rendered without authority inference.

The normal workspace gates remain required before commit:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
node --check crates/rah-desktop/frontend/status.js
```

## Explicit non-goals

Task 206 does not add network MCP, Streamable HTTP, OAuth, credentials, provider health polling, restart/reconnect/retry/replay, PluginManager, installation or marketplace flows, profile discovery/persistence/editing/hot reload, generic shell/process/filesystem/Git authority, rollback, stronger OS sandbox or network-isolation claims, or Linux live certification.

## Next task

Task 207 — Windows Desktop External Provider Live Certification — is the next task. Task 206 does not claim that live certification has been performed.
