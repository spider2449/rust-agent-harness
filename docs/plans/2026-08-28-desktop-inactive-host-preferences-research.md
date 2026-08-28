# Task 121 — Desktop Inactive Host Preferences Research

## 1. Current state

Task 117 resolves the Codex executable only at explicit connection time with
the Rust-private precedence `RAH_CODEX_EXECUTABLE` override, freshly verified
certified baseline, then PATH fallback. JavaScript receives only source,
version, and status. Task 120 adds a closed, normalized `llama_cpp` endpoint
(`http`/`https`, IP or ASCII DNS host, port, fixed `/v1`) and one human-invoked
readiness check. It has no credentials, process ownership, or persistence.

Desktop already persists conversation history in the application-local-data
directory. That transcript is a separate bounded display/replay domain; it is
not a settings container. It restores no active replay context or authority.

## 2. Product motivation

Remembering a human's desired provider/model avoids retyping configuration
after an application restart. It must not make a stored selection evidence
that an executable, endpoint, repository, profile, or runtime is presently
valid. The useful product outcome is an editable, inactive form prefilled with
safe data and an explicit Connect action.

## 3. Candidate persisted fields matrix

| Candidate | Decision | Rationale and validation |
| --- | --- | --- |
| Provider enum | Persist | Closed `inherit`, `openai`, `ollama`, `lm_studio`, `llama_cpp`; semantic combinations remain Rust-validated. |
| Model identifier | Persist when provider is not `inherit` | Arbitrary model names are acceptable local inactive data, but require valid UTF-8, no C0/C1 controls or NUL, no leading/trailing Unicode whitespace, and 1..=256 UTF-8 bytes after validation. They may be private and must not enter committed diagnostics. |
| Literal-loopback llama.cpp endpoint | Persist | Closed normalized object only: `127.0.0.0/8` or `::1`, `http` or `https`, port 1..=65535, fixed implicit `/v1`. It is convenient for the common local server and contains no arbitrary URL/path. |
| Non-loopback LAN endpoint | Do not persist in v1 | Task 120 may use it for one explicit connection under ADR 0015, but a LAN address/name can expose host topology and should require fresh deliberate entry after restart. |
| Remote DNS endpoint | Do not persist in v1 | It is privacy-sensitive destination data and must be deliberately re-entered; storing it adds little over a future explicit privacy/UX decision. |
| HTTP versus HTTPS | Persist only for a literal-loopback endpoint | Both are permitted there. Non-loopback HTTP/HTTPS is not persisted because non-loopback endpoints are excluded. |
| Codex executable source identity | Do not persist | Current resolution is automatic at Connect. A symbolic source would be stale/redundant and cannot safely recreate an override; no preference is needed. |
| Supported Codex version | Do not persist | It is code-owned certified-baseline metadata, not user preference. |

**Strong recommendation:** v1 persists provider, bounded model identifier, and
only a normalized literal-loopback `llama_cpp` endpoint. It persists no
non-loopback endpoint. This is **Option A** for endpoint scope. The restriction
keeps the initial durable surface small and avoids silently retaining LAN/remote
topology while preserving Task 120's explicit non-loopback connection feature.

## 4. Forbidden persisted fields

The following are forbidden in v1: absolute Codex executable path; llama-server
executable path; GGUF/model filesystem path; argv; cwd; environment;
credentials/tokens/API keys; arbitrary HTTP headers; proxy configuration;
redirect destination; readiness result; runtime connection state; Codex
thread/session identifiers; active turn; model generation; repository
path/selection; ToolRegistry inventory; permission grants; trusted-profile
activation; MCP/plugin process state; provider child PID; cancellation state;
transient frontend errors; conversation transcript/history; and private runtime
capability objects.

Also forbidden: URL paths, queries, fragments, userinfo, an endpoint's rendered
`normalized` URL, endpoint reachability/TLS/auth outcomes, executable hash,
baseline-store location, provider-native configuration, model context/window
settings, model installation/download state, and any unknown extension field.

## 5. Recommended v1 schema

Use the separate fixed filename `desktop-preferences.json` directly in the
same Rust-resolved Tauri `app_local_data_dir()` used for conversation files.
It shares the application-owned directory, never the transcript file or a
repository directory. The size limit is 4 KiB serialized UTF-8 bytes.

The schema is closed and has exactly `version: 1` and `model`. Fields that do
not apply are omitted, never represented as `null`.

```json
{"version":1,"model":{"provider":"llama_cpp","model":"local-model","endpoint":{"scheme":"http","host":"127.0.0.1","port":8080}}}
```

