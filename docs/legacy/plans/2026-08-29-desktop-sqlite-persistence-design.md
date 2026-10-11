# Task 127 — Desktop SQLite persistence design gate (Phase A)

Status: research/design only.  No production code, Cargo metadata, SQLite
dependency, or commit is part of this task.

Starting checkpoint: `2fc9037da8c387f8c6d6144850b333d804cc6f02` (`origin/master`
matched at inspection): `feat: isolate Desktop repository context`.

## Decision

Replace only the Desktop conversation-persistence storage backend with one local
SQLite file named `conversation-transcript.sqlite3` in the existing Desktop
application-data directory.  Preserve Task 126's host-selected namespace,
presentation, restart, trimming, and explicit Resume semantics exactly.  This
is not a general Desktop state database.

Recommended Phase B dependency: `rusqlite` with feature `bundled`, directly in
`rah-desktop`'s Windows dependency section.  It is a synchronous, narrow
wrapper around SQLite and supplies connections, prepared statements,
transactions, and foreign-key-compatible SQL without creating an async pool or
changing any RAH public boundary.

## Current committed JSON audit

`conversation_persistence.rs` currently has three strict serde formats:

| Format/file | Shape and current startup treatment |
|---|---|
| V1, `conversation-transcript-v1.json` | `{ version: 1, records: [ completed_pair | context_separator ] }`; only read if no V3/V2 exists.  It is legacy and unscoped. |
| V2, `conversation-transcript.json` | `{ version: 2, epochs: [{ id, parent_epoch_id, boundary, history_trimmed_before, pairs: [{ user, assistant }] }] }`; only read if no V3 exists.  It is legacy and unscoped. |
| V3, `conversation-transcript-v3.json` | `{ version: 3, namespaces: { namespace_key: V2 } }`; the current authoritative repository-scoped format.  Unknown fields and unknown separator values fail parsing. |

The namespace key is exactly `neutral-v1` or `repo-sha256:` followed by 64
lowercase hex characters.  Namespace selection is host input; only the selected
V3 namespace is readable or writable.  V1/V2 records are never made visible
through a selected namespace; selecting one converts the in-memory backing to
an empty V3 map, preserving the legacy file but not assigning its content.

An epoch is an ordered, positive, increasing-id segment.  The first epoch has
no boundary unless it carries `history_trimmed_before`; subsequent epochs have
a boundary.  A parent is optional, must name an earlier epoch in the same V2
lineage, and is permitted only on an `application_restarted` epoch.  A
`ContextSeparator` creates a new unparented epoch.  Presentation emits a
synthetic `history_trimmed` separator before an epoch marked
`history_trimmed_before`, then its stored boundary, then each pair as adjacent
user/assistant completed messages.

Only complete pairs persist: a user input without a terminal assistant reply is
never committed.  A process restart is per non-empty namespace: first selection
after startup appends an empty `application_restarted` epoch only for that
namespace.  Selection itself can display that namespace's durable transcript;
it does not replay it to the model.

Resume is available only from the current empty, unparented,
`application_restarted` epoch.  It scans backward over empty restart epochs to
the nearest non-empty epoch, rejecting a fresh-conversation/model/repository
boundary; a trim marker is incompatible.  Reconstruction follows that source's
parent chain, fails closed for missing/trimmed/incompatible epochs, and returns
only complete pairs.  `commit_resume_lineage` first validates a candidate whose
current restart epoch points to that source; it writes durably and changes memory
only after successful replacement.

Existing write bounds are `MAX_NAMESPACES=64`, `MAX_BYTES=256 KiB`,
`MAX_MESSAGE_BYTES=16 KiB`, `MAX_RECORDS=79`, `MAX_PAIRS=64`, and
`MAX_EPOCHS=16`.  V1 trimming removes whole prefix segments through a separator
and inserts `history_trimmed`; V2/V3 trimming removes whole oldest epochs,
severs the new first epoch's parent, and marks it trimmed.  It cannot trim a
single remaining epoch, so the mutation fails.  V3 applies the V2 limits per
namespace but additionally had one serialized-file byte cap.

Current clear removes only the selected V3 namespace and atomically rewrites
the map; V1/V2 clear removes their private files (with ordered error behavior).
Warnings are sanitized `RestoreFailed` and `SaveFailed`.  An unreadable,
oversized, malformed, unsupported, or ownership-invalid JSON file is renamed
to `<file>.corrupt` best-effort and starts failed/empty, never guesses an owner.

