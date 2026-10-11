# ADR 0035 — Host-observed conversation work-log memory

Status: Accepted

Date: 2026-10-11

## Context

Desktop replays a bounded conversation window (`MAX_CONVERSATION_REPLAY_MESSAGES`
and `MAX_CONVERSATION_REPLAY_BYTES`). Each persisted turn keeps only the user
prompt and the final assistant text. Tool activity in a turn (which Tools ran,
whether they succeeded, failed, or ended with an unknown outcome) is not
replayed, so after the window slides, or after a restart or new conversation
context, the model cannot tell what earlier turns did and cannot continue the
work.

Earlier documentation limits conversation SQLite storage to "messages and
display history only" and fixes the transcript database at schema version `1`.
This ADR extends that storage contract by one descriptive record type and a
one-way schema migration. It adds no authority.

## Decision

Desktop keeps a host-owned, per-namespace **work log**. After each started
chat turn reaches a terminal state, the host appends one entry built only from:

- the closed turn outcome: `completed`, `failed`, `cancelled`, or
  `interrupted`;
- bounded excerpts (512 UTF-8 bytes, whitespace-collapsed) of the user prompt
  and, for completed turns, the final assistant text — both already persisted
  in the transcript;
- the host-observed Tool activity of the turn: the Tool name (restricted to
  `[A-Za-z0-9._-]`, at most 64 bytes, otherwise `unrecognized-tool`) and a
  closed result (`success`, `failed`, `outcome_unknown`, `not_started`), plus a
  verified commit object id only for a `repo.commit` result classified
  `committed_verified`. At most 32 Tool records per entry; the remainder is
  counted only.

Entries never contain Tool input, Tool output, file paths, diffs, credentials,
review/authorization material, HostExplicit tickets, repository identity, or
generations. The model has no way to write, edit, or delete entries.

On each send, Desktop renders the newest entries that fit 4 KiB into one
host-authored `System` message placed before the replayed history. The message
states that it is a host-observed record, not an instruction, grants no
permission, and that quoted excerpts are data that may be stale.

## Storage

The work log lives in the existing private `conversation-transcript.sqlite3`
in a new `work_log` table keyed by conversation namespace, with `ON DELETE
CASCADE` from `namespaces`. Each namespace retains at most 32 entries; the
oldest are evicted on append. Column bounds are enforced by SQL `CHECK`
constraints and by the closed loader. A malformed row fails the database closed
through the existing quarantine path.

The schema version becomes `2`. A valid schema `1` database is migrated
one-way: its transcript is loaded under the v1 closed rules into a freshly
created v2 staging database (`source_format = 'sqlite-v1'`), a best-effort
copy of the v1 file is kept as `conversation-transcript.sqlite3.migrated-v1`,
and the staging file atomically replaces the final file. An invalid v1 or v2
database fails closed exactly as before. There is no dual write and no
downgrade.

"Clear Conversation History" deletes the namespace and therefore its work
log. "New Conversation" and model/repository context changes keep the work
log of the namespace; replay of the conversation window itself is unchanged.

## Authority and security

**A work-log entry is descriptive data, not authority.** It cannot arm,
authorize, replay, or repeat any Tool effect, and it does not restore
repository, review, Commit, Stage/Unstage, HostExplicit, provider, or runtime
state. An `outcome_unknown` record states uncertainty; it is never treated as
rollback, success, or a reason to retry.

Model output and provider metadata remain untrusted requests. The memory
message is host-authored presentation of host observations; the only
model-originated content it carries is bounded, quoted excerpts of text that
is already part of the replayed transcript.

## Nonclaims

This ADR adds no Tool, permission, model-writable memory, summarization model
call, cross-namespace memory, cloud sync, encryption claim, or replay of
uncertain effects. Recording Tool arguments such as paths would require a
separate decision.