The only other valid shapes are `{"provider":"inherit"}` and, for `openai`,
`ollama`, or `lm_studio`, `{"provider":"…","model":"…"}`. `endpoint` is
required only for `llama_cpp`; it is prohibited otherwise. Endpoint `host` is
the canonical literal address without IPv6 brackets; `/v1` is not serialized.
The persisted endpoint parser must additionally require `IpAddr::is_loopback()`
(thus rejecting `localhost`, DNS, and all non-loopback literals), retain the
Task 120 closed scheme/port rules, and serialize the canonical parsed address.

Use typed Rust structs/enums with `deny_unknown_fields`, exact snake-case enum
strings, and whole-document semantic validation. Reject duplicate JSON object
keys before typed deserialization: generic JSON map decoding alone is not an
adequate duplicate-key policy. JSON is UTF-8 only; invalid UTF-8, BOM-prefixed
bytes, malformed Unicode, or a non-canonical semantic value are invalid.
Canonical serialization emits compact UTF-8, fixed field order shown above,
one final newline, canonical endpoint spelling, and no optional/null fields.

## 6. Startup/inactive semantics

The exact restart path is:

```text
resolve app-local-data directory
 -> bounded read desktop-preferences.json
 -> strict parse and whole-record validation
 -> construct inactive desired DesktopModelSelection (or built-in inherit default)
 -> readiness = NotTested; model generation = 0
 -> ConnectionState = NotConnected
 -> wait for explicit human Connect
```

This path performs no network operation or readiness check, launches no
process, resolves no Codex executable, creates no Codex runtime or
ToolRegistry, selects no repository, activates no profile/permission, and does
not activate transcript replay. Generation is process-local and starts at `0`;
it is never derived from disk. Explicit Connect revalidates the then-current
desired selection, freshly resolves/verifies Codex, captures an immutable
connection snapshot, and only then creates the active runtime. Existing Task
120 changed-selection/reconnect rules remain unchanged.

## 7. Validation/corruption behavior

Choose **A: fail closed to built-in defaults with a sanitized warning**. The
implementation must make no partial acceptance of any invalid record.

| Condition | Deterministic result |
| --- | --- |
| File absent | Built-in `inherit` desired state, no warning. |
| Empty, malformed, truncated, invalid UTF-8/BOM | Defaults and `preferences_restore_failed` warning. |
| Unsupported version, unknown field, duplicate key, invalid enum | Defaults and warning. |
| Invalid model or endpoint, semantic mismatch, oversize file | Defaults and warning. |

Leave invalid data in place in v1. Do not rename/quarantine it: a bounded
preferences file has no recoverable authority value, and automatic file moves
add failure/diagnostic behavior without improving safe startup. A later valid
Apply atomically replaces it. The frontend warning is closed and path-free; it
must not expose parser detail or file contents.

## 8. Atomic write/failure semantics

Choose **Option B**: fully validate and normalize; update process-local desired
state; then persist a sanitized snapshot. A save failure is a non-fatal,
path-free warning: the explicit Apply took effect for this process, but it will
not necessarily survive restart. Do not roll back desired state or retry a
write automatically.

Serialize Apply/persistence under one host-owned mutex so a later Apply cannot
be overwritten by an earlier snapshot. Build and size-check the complete bytes
before touching disk. Reuse the proven *mechanism*, not the conversation file
or schema: create a unique same-directory temporary file, write, `sync_all`,
close, then use `ReplaceFileW` for an existing destination and
`MoveFileExW(..., MOVEFILE_WRITE_THROUGH)` for first creation. Never
delete-then-rename; no cross-volume move/copy; safely clean only exact private
temporary names. This is best-effort atomic replacement, not a power-loss or
rollback guarantee. No write occurs for readiness, Connect, Disconnect, Cancel
Turn, generation-only changes, or transient errors.

## 9. Privacy/redaction

The local preferences file is host-owned state protected only by the current
user's normal application-local-data access; v1 adds no encryption. A model id
and loopback port can still be sensitive and are never written to committed
diagnostics, crash/error logs, JavaScript error text, or transcript data.

Non-loopback hostnames/IPs are deliberately not retained. If a later version
proposes them, its UI must disclose durable endpoint retention and its logs,
crash reports, committed evidence, and frontend error text must continue to
redact endpoint identity. Local persistence permission never authorizes public
disclosure.

## 10. Conversation-persistence separation

`desktop-preferences.json` is a host configuration record; the existing
`conversation-transcript.json` remains a bounded conversation presentation
record. Restart may restore both independently: preferences prefill inactive
desired configuration, while transcript may display history. Neither creates a
runtime or replay context. The files, schemas, locks, warnings, cleanup names,
and reset operations remain separate.