JSON writes validate first, write a bounded temporary file, `sync_all`, then
perform a native Windows replacement (`ReplaceFileW`, with a single
replace-existing `MoveFileExW` fallback) or platform rename.  Temporary names
are cleaned at startup; a failed replacement removes its temporary file.

## Backend decision matrix

| Candidate | Assessment |
|---|---|
| `rusqlite` + bundled SQLite | **Recommend.** Mature synchronous wrapper with prepared statements, explicit transactions and foreign-key support. Bundling compiles and statically links SQLite, so Windows x86_64 distribution does not depend on a separately installed `sqlite3.dll`. One small local database and an existing mutex suit this API. |
| `rusqlite` + system SQLite | Rejected: its default discovery seeks a system SQLite (including pkg-config/vcpkg on MSVC), adding host/install and version variability with no benefit for this Desktop-only store. |
| `sqlx` SQLite | Rejected: supports migrations and async/pooling, but is materially broader and its SQLite feature also bundles SQLite. No network DB, concurrent query workload, or async SQL requirement justifies its runtime, macros, pool model, and larger dependency closure. |
| Another wrapper | No concrete need. `diesel` is ORM-shaped and heavier; direct FFI would recreate wrapper responsibilities. |

At the time of research, the workspace has no direct or transitive
`rusqlite`, `libsqlite3-sys`, or `sqlx` entry in `Cargo.lock`.  A bundled
`rusqlite` addition will introduce `rusqlite`, `libsqlite3-sys`, and a C compile
build dependency such as `cc` (plus its small Rust support closure), so lockfile
growth is expected but not enormous.  Windows MSVC must already provide its
normal C/C++ build toolchain; no SQLite runtime DLL or system package should be
required.  Phase B must verify this on the supported Windows x86_64 build and
CI. SQLite is public-domain; rusqlite and libsqlite3-sys licensing and current
advisories must be recorded from the resolved versions in the Phase B supply
chain review.

`rusqlite::Connection` is documented as `Send` but `!Sync`.  That fits the
existing ownership: one connection remains inside `Persistence`, and the
existing `std::sync::Mutex<Persistence>` serializes access.  Phase B must add a
compile-time/Tauri-state validation using the resolved version.  No guard may cross an
`.await`; Desktop calls are already synchronous persistence operations.  Opening
per operation weakens transaction ownership and repeats setup; a pool has no
demonstrated consumer.

## Proposed schema and integrity model

Use schema version `1`, with `PRAGMA user_version=1` and a one-row metadata
record as a deliberately redundant completeness check.  Both must agree.

```sql
CREATE TABLE schema_metadata (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  schema_version INTEGER NOT NULL CHECK (schema_version = 1),
  migration_complete INTEGER NOT NULL CHECK (migration_complete IN (0, 1)),
  source_format TEXT NOT NULL CHECK (source_format IN ('empty', 'v3')),
  imported_namespace_count INTEGER NOT NULL CHECK (imported_namespace_count >= 0),
  imported_epoch_count INTEGER NOT NULL CHECK (imported_epoch_count >= 0),
  imported_pair_count INTEGER NOT NULL CHECK (imported_pair_count >= 0)
);

CREATE TABLE namespaces (
  namespace_key TEXT PRIMARY KEY NOT NULL
    CHECK (namespace_key = 'neutral-v1' OR
      (length(namespace_key) = 76 AND substr(namespace_key, 1, 12) = 'repo-sha256:'
       AND substr(namespace_key, 13) NOT GLOB '*[^0-9a-f]*'))
);

CREATE TABLE epochs (
  namespace_key TEXT NOT NULL,
  epoch_id INTEGER NOT NULL CHECK (epoch_id > 0),
  parent_epoch_id INTEGER,
  boundary TEXT,
  history_trimmed_before INTEGER NOT NULL CHECK (history_trimmed_before IN (0, 1)),
  PRIMARY KEY (namespace_key, epoch_id),
  FOREIGN KEY (namespace_key) REFERENCES namespaces(namespace_key) ON DELETE CASCADE,
  FOREIGN KEY (namespace_key, parent_epoch_id)
    REFERENCES epochs(namespace_key, epoch_id),
  CHECK (parent_epoch_id IS NULL OR parent_epoch_id < epoch_id),
  CHECK (boundary IS NULL OR boundary IN (
    'new_conversation', 'repository_changed', 'model_configuration_changed',
    'repository_and_model_changed', 'application_restarted', 'history_trimmed'))
);

CREATE TABLE pairs (
  namespace_key TEXT NOT NULL,
  epoch_id INTEGER NOT NULL,
  pair_index INTEGER NOT NULL CHECK (pair_index >= 0),
  user_text TEXT NOT NULL CHECK (length(CAST(user_text AS BLOB)) <= 16384),
  assistant_text TEXT NOT NULL CHECK (length(CAST(assistant_text AS BLOB)) <= 16384),
  PRIMARY KEY (namespace_key, epoch_id, pair_index),
  FOREIGN KEY (namespace_key, epoch_id)
    REFERENCES epochs(namespace_key, epoch_id) ON DELETE CASCADE
);
```

