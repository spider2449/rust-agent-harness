# Task 126 — Desktop repository context binding and isolation

## Scope

Bind every Desktop-created Codex thread to the host-selected canonical
repository root, while preserving the existing host-owned ToolRegistry and
generation lifecycle. Do not add model-selected working directories or process
authority.

## Evidence and decisions

- The certified `codex-cli 0.149.0` schema exposes `cwd` on
  `v2/ThreadStartParams` and returns effective `cwd` plus `instructionSources`
  from `v2/ThreadStartResponse`.
- With `thread/start` omitting `cwd`, the app server uses its launch directory
  and loads that directory's `AGENTS.md`; this is the contamination seam.
- A selected repository supplies one canonical root both to Desktop repository
  tools and the thread-start workspace context.
- No selected repository uses an app-owned neutral workspace and only the
  permission-free Desktop registry. It never derives project context from the
  Desktop launch directory.
- A repository or model generation mismatch fails closed before a chat turn is
  started; reconnect constructs the next generation's registry and workspace
  together.

## Validation

1. Cover the outgoing `thread/start.cwd` and reject an effective-CWD mismatch.
2. Preserve the captured 0.149.0 schema compatibility contract for `cwd` and
   `instructionSources`.
3. Cover stale connection generation rejection and the repository registry's
   unchanged host-owned permissions.
4. Run focused Desktop and runtime-Codex tests, then inspect diff and status.

## Authority boundary

`AGENTS.md` is model instruction context only. It cannot alter the registry,
permission levels, repository mutation policies, or acquire shell, process, or
network authority.

## Task 126 extension: repository-scoped conversation persistence

Desktop durable conversation history is partitioned by an opaque stable key
derived by the host from the canonical selected repository root. The key is
never model-provided and `repository_generation` remains a live freshness check
only. The no-repository workspace uses its own neutral namespace.

The persistence file must not store raw repository paths. Legacy v1/v2 global
transcripts have no provable repository owner, so they remain legacy/unscoped
and are never replayed or presented after selecting a repository. Invalid or
unknown namespace metadata fails closed without falling back to global history.

Switching repositories selects that repository's transcript namespace and
clears active replay context. Switching back may make only the earlier
repository's history available. Model configuration changes preserve the
existing same-repository context-reset behavior; they cannot select a different
repository persistence namespace.