## 11. Generation/reconnect behavior

No generation is durable. Restored desired state begins at generation `0` and
readiness `NotTested`. A new process has no active snapshot to compare against.
After explicit Connect, the current validated desired selection is captured as
the connection snapshot. Subsequent Task 120 structural equality rules decide
whether an in-process Apply changes generation and requires reconnect; no
stored value manufactures continuity with a prior process.

## 12. Reset/default semantics

v1 should include a future **Reset Model Preferences / Restore Defaults**
command in Task 122's scope, but no UI is added here. It writes the canonical
default `{"version":1,"model":{"provider":"inherit"}}` rather than deleting
the file, so reset is deterministic and atomic. It clears only host
preferences, never conversation history. It does not disconnect an already
active runtime; it changes the desired configuration and the usual UI reports
reconnect required if it differs from the active snapshot.

## 13. Security/authority analysis

This persistence adds no active authority boundary. It carries only bounded
untrusted-at-activation preference data and cannot manufacture a path, secret,
network destination beyond loopback, process, registry, repository, profile,
or permission. Every activation remains an explicit, fresh Rust-side action.
Model output cannot select or mutate this record. ADR 0011 remains the static
trusted-profile composition boundary; a preference record cannot select or
activate it. ADR 0015 remains the explicit one-connection endpoint-selection
authority; durable restoration performs no endpoint connection or probe.

## 14. ADR conclusion

No ADR 0016 is required. The selected v1 data is Desktop-private inactive
state, has a closed schema, restores no authority, and introduces no dependency
or public RAH boundary. Persisting non-loopback endpoints, credentials,
executable/resource identities, or any auto-activation behavior is excluded
from Task 122 and requires fresh authority/privacy review before reconsideration.

## 15. Deterministic implementation acceptance matrix

| Area | Required deterministic evidence |
| --- | --- |
| Schema | Each valid provider shape round-trips to canonical bytes; nulls, unknown fields, duplicate keys, wrong versions, and invalid enum values fail. |
| Bounds | 4 KiB file, 256-byte model, malformed/control/whitespace model, port bounds, and every non-loopback/DNS/`localhost` endpoint are rejected. |
| Startup | Valid restore yields desired state only, generation 0, `NotTested`, and `NotConnected`; instrumentation proves no resolver, network, process, registry, repository, or transcript activation call. |
| Corruption | Every invalid input produces defaults plus only the closed restore warning; no partial field survives. |
| Write | Apply writes exactly normalized validated data; no-op/forbidden event classes write nothing; injected create/replace failures preserve prior on-disk complete snapshot and give only save warning. |
| Windows | Same-directory temporary naming, first write, replacement, access-denied fallback, lock/replacement failure, and stale exact temp cleanup are covered through an injectable seam. |
| Reset | Reset writes inherit-only preferences, preserves transcript, does not disconnect active runtime, and observes normal reconnect-required semantics. |
| Privacy | Presentation/error JSON contains no storage path, endpoint/model details in errors, executable identity, generations, credentials, repository/profile/tool data, or transcript. |
| Resume | Restored preferences do not change Task 115B availability/lineage; Resume succeeds only under its existing fresh runtime/repository/model-context checks. |

## 16. Windows-specific requirements

Resolve only Tauri `app_local_data_dir()`; do not hardcode a user profile path.
Use Unicode native paths and same-directory temporaries. Existing destination
replacement uses `ReplaceFileW`, with the already-proven narrow
`MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` fallback only for the documented
`ERROR_ACCESS_DENIED` case; first creation uses `MoveFileExW(WRITE_THROUGH)`.
Treat sharing violations, antivirus/indexer interference, and failed native
calls as save failure, never a reason to delete the destination, retry, or
claim power-loss durability.

## 17. Recommended Task 122 scope

Implement this private preferences module and closed Rust command seam only:
bounded strict load/save, duplicate-key-aware JSON parse, atomic Windows write,
startup construction of inactive desired model state, Apply persistence warning,
and reset command semantics. Add deterministic Rust tests and minimal frontend
presentation only where required to display/submit existing model configuration
and sanitized warnings. Do not add dependencies, persistence of non-loopback
endpoints, executable selection, readiness/network/process work, transcript
changes, public RAH API changes, or a new ADR.

## 18. Explicit non-goals

No Rust/frontend production implementation in Task 121; no endpoint discovery,
probe, auto-connect/reconnect, server launch/ownership, model/GGUF selection,
credentials, headers/proxies/redirect policy, encryption/cloud sync, multiple
preference profiles, conversation migration, durable generation/lineage,
repository/tool/profile/permission restoration, Codex thread reuse, or Task
120 second-machine validation.