The current JSON contract permits empty pair text, so SQL must not newly reject
it.  Rust must retain byte-length checks and must run the existing strict V2
semantic validator after every namespace load/import: SQL cannot conveniently
express first-epoch boundary rules, monotonic whole-lineage ordering, aggregate
pair/epoch/record caps, or the restriction that parents occur only on restart
epochs.  Schema-open validation must also require exactly the expected named
tables/primary keys/foreign keys/CHECK-bearing DDL, `foreign_keys=ON`, one valid
metadata row, matching user version, known enum values, expected count metadata,
and `PRAGMA integrity_check`.  Any discrepancy is corruption/unsupported and
fails closed; no unknown storage value is coerced.

Pairs, not generic message rows, are selected.  This stores precisely the
current durable unit and makes an incomplete model turn structurally
unrepresentable.  Generic messages add an ordering/status product model and
would require new interpretation rules for Resume.  `pair_index` preserves
order inside the epoch and is unique by primary key.

## PRAGMAs, journal, and transaction boundaries

On every opened connection, Phase B should set and verify:

- `PRAGMA foreign_keys = ON` before transactions (SQLite does not enforce them
  unless enabled per connection).
- `PRAGMA journal_mode = DELETE` and verify the returned mode.
- `PRAGMA synchronous = FULL` for the durable conversation contract.
- `PRAGMA busy_timeout = 250` milliseconds (a bounded wait, surfaced as a
  save/restore failure after expiry).
- Do not set `temp_store`; default behavior is sufficient for these bounded
  statements.

DELETE journal is chosen over WAL.  SQLite's rollback journal gives normal
atomic commit/recovery with one writer and avoids persistent `-wal` and `-shm`
sidecars that complicate Windows backup, archive, quarantine, and shutdown.
WAL improves readers/writers concurrently, which Desktop has not demonstrated;
it would require all three files to move together and explicit checkpoint
policy.  `synchronous=FULL` favors committed-state durability over a small local
write-performance gain.  This transaction atomicity applies only to local
Desktop persistence, never tool side effects, repository mutations, MCP/plugin
effects, or provider/model effects.

All mutation operations use an immediate write transaction, prepared
parameterized statements, validate the resulting affected namespace before
commit, and leave in-memory state unchanged on failure:

1. Append complete pair: ensure the selected namespace/current epoch, insert
   both texts in one pair row, trim whole oldest epochs of that namespace if a
   retained per-namespace limit needs it, sever the first retained parent and
   mark it trimmed, validate, then commit.
2. Commit Resume lineage: read/verify current and source epochs scoped by the
   selected namespace, set only the current epoch parent, validate, commit.
3. New conversation/context separator: insert the next positive epoch id with
   its exact boundary and no parent, validate, commit.
4. Clear: delete only `namespaces.namespace_key = ?`; cascades remove only its
   epochs/pairs, then commit.  It remains idempotent.

Every public query that reads epochs or pairs must bind `namespace_key`; Phase B
tests must inspect each query seam to prevent an unscoped ownership lookup.

## Bounds after SQLite

| Existing bound | Classification and Phase B rule |
|---|---|
| `MAX_MESSAGE_BYTES` | A, retain at 16 KiB in Rust and SQL byte checks. It bounds untrusted persisted text. |
| `MAX_PAIRS` | B, retain at 64 per namespace as current durable/replay safety. It is not permission to conflate this with a future request replay cap. |
| `MAX_EPOCHS` | B, retain at 16 per namespace; preserve whole-epoch pruning and its trim marker/lineage severing. |
| `MAX_RECORDS` | B, retain at 79 per namespace because it limits presented durable records and is part of the current trim validator. |
| `MAX_NAMESPACES` | A initially, retain 64 namespaces per database. It is a host-owned local storage-growth bound; revisiting it needs separate evidence, not SQLite's capacity alone. |
| `MAX_BYTES` | D for the one 256 KiB JSON snapshot limit. Do not apply it to the whole SQLite file: it includes pages/journal overhead and unrelated namespaces. Per-message and per-namespace structural limits remain; no new global DB byte quota is introduced in this backend migration. |

## One-time V3 migration and recovery

Startup precedence is exact:

1. If final `conversation-transcript.sqlite3` exists, open read/write without
   creating a replacement, validate header/schema/metadata/integrity and every
   namespace.  A valid database with `migration_complete=1` is authoritative;
   never read or import V3 JSON.
2. If the final DB is absent and V3 exists, parse it through the existing
   bounded V3 reader/validator.  Create only a same-directory staging file
   `conversation-transcript.sqlite3.importing`; set schema and metadata with
   `migration_complete=0`; in one transaction insert all V3 namespaces/epochs/
   pairs, validate counts/structure, set complete and `user_version=1`, and
   commit.  Reopen the staging DB and run the normal full validation.  Only then
   close and atomically rename it to the final DB.  V3 is untouched until this
   point.
3. If final DB is absent and only V1/V2 exists, preserve their current
   fail-closed rule: do not infer or import them to either repository or
   `neutral-v1`.  Create an empty complete SQLite DB only when the caller first
   selects/writes a current namespace (or initialize an empty final DB with
   `source_format='empty'`); legacy content remains inert.
4. If no persistence exists, create/validate an empty complete final DB through
   the same staging-and-rename protocol.

After a successful final rename, rename V3 best-effort to
`conversation-transcript-v3.json.migrated-v3` (never delete it in Task 127).
The completed final DB remains authoritative whether this archival rename
succeeds or not, preventing stale re-import.  On the next startup an existing
valid DB still wins.  A later, explicit maintenance task may define bounded
cleanup of migrated backups.

| Crash/failure point | Next startup behavior |
|---|---|
| Before staging DB creation | V3 remains source; retry import. |
| During schema creation/import before commit | Final DB absent; discard/quarantine stale `.importing`, retain V3, retry from validated V3. |
| After import commit, before staging validation/final rename | Final DB absent; validate the complete staging DB; if valid, finish rename, otherwise quarantine staging and retry from V3. |
| After final rename, before V3 archive | Final validated DB is authoritative; ignore V3 and retry only the inert archive rename. |
| After V3 archive | Final DB remains authoritative; migrated backup is inert. |

No final filename is created with an incomplete marker.  Therefore an existing
final database is never ambiguous with a partial V3 import.  If the final DB
exists but is unreadable, invalid-headered, schema-incompatible, malformed,
integrity-failed, locked past timeout, or otherwise corrupt, quarantine it
best-effort as `conversation-transcript.sqlite3.corrupt` and start the existing
sanitized failed state; **do not fall back to V3**.  If quarantine cannot be
performed, still fail closed and do not write a substitute database over it.

With DELETE journaling, ordinary `-journal` files are transaction artefacts and
are not independent authority; startup must let SQLite recover before judging
the database.  A quarantine attempt first closes the connection, then moves the
main file and any contemporaneous `-journal` to the same bounded quarantine
generation (or leaves all files in place and fails closed if a coherent move is
not possible).  Quarantine naming must avoid overwriting an existing evidence
file and retain at most one prior corrupt generation; removal/rotation failure
must not expose another namespace.  Disk-full, permission, I/O, busy/locked,
and transaction errors map to existing `SaveFailed`/`RestoreFailed` surfaces
without raw paths, SQL, or SQLite diagnostics in the UI.

## Future deterministic test matrix

Phase B must retain existing behavior tests and add fault-injection seams for
SQLite open, transaction, commit, rename, and archive.  At minimum:

- Namespace isolation: append A then query B (zero A rows); append B leaves A
  unchanged; A→B→A presentation; restart A and B independently; neutral cannot
  see A/B; clear A leaves B; corrupt A lineage fails closed rather than showing
  B; Resume is namespace-bound; and an audit proves every ownership-requiring
  public SQL path binds namespace.
- Migration fixtures: empty install; V3 neutral-only, A-only, A+B, and every
  configured bound; corrupt/unsupported V3; interruption before and after
  import commit; valid DB with stale V3; corrupt final DB with valid stale V3;
  V1-only; V2-only; too-new and too-old DB schema; write/commit failure; and
  disk/write fault.
- Invariants: unknown separator/schema values, invalid namespace, cross-
  namespace parent/pair insertion, duplicate pair index, malformed required
  schema, foreign-key enforcement, bounded busy timeout, restart separator,
  pruning/`history_trimmed_before`, Resume source/reconstruction/commit, and
  presentation equality against V3 fixtures.

## ADR, authority, and Phase B sequence

No ADR is recommended: the backend is a private Desktop implementation detail;
it introduces no RAH crate direction change, stable boundary, protocol,
authority, runtime model, or plugin/security model change.  The new direct
dependency edge is `rah-desktop -> rusqlite`, justified solely for local
conversation persistence.  Reassess if implementation requires exposing SQL,
sharing this database with another subsystem, or changing a stable boundary.

SQLite remains storage, not authority.  The model, provider, AGENTS.md, and
persisted data cannot select a namespace or issue SQL.  Repository root,
ToolRegistry, Codex cwd, policy, sandbox, filesystem, network, and tool
authority are unchanged.  No generic SQL, shell, filesystem-write, or network
capability is exposed.

Recommended Phase B order:

1. Add the scoped bundled dependency and compile-time synchronization proof.
2. Build schema/open/validation and deterministic SQLite fault seams behind the
   existing private `Persistence` API.
3. Port V3-equivalent namespace, presentation, pair, epoch, trimming, restart,
   Resume, and clear operations with namespace-bound prepared statements.
4. Implement staging V3 migration, archive, corruption/quarantine, and tests.
5. Run formatting, focused Desktop tests, workspace checks/tests/clippy, then
   inspect `git diff --check`, diff, and status.  Do not add unrelated state or
   authority.

## Research sources

- [rusqlite API documentation](https://docs.rs/rusqlite/latest/rusqlite/) —
  connection, prepared-statement, and transaction API.
- [rusqlite Connection documentation](https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html)
  — documented `Send`/`Sync` implementation to re-verify at resolution time.
- [rusqlite repository](https://github.com/rusqlite/rusqlite) — default system
  discovery behavior and bundled-build feature context.
- [SQLite PRAGMA documentation](https://www.sqlite.org/pragma.html) —
  foreign-key, journal, synchronous, and busy-timeout behavior.

## Task 127 implementation status (Phase C)

Implemented in the dirty Task 127 worktree; no commit has been made. The
Desktop-only dependency is `rusqlite 0.37.0` with `bundled`, resolving
`libsqlite3-sys 0.35.0`. The database filename is
`conversation-transcript.sqlite3`; schema version `1` has the four specified
tables: `schema_metadata`, `namespaces`, `epochs`, and `pairs`. Connections set
and verify `foreign_keys=ON`, `journal_mode=DELETE`, `synchronous=FULL`, and
`busy_timeout=250`.

The final filename is authoritative only after the migration transaction has
committed and staging has been promoted. Valid final SQLite wins over original
or migrated V3 JSON; archive failure leaves original V3 inert. Staging is
non-authoritative. Invalid headers, integrity failures, metadata/version
mismatches, and incompatible schemas fail closed and are best-effort
quarantined to the bounded `conversation-transcript.sqlite3.corrupt` name.
Quarantine never enables a V3 fallback.

Schema validation checks user version, completion metadata, integrity,
namespace data, and exact expected table DDL from `sqlite_master`; matching
`user_version` alone is rejected. Private deterministic seams cover creation,
migration pre-commit/commit, post-commit archive, and mutation transactions.
`MAX_NAMESPACES=64` remains per DB; `MAX_MESSAGE_BYTES=16KiB`,
`MAX_RECORDS=79`, `MAX_PAIRS=64`, and `MAX_EPOCHS=16` remain per namespace.
`MAX_BYTES=256KiB` is named and commented as a bound only for a single legacy
V3 JSON migration input, never the SQLite database. Agent request replay bounds
`MAX_CONVERSATION_REPLAY_MESSAGES` and `MAX_CONVERSATION_REPLAY_BYTES` remain
separate model-context bounds.

Schema validation compares the complete expected DDL in `sqlite_master`, so it
certifies the required tables, columns, primary keys, foreign keys, unique
primary-key indexes, and CHECK constraints together; it also rejects extra
user tables. This is deliberately stronger than trusting `user_version`.

Quarantine uses the bounded filename `conversation-transcript.sqlite3.corrupt`
and, if a DELETE rollback journal exists, the paired
`conversation-transcript.sqlite3.corrupt-journal`. One prior generation is
rotated away before the replacement pair is moved. Any rotation, journal move,
or main-file move failure leaves startup fail-closed; a failed main move makes
a best-effort journal rollback. The presence of this quarantine marker keeps
old V3 JSON inert on later starts, so corrupted authoritative SQLite can never
resurrect stale JSON history.

Critical deterministic migration coverage is explicit: (A) an archive/rename
fault after a committed V3 import proves the final SQLite file wins after a
restart and row counts contain no duplicate namespace, epoch, or pair; (B) a
valid final SQLite file wins over subsequently supplied stale V3; and (C) a
corrupt final SQLite with valid V3 produces only sanitized `RestoreFailed`,
quarantines SQLite, and remains fail-closed across a further startup.

The normal shared-target Desktop link failure is MSVC `LNK1318` while creating
the reqwest PDB. It is a target-directory/toolchain contention artifact, not a
product-code result; clean isolated targets are the deterministic validation
path. The final clean isolated workspace test, Clippy, and release-build
results are recorded in the Task 127 completion report rather than claimed
here before they run.

## Task 127 deterministic closeout certification

The resolved Desktop dependency is `rusqlite 0.37.0` with its `bundled`
feature, which resolves `libsqlite3-sys 0.35.0`. The durable database remains
`conversation-transcript.sqlite3`, schema version `1`. The schema has exactly
`schema_metadata`, `namespaces`, `epochs`, and `pairs`; their composite primary
keys, namespace-scoped foreign keys, and CHECK constraints are verified by
matching the complete expected DDL in `sqlite_master`, not merely
`PRAGMA user_version`. The primary keys supply the required unique indexes;
no additional secondary index is needed for the namespace-bound queries.

All connection opens set and verify `foreign_keys=ON`, `journal_mode=DELETE`,
`synchronous=FULL`, and `busy_timeout=250`. CASE A passed: after a committed
V3 import with injected archive failure, the final SQLite DB remained
authoritative across restart, the original V3 stayed inert, and namespace,
epoch, and pair counts showed exactly one import. CASE B passed: valid SQLite
wins over a subsequently supplied stale V3 with no stale-data resurrection.
CASE C passed: corrupt authoritative SQLite is quarantined and produces only
`RestoreFailed`; valid older V3 remains inert across subsequent restart.
Pre-commit migration failure also remains non-authoritative and safely retries
from V3; namespace isolation and failed mutation rollback are deterministic.

Quarantine is bounded to `conversation-transcript.sqlite3.corrupt` and its
paired DELETE sidecar `conversation-transcript.sqlite3.corrupt-journal`. An
existing generation is removed before the replacement pair is moved. A failed
rotation, journal move, or main-file move fails closed (with a best-effort
journal rollback), and the marker prevents any V3 fallback. Quarantine never
activates another source of history.

`MAX_NAMESPACES=64` bounds the database-wide namespace count.
`MAX_BYTES=256 KiB` bounds only one legacy V3 JSON import input and never the
SQLite database file. `MAX_MESSAGE_BYTES=16 KiB` is a UTF-8 byte bound in both
Rust (`String::len`) and SQL; the multibyte regression test proves it.
`MAX_RECORDS=79`, `MAX_PAIRS=64`, and `MAX_EPOCHS=16` are retained semantic
per-namespace durable-history bounds. They are distinct from model replay
bounds `MAX_CONVERSATION_REPLAY_MESSAGES=8` and
`MAX_CONVERSATION_REPLAY_BYTES=32 KiB`.

The serialized isolated command `cargo test --workspace -j 1` with
`CARGO_TARGET_DIR=target-task127-final-workspace` passed with exit code 0;
the Desktop crate ran 109 tests, with two explicitly host-only tests ignored.
Isolated all-target/all-feature workspace Clippy passed with
`CARGO_TARGET_DIR=target-task127-final-clippy`. A shared default target may
encounter the known MSVC `LNK1318` reqwest-PDB contention; this is classified
as target-directory/toolchain contention, not a product-code failure.
The rebuilt isolated release command `cargo build -p rah-desktop --release`
passed with `CARGO_TARGET_DIR=target-task127-final-release`; its executable is
self-contained with respect to SQLite and requires no adjacent `sqlite3.dll`.
