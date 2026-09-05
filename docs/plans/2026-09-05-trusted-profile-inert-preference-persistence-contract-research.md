# Task 213 — Trusted Profile Inert Preference Persistence Contract Research

## Result

Recommend **Option B: remembered path plus an explicit Restore action**.

Desktop may remember one host-selected Trusted Profile source path, but startup
must load only the bounded preference record. It must not open the profile
source, statically validate it, construct a `DesktopTrustedProfileSelection`,
construct a `TrustedStaticProfile`, compose providers, publish an effective
registry, advertise runtime Tools, or start a provider process.

The explicit host sequence is:

```text
remembered path on disk
  -> startup displays Remembered — not restored
  -> host clicks Restore
  -> current source is statically validated, without provider spawn
  -> successful result becomes selected/configured inert intent
  -> host clicks Connect
  -> current source is loaded again and providers are freshly composed
  -> only successful composition is effective/advertised/current
```

`Restore` and `Connect` are separate host actions. A remembered path is a
location preference, not a selected profile, and a selected profile is not an
effective provider composition. This is the smallest interpretation of Task
212's “inert Trusted Profile path persistence and explicit restore” language
that does not create an automatic authority transition.

The result confirms Task 212: this milestone introduces no new authority class
and does not require a new ADR. ADR 0011 remains sufficient because the
persisted value is only an input to the existing host-owned explicit profile
selection/composition boundary.

## Authoritative starting point

The verified starting state was:

- `HEAD` and `origin/master`: `005a16bc5664f9fde5cd4036bd2acbe55afe4062`;
- clean worktree before research;
- v0.17.0 released;
- 13 workspace packages, Rust edition 2024, package version 0.17.0;
- Task 212 exact-head CI reported PASS as supplied by the task; and
- no Rust, Cargo manifest/lockfile, version, provider, runtime, or production
  behavior changes are authorized by this task.

The relevant source and decisions inspected were:

- `crates/rah-desktop/src/desktop_preferences.rs`;
- `crates/rah-desktop/src/main.rs`;
- `crates/rah-desktop/src/trusted_profile_selection.rs`;
- `crates/rah-desktop/src/provider_composition.rs`;
- `crates/rah-desktop/src/effective_authority.rs`;
- `crates/rah-desktop/frontend/index.html` and `status.js`;
- `crates/rah-tools/src/trusted_profile.rs` and
  `trusted_profile_source.rs`;
- `crates/rah-profile-composition/src/lib.rs`;
- the Desktop preference tests and Task 204–206 provider/effective-authority
  tests;
- `docs/adr/0011-trusted-capability-profile-authority-boundary.md`;
- `docs/ARCHITECTURE.md`, `docs/ARCHITECTURE_GUARDRAILS.md`, and
  `docs/SECURITY.md`; and
- `docs/plans/2026-09-05-v0.18-scope-and-authority-roadmap.md` and the v0.17
  composition/audit plans.

## Current persistence implementation

`crates/rah-desktop/src/desktop_preferences.rs` currently owns private
`desktop-preferences.json` persistence. The implementation has these relevant
properties:

- the bounded file limit is `MAX_BYTES = 4096`;
- startup calls `cleanup_temps`, bounded-reads one regular file, rejects empty,
  oversized, non-UTF-8, and BOM-prefixed input, then parses a closed schema;
- the canonical writer currently emits version 2, a required `model` object,
  and an optional `commit_identity` object;
- v1 is read as model-only; v1 with `commit_identity` is rejected;
- v2 may contain the validated commit identity;
- unknown fields and duplicate fields are rejected by the custom closed-map
  deserializers at every preference object level;
- an invalid existing file is left in place, startup falls back to default
  model/no identity, and one bounded `RestoreFailed` warning is retained;
- model save and identity save use the same atomic machinery and preserve the
  other currently loaded preference values; and
- a failed save returns `SaveFailed` and does not replace the destination.

The existing writer creates an owned temporary file with `create_new`, writes
the complete byte record, calls `sync_all`, and then atomically moves it into
place. On Windows an existing destination uses `ReplaceFileW`; an
`ERROR_ACCESS_DENIED` result has the existing `MoveFileExW` replacement
fallback. Other replacement failures are not retried. A failed operation
cleans up the temporary file when possible. This is the persistence guarantee
to reuse; it is not a rollback or multi-process transaction guarantee.

The current `Preferences::start` return value contains only model selection;
the loaded commit identity remains in the `Preferences` object. There is no
Trusted Profile field or profile preference API today. The current startup
`DesktopAppState::new` therefore initializes:

```text
model                 = loaded v1/v2 model preference
commit identity       = loaded v2 identity, if any
trusted_profile       = None
trusted_profile_gen   = 0
provider_activation   = None
connection            = NotConnected
```

The current frontend presents a selected profile as “Configured — providers
inactive”, shows a sanitized profile ID and provider/tool counts, and does not
show the source path. `Choose Profile` statically validates and publishes the
process-local selected intent. `Clear Profile` removes that process-local
selection when selection is allowed. Neither current action persists the
source path.

## Selected restore semantics

Option B is selected.

### Why automatic inert configured restore is not selected

Option A would not itself spawn providers, but it would make startup perform a
new source-dependent operation and would make an old path appear to be the
current configured profile before a fresh host action. It would also require
startup to decide whether to validate changed contents, whether to publish
stale configured counts, and how to represent a source that disappeared. That
presentation would be easy to confuse with the process-local selected state
already called “Configured” in v0.17.

The security boundary can technically remain closed under Option A, but the
semantics are less explicit and the startup path becomes coupled to profile
source validation. Task 212 explicitly calls for explicit restore, so that
coupling is not justified.

### Option B contract

At startup:

- parse the preference record and retain a syntactically representable
  `RememberedProfilePath` value only;
- do not read the remembered profile file;
- do not perform `TrustedStaticProfile::load`;
- do not derive profile ID, provider counts, expected Tool inventory, or
  external descriptors;
- do not allocate provider adapters or provider lifecycle ownership;
- do not publish `trusted_profile` selected state or increment its generation;
- do not alter `ConnectionState`, the effective registry, runtime state, or
  dynamic Tool metadata; and
- show a bounded “Remembered — not restored” state with an explicit Restore
  action, if a remembered path exists.

`Restore` is a host-only action. It takes no model input and no provider
metadata. It obtains the remembered path from host state, checks the same
profile-selection lifecycle gate as Choose Profile, and invokes the existing
bounded non-spawning provider-only static loader on the current source. A
successful result replaces the current selected/configured inert intent and
increments the profile generation. It still starts zero provider processes.

If there is no remembered path, Restore is unavailable; the user uses Choose
Profile. Choose Profile and Restore have the same validation and publication
semantics after their respective host-selected path sources are obtained.

If Restore fails, the current selected intent is unchanged, no new generation
is published, no provider is started, and a bounded profile error is returned.
On a fresh startup this leaves the state as remembered-only/inert. A missing,
changed, unreadable, linked, malformed, or otherwise invalid source is not a
connection failure because the user has not requested Connect.

## Persisted schema contract

The canonical persisted schema advances to **version 3**:

```json
{
  "version": 3,
  "model": {
    "provider": "inherit"
  },
  "commit_identity": {
    "name": "RAH Host",
    "email": "rah-host@example.invalid"
  },
  "trusted_profile": {
    "path": "C:\\profiles\\desktop-provider.json"
  }
}
```

The exact closed schema is:

```text
root             = { version: u64, model: Model,
                     commit_identity?: CommitIdentity,
                     trusted_profile?: TrustedProfilePreference }
Model            = { provider: String, model?: String, endpoint?: Endpoint }
Endpoint         = { scheme: String, host: String, port: u16 }
CommitIdentity   = { name: String, email: String }
TrustedProfilePreference = { path: String }
```

The parser must accept exactly these keys and reject unknown or duplicate keys
at root and in every nested object. It must reject a missing required field,
wrong JSON type, `null`, trailing data, malformed UTF-8, BOM input, and a
malformed `trusted_profile` object. The profile object is all-or-nothing: an
invalid path does not get ignored while model or identity is accepted.

There is no profile ID, parsed profile content, capability list, provider list,
permission, executable path, provider identity, Tool inventory, generation,
runtime alias, credential, token, endpoint session, or effective registry in
this schema.

Canonical v3 output preserves the current compact JSON style and final newline,
with root members ordered as `version`, `model`, `commit_identity` when
present, and `trusted_profile` when present. Optional objects are omitted, not
written as `null`. For example, a profile-only addition to the default model
is:

```json
{"version":3,"model":{"provider":"inherit"},"trusted_profile":{"path":"C:\\profiles\\desktop-provider.json"}}
```

followed by one newline. The writer must escape the path as a JSON string; the
stored path is not a second profile format.

The persisted record is deliberately one host preference record. It does not
become an authorization cache merely because the host previously wrote it.

## Path representation and validation

The path is stored as one **absolute, platform-native path string**, with the
exact accepted lexical spelling supplied by the host file picker. It is not
converted to a portable slash format, URI, profile ID, hash, or relative path.

Storage rules:

1. The selected path must already have passed the current
   `load_provider_only_profile`/`TrustedStaticProfile::load` path rules before
   it can be remembered.
2. Persistence performs no `fs::canonicalize` and no case normalization.
   Canonicalization would make a missing source impossible to remember and
   would change the preference rather than merely remembering the user's
   source spelling.
3. The v3 parser checks that the decoded string is representable by the
   platform `PathBuf`, is absolute, has no `.` or `..` components, and meets
   the platform's existing source-prefix rules. It does not require the path
   to exist and does not inspect the source file.
4. Explicit Restore/Select calls the existing static profile loader on the
   current path. That loader remains the authority for regular-file,
   link/reparse, source-size, encoding, schema, resource, and provider-only
   validation. Persistence must not create a second broader acceptance path.
5. Explicit Connect calls the v0.17 activation path, which fresh-loads the
   source again before provider composition. It must not reuse parsed bytes,
   provider descriptors, permissions, or Tool inventory derived by Restore.
6. A path that later disappears is a valid remembered preference record. It is
   not a startup error. Restore reports bounded unavailable/invalid profile
   state and leaves the state inert.

The preference JSON format supports only paths that can be represented as a
Rust `String` and serialized as UTF-8 JSON. Save fails cleanly for an
`OsStr` that cannot be converted losslessly to the existing string format.
This is an explicit v0.18 product limitation; no new byte-encoding scheme is
introduced.

The raw path is bounded to 1024 UTF-8 bytes before JSON escaping, and the
complete canonical preference document must still fit `MAX_BYTES`. The path
limit is a preference-format limit, not a change to Trusted Profile source
acceptance.

## Startup semantics

Startup behavior is defined by the following table:

| Input | Model/identity result | Remembered path | Profile source I/O | Warning |
| --- | --- | --- | --- | --- |
| No file | defaults | none | none | none |
| Valid v1 | v1 model | none | none | none |
| Valid v2 | v2 model/identity | none | none | none |
| Valid v3 without profile | v3 model/identity | none | none | none |
| Valid v3 with profile path | v3 model/identity | path only | none | none |
| New/unknown version | defaults/no identity | none | none | `RestoreFailed` |
| Malformed JSON/trailing data | defaults/no identity | none | none | `RestoreFailed` |
| Oversized/non-regular/unreadable file | defaults/no identity | none | none | `RestoreFailed` |
| BOM/non-UTF-8/empty file | defaults/no identity | none | none | `RestoreFailed` |
| Invalid trusted-profile field | defaults/no identity | none | none | `RestoreFailed` |
| Valid path, source missing or changed | v3 model/identity | path only | no source read | none |

The last row is intentional. Startup validates only the preference
representation. It must not turn a stale path into an error that looks like a
failed connection, and it must not read a provider-bearing profile merely to
decorate the startup screen.

An invalid profile-preference field invalidates the entire closed preferences
document. The parser must not partially accept model or identity while dropping
the invalid profile field. This matches the current fail-closed parser and
prevents a malformed or authority-looking extension from being silently
reinterpreted.

Startup publishes no `Current` profile state, no effective profile generation,
no external unavailable-capability inventory based on the remembered source,
and no runtime advertisement. The only UI state added for the path is
remembered/inert preference state.

## Explicit restore semantics

Restore uses the current bytes at the moment of the host action. Its sequence
is:

```text
check idle/disconnected profile-action gate
  -> read remembered path from host preference state
  -> TrustedStaticProfile::load(path)
  -> enforce Desktop provider-only rule
  -> derive bounded configured presentation from this load
  -> recheck lifecycle gate before publication
  -> publish selected/configured inert intent
  -> increment trusted-profile generation once
```

`TrustedStaticProfile::load` is the existing non-spawning loader. It validates
the source and static schema but does not start MCP or Process Plugin
providers. The returned parsed profile is not retained as the authority input
for Connect; only the existing configured selection representation may retain
the path and sanitized configured metadata.

Restore success does not mean Effective, Advertised, Current, or authorized.
The status is “Configured — providers inactive” after success. A successful
Restore with zero providers is still inert and still requires Connect for a
runtime connection, consistent with the existing explicit boundary.

Restore failure is non-destructive. It does not clear a prior selected intent,
does not replace its generation, does not publish an unavailable effective
registry, and does not spawn or retry a provider. If the source has changed,
the current bytes are either accepted as the new statically validated
configured intent or rejected by the current loader; no previous profile
contents are trusted.

## Connect/reconnect semantics

The v0.17 Connect contract remains unchanged after Restore:

- Connect is the only action that may activate configured MCP or Process
  Plugin providers;
- Connect fresh-loads the exact selected source using the current file;
- provider permissions and descriptors are derived from that fresh profile;
- the shared composer constructs a fresh effective provider composition and
  owns its provider lifetimes;
- first-party and external registries are merged only after all admission
  succeeds;
- the runtime receives only the fresh final registry;
- effective state is published atomically only after runtime and host
  currentness checks succeed; and
- any failure shuts down staged providers/runtime through the existing cleanup
  path and publishes no partial replacement.

Restore metadata, old profile bytes, old Tool inventory, old permissions,
provider identity, or old profile schema must never be passed through as an
activation shortcut. If the profile file changes between Restore and
Connect, Connect reads and validates the new bytes. If that fresh load or
provider admission fails, Connect fails closed and the old effective provider
composition is not revived as the new one.

Reconnect remains explicit. It creates a new connection generation and a fresh
composition; it is not a provider restart inferred from preference state.
Disconnect stops the connected runtime and owned providers according to the
existing lifecycle contract but leaves the selected inert intent and
remembered preference unchanged unless the user separately clears or forgets
them.

## Generation/currentness contract

The profile generation represents the current process-local selected/configured
profile intent. It does not represent remembered preference state and it does
not represent provider contents.

| Action | Selected intent | Profile generation | Effective/advertised effect |
| --- | --- | --- | --- |
| Startup, no remembered path | none | 0 | none |
| Startup, remembered path only | none | 0 | none |
| Restore succeeds | replace/set | increment once | none; Connect required |
| Restore fails | unchanged | unchanged | none from failed restore |
| Choose a different path succeeds | replace/set | increment once | none; reconnect/Connect required |
| Choose same path succeeds | replace metadata from current bytes | increment once | none; fresh activation required |
| Choose/Restore static validation fails | unchanged | unchanged | no publication |
| Clear Current Selection | none | increment if selected | current active connection is not touched; action is disconnected-only |
| Forget Remembered Profile | unchanged | unchanged | no runtime/effective effect |
| Connect succeeds | unchanged | unchanged | new connection generation; fresh effective publication |
| Connect fails | unchanged | unchanged | no new effective publication |
| Reconnect | unchanged | unchanged | new connection generation and fresh composition |
| Disconnect | unchanged | unchanged | current runtime/provider composition is withdrawn |
| Repository switch | unchanged | unchanged | repository generation changes; reconnect required as today |
| Model switch | unchanged | unchanged | model generation changes; reconnect required as today |

Restore of the same lexical path always revalidates and replaces the
process-local configured metadata, so it increments the profile generation even
when the string is unchanged. This makes the explicit host action a clear
fresh-selection boundary and prevents an old configured snapshot from being
treated as current.

The effective publication currentness tuple must include the captured profile
generation alongside repository, model, and connection generations:

```text
(repository_generation,
 model_generation,
 profile_generation,
 connection_generation)
```

The current v0.17 connect publication already checks the four values at the
async publication boundary, while `ConnectionState::Connected` currently
retains repository/model/connection fields and profile mutation is blocked
while connected. The v0.18 integration must make the profile-generation part
of the retained effective/currentness record as well, or preserve an equally
strong host-owned invariant. It must not rely on the UI disabled state as the
only currentness proof.

A remembered path alone must never manufacture a current profile generation.
A failed Restore must never publish a new generation. Runtime advertisement is
current only for a successful fresh publication whose captured tuple still
matches all current host generations.

## Clear/forget semantics

Recommend two explicitly named host actions rather than one ambiguous Clear:

1. **Forget Remembered Profile** removes only `trusted_profile.path` from the
   durable preference record. It does not clear current selected intent, stop
   providers, disconnect the runtime, alter repository/model state, or change
   profile generation. It may be used while connected when the chat is idle,
   because it is a preference-only operation; the current connection remains
   exactly as published. If the user then disconnects, the selected profile can
   still be selected for a later reconnect in the same process until Clear
   Current Selection is used.
2. **Clear Current Selection** removes only the process-local selected inert
   profile intent and increments profile generation if one was present. It is
   allowed only while chat is idle and the connection is `NotConnected` or
   `Error`, preserving the existing v0.17 selection gate. It does not stop a
   provider because no active connection is permitted during the action.

This distinction makes a preference-only operation visibly different from
clearing configured state. A combined “Clear Profile” button may be offered
only as a host UI convenience that performs Forget followed by Clear Selection
under the same disconnected gate; its documentation must say that it removes
both remembered and selected state. The backend contract remains the two
separate operations so a failed durable write cannot be mistaken for an
in-memory selection transition.

If a Forget save fails, the old durable path remains and the current state is
unchanged; emit the existing bounded `SaveFailed` warning. If Clear Current
Selection has no durable work, it can succeed independently. If a combined UI
operation cannot durably forget the path, it must not claim that both actions
completed.

Neither action is a provider revocation mechanism. Disconnect is the explicit
provider/runtime lifecycle action. No clear/forget operation silently kills a
connected provider.

## Active-connection behavior

Selection and Restore continue to require chat idle and `NotConnected` or
`Error`, as current v0.17 does. The UI must disable them while connected,
connecting, disconnecting, or a chat turn is running. This prevents profile
replacement from becoming hot reload.

Forget Remembered Profile is allowed while connected only because it changes no
selected, effective, advertised, current, or provider state. The UI must say
that the active connection is unchanged. A later reconnect in the same process
still uses the selected process-local source; Forget does not clear it.

If another process or an external actor changes `desktop-preferences.json`, an
active Desktop process does not watch or reload it. Its active provider
children and runtime remain owned by the current connection. The next
explicit Restore/Choose/Connect boundary obtains current host state according
to the command's contract. No preference save triggers hot reload, provider
replacement, automatic reconnect, or automatic shutdown.

Changing model or repository continues to invalidate the applicable existing
generation and require reconnect. It does not clear or rewrite the profile
preference. Profile selection remains separate from Desktop repository
canonicalization and cannot select a repository resource.

## Effective Authority presentation

The minimum unambiguous presentation states are:

| State | Recommended wording | Show profile ID/provider counts? | Effective/advertised? |
| --- | --- | --- | --- |
| No preference | `No profile remembered` | no | no |
| Remembered only | `Remembered — not restored` | no | no |
| Restore failed | `Profile could not be restored; choose a profile` | no | no |
| Selected after Restore/Choose | `Configured — providers inactive` | sanitized configured fields only | no |
| Connected but stale | `Configured — reconnect required` | sanitized current configured fields | no |
| Connected/current | `Active` | effective sanitized inventory | yes |
| Disconnected after selection | `Configured — providers inactive` | sanitized configured fields | no |

Remembered-only state must not be rendered as “Configured”, “Effective”,
“Active”, “Current”, or “Advertised”. In particular, no external unavailable
Tool entries should be derived from a remembered-only path because that would
require reading and interpreting profile contents at startup.

The existing Effective Authority snapshot remains informational and continues
to show only host-composed sanitized state. A remembered path is not an
effective authority descriptor. `Refresh Authority` remains read-only and must
not Restore, statically validate, Connect, reload, or alter generations.

## Privacy/redaction

The path necessarily appears in the private local preference file. Release and
user documentation should state that Desktop stores the selected Trusted
Profile source path locally in `desktop-preferences.json`.

Normal UI should not display the full path by default. It should display only
the remembered/restored state and existing sanitized profile ID/counts after
explicit Restore. If a future UX needs a source label, it must use a bounded
host-owned redacted form and must not make the path an authority descriptor.

Raw profile paths must not appear in:

- Effective Authority DTOs or external Tool descriptors;
- dynamic Tool definitions or provider-private aliases;
- normal live evidence or activity messages;
- model prompts, Tool metadata, or conversation records;
- ordinary frontend error strings; or
- logs at normal warning level.

Diagnostics may retain the existing sanitized closed error categories. A
developer-only diagnostic can be considered separately, but Task 214 must not
add path logging as part of persistence.

## Model visibility

The remembered path is host-only and is not a Tool. It must not:

- be queryable by the model;
- be selectable, restored, forgotten, or cleared by a model request;
- be inserted into prompts or transcript messages;
- be included in `dynamicTools` metadata;
- be added to `ToolRegistry`;
- be included in Effective Authority external descriptors; or
- be changed by MCP/Process Plugin metadata, Tool descriptions, or provider
  output.

The current Tauri command surface exposes profile selection only through
frontend host commands. Task 214/215 must preserve that host-only routing and
must not add a model-facing command or generic command bridge. Any discovery
that a model-visible command can reach preference mutation is a defect that
must be fixed or blocked before implementation proceeds.

## Conversation/repository interaction

Trusted Profile preference persistence is separate from conversation
persistence. It remains in the app-owned `desktop-preferences.json` record,
not in the repository-scoped SQLite transcript namespace.

Conversation records must not gain a profile path merely because a conversation
used a profile. Resume must not select, Restore, reconnect, or activate a
profile. Resume remains explicit replay into the currently connected model
context and does not restore repository, model, Tool, provider, commit, index,
or profile authority.

The remembered profile path is a **global Desktop preference**, not a
repository-scoped or conversation-scoped value. It follows the app-owned
Desktop preference directory. Changing the selected repository only changes
the host repository context and its generation; it does not migrate, copy,
resolve, or reinterpret the profile path. A profile may contain resources
related to a repository, but the existing host repository selection and
canonicalization rules remain authoritative.

Moving a home/profile directory or using a different Desktop preference
directory may leave a global remembered path pointing at an unavailable
source. That is a stale preference, not an implicit migration or repository
selection event. The user uses Choose Profile or Forget.

## Atomic persistence semantics

Profile preference writes must use the existing `Preferences` writer and its
bounded test fault seams. No direct destination write, truncate-in-place, or
second persistence mechanism is permitted.

For every profile save/forget operation:

- construct the complete v3 record in memory, preserving model and identity;
- reject it before any filesystem write if validation or the 4096-byte limit
  fails;
- skip the write if the complete bytes equal the current bounded destination;
- create an owned temporary file with `create_new`;
- write all bytes and `sync_all` the temporary file;
- atomically move/replace it with the existing platform machinery;
- remove the temporary file after a failed replacement when possible; and
- report `SaveFailed` without claiming the durable update succeeded if any
  step fails.

Create failure, write failure, sync failure, replacement failure, and cleanup
failure must not silently replace or truncate a previously valid destination.
The existing Windows access-denied replacement fallback remains the only
documented fallback. A process crash can leave a temporary file or leave the
old destination; startup cleanup removes only names owned by this preference
writer. No rollback guarantee beyond the existing atomic persistence contract
is made.

The implementation must preserve current model/identity update behavior:

- model saves preserve `trusted_profile`;
- identity saves preserve `trusted_profile`;
- profile saves preserve model and identity;
- Forget preserves model and identity;
- Clear Current Selection has no durable effect unless the host also invokes
  Forget; and
- Reset Model Preferences resets only model state and preserves the remembered
  profile and commit identity unless a separately documented product action
  explicitly says otherwise. The selected recommendation is preservation.

Concurrent Desktop processes remain **unsupported for semantic coordination**
and follow the existing last-writer-wins behavior. Atomic replacement prevents
partial JSON bytes, but two read-modify-write operations can lose one another's
preference field. v0.18 does not add an interprocess lock because the file is
not an authorization store and the same bounded behavior already applies to
model/identity preferences. A future lock/versioning design is separate.

## Backward compatibility / schema migration

v1 and v2 remain readable with their current semantics. Their absence of
`trusted_profile` means “no remembered profile”. No startup migration or
canonical rewrite is performed merely by reading them.

The first successful canonical write through any preference mutation emits v3.
That includes model save, identity save, profile save, Forget, or a reset that
writes preferences. A v1/v2 file therefore upgrades only after a successful
write, and the write preserves every preference value that the caller did not
intend to change.

Version handling is closed:

- v1: model required, identity forbidden, profile forbidden;
- v2: model required, optional identity, profile forbidden;
- v3: model required, optional identity, optional profile; and
- every other version: reject the entire file with `RestoreFailed`.

An old file that contains an unknown profile-looking field must fail closed as
an unknown field; it must not be accepted as v1/v2 and must not have the field
dropped. A v3 profile object with any malformed field invalidates the entire
file, including otherwise valid model and identity values.

## Windows considerations

The current Desktop Trusted Profile path is Windows-only (`cfg(target_os =
"windows")`), and the current source validator is the authority for Windows
acceptance. It requires an absolute drive-rooted path and rejects the source
forms covered by existing tests:

- relative paths;
- UNC paths such as `\\server\share\profile.json`;
- verbatim/device prefixes such as `\\?\C:\profile.json`;
- alternate data stream syntax such as `C:\profile.json:ads`;
- lexical `.` and `..` aliases; and
- files or parent components that are symlinks, junctions, or Windows reparse
  points.

Persistence must not broaden any of these rules. Drive-letter paths are stored
in native Windows spelling and JSON-escaped. Case is not normalized; a
case-only spelling change is a new remembered string and is revalidated by the
host loader. Verbatim/device and UNC paths remain rejected even if they would
make long-path persistence easier.

The existing Windows source validation checks path topology before opening and
again around the open, and detects links/reparse points. The existing source
identity/race limitations remain; Task 213 must not claim perfect ACL or
race-free identity. Restore and Connect reuse the same loader rather than
adding a weaker preference-specific open.

Windows paths that cannot be losslessly represented by the existing UTF-8 JSON
string format are unsupported for v0.18 persistence and fail Save. This does
not change whether a directly supplied path might be representable to another
host API; it only defines the persistence format's supported subset.

## Linux considerations

The current Desktop profile selection and provider composition path is gated to
Windows, so Linux is a contract/test target for the persistence format rather
than a claim of Linux Desktop provider certification.

On Unix, the persisted path must be absolute, have no `.` or `..` components,
and be representable as UTF-8 in the existing JSON format. Arbitrary Unix
`OsStr` byte paths that are not valid UTF-8 are not persisted in v0.18. No
surrogate/byte escape scheme is added without a separate product decision.

Symlinks are not made valid by persistence. Explicit Restore and Connect use
the current source validator, which rejects linked source components and uses
the existing no-follow/opened-identity checks where implemented. A missing
file is retained as a remembered string at startup and rejected only when the
host explicitly restores or connects.

No Unix `fs::canonicalize` is performed for storage. Home-directory moves,
mount changes, and stale paths produce an inert remembered preference until the
user restores a currently valid source or chooses another. Deterministic Unix
tests should cover absolute/relative paths, UTF-8 limitation, missing source,
symlink/replacement behavior, malformed preference records, and no provider
spawn. They must not be described as Linux live Codex/provider certification.

## Threat model

| Threat/event | Required fail-closed behavior |
| --- | --- |
| 1. Stale remembered path | Startup keeps only inert path text. Restore/Connect use current validation; no provider activation from stale state. |
| 2. Profile contents replaced | No contents are persisted. Restore reads current bytes; Connect reads current bytes again. Old descriptors/permissions are discarded at the activation boundary. |
| 3. Path target deleted | Startup remains non-error and non-spawning. Explicit Restore/Connect returns bounded unavailable/invalid profile failure; no generation/effective publication from failure. |
| 4. Path changed to symlink/reparse point | Existing source validation rejects it; no static selection publication, provider spawn, or effective registry. |
| 5. Attacker modifies `desktop-preferences.json` | The file is not trusted authority. Closed parsing, size/type/UTF-8/BOM checks, absolute-path syntax checks, and later source validation fail closed. A valid attacker-written path still cannot activate until explicit host Restore/Connect and all existing policy checks pass. |
| 6. Attacker changes profile between Restore and Connect | Restore metadata is not reused as authority. Connect reloads/revalidates and freshly composes; failure publishes no new connection. Existing connected state is not hot-reloaded. |
| 7. Preference write interrupted | Atomic writer leaves the last complete destination or a recoverable temp; failed update reports SaveFailed. No partial JSON is accepted as a successful update. |
| 8. UI presents remembered state as Effective | The contract forbids profile ID/counts/effective inventory at remembered-only startup and requires explicit “Remembered — not restored” wording. |
| 9. Connect uses cached old profile bytes | Connect must call the existing fresh activation load. Tests must replace source contents between Restore and Connect and assert new behavior or fail closed. |
| 10. Model/provider tries to influence preference | No model-visible command or metadata path exists. Only host commands and the closed host writer can mutate the preference; provider metadata cannot grant or change it. |

These controls do not claim OS sandboxing, network isolation, perfect source
identity, rollback, or absence of ambient provider effects. An uncertain
provider effect is not replayed or represented as rolled back.

## Deterministic test matrix

Task 214+ must add tests at the narrowest applicable layer. Tests must use
temporary app-owned directories and existing provider fixtures/spawn counters;
they must not require paid credentials, a live model, internet, or GPU.

### Persistence

- save one valid absolute profile path;
- simulate restart and read the same remembered path;
- read valid v1 model-only input;
- read valid v2 model/identity input;
- verify first successful mutation canonicalizes v1/v2 input to v3;
- model save preserves the remembered profile path;
- identity save preserves the remembered profile path;
- profile save preserves model and identity;
- Forget preserves model and identity;
- Clear Current Selection does not erase the remembered path unless Forget is
  explicitly called;
- Reset Model Preferences preserves the profile preference;
- path at the 1024-byte raw UTF-8 limit is accepted when the full record fits;
- oversized path or full record fails without replacing the valid destination;
- malformed path/schema fails closed;
- duplicate root, profile, model, endpoint, and identity fields are rejected;
- unknown root/profile fields are rejected;
- null, trailing data, BOM, invalid UTF-8, oversized, and unknown-version
  inputs are rejected as whole-file failures;
- create/write/sync/replace/fallback failure leaves the old destination intact;
- failed replacement cleans owned temp files where possible; and
- concurrent writer behavior is documented as atomic last-writer-wins, not
  asserted as a merge.

### Startup

- no remembered path produces no profile state and generation 0;
- remembered path startup produces only remembered/inert state and generation
  0;
- startup does not read the profile source;
- startup does not spawn a provider or perform provider handshake;
- startup does not construct/publish an effective registry;
- startup does not advertise an external Tool;
- startup does not manufacture `Current` external provider state;
- valid v1/v2 input retains existing model/identity behavior;
- invalid preference input retains the old file, defaults model/identity, and
  emits one bounded RestoreFailed warning; and
- valid remembered-but-missing or changed source causes no startup warning and
  no connection failure because source I/O is deferred.

### Restore/Select

- Restore reads current profile bytes, not a cached profile object;
- a valid Restore becomes selected/configured inert intent only;
- Restore performs zero provider spawn and zero provider handshake;
- Restore derives only bounded configured presentation;
- missing, stale, linked, malformed, or invalid source remains inert;
- failed Restore leaves selection and generation unchanged;
- successful Restore increments generation once;
- same-path Restore revalidates and increments generation once;
- different-path Select replaces selection only after static validation;
- selection is rejected while connected, connecting, disconnecting, or a chat
  turn is active; and
- no Restore/Select action changes model, repository, conversation, or
  provider lifecycle state.

### Connect/reconnect

- Connect after Restore fresh-loads the current source;
- source replacement between Restore and Connect is observed;
- Connect activates providers only after explicit host invocation;
- activation failure is all-or-nothing and reaps staged providers;
- no cached provider authority survives a profile source replacement;
- profile-generation mismatch rejects stale async publication;
- reconnect gets a new connection generation and fresh provider composition;
- Disconnect reaps the published provider composition and leaves selected/
  remembered preference state as specified; and
- no preference read/save/forget operation starts or stops providers.

### Clear/Forget and active connection

- Forget removes only the durable path and preserves current selected state;
- Forget failure leaves the durable path and current state intact;
- Clear Current Selection increments profile generation only when selected;
- Clear Current Selection is disconnected-only and does not stop providers;
- a combined UI action cannot claim success if Forget failed;
- Forget while connected leaves runtime, effective registry, advertisement, and
  children unchanged; and
- selection/Restore/Clear remain unavailable while connected, preserving
  reconnect-required behavior instead of hot reload.

### Generation/currentness

- remembered-only startup does not create a false Current state;
- failed Restore does not create a generation;
- same-path successful Restore creates a generation;
- profile change makes a pending connection publication stale;
- repository/model/profile/connection generation mismatches are not Current;
- stale runtime advertisement is presented as stale/reconnect-required; and
- reconnect is required before a changed selected profile can become effective.

### Privacy/model boundary

- raw path is absent from Trusted Profile presentation and Dynamic Tool
  metadata;
- raw path is absent from Effective Authority external descriptors;
- raw path is absent from normal live evidence and bounded warnings;
- private aliases remain internal;
- path is absent from prompts, transcript rows, and conversation Resume; and
- no model/provider command can mutate or inspect profile preference state.

### Cross-platform

- Windows drive-letter absolute path round trip;
- Windows UNC, verbatim/device, ADS, relative, lexical-alias, and
  link/reparse source cases remain rejected by the existing loader;
- Windows UTF-8 representability and 4096-byte failures are clean;
- Unix absolute path and relative-path rejection;
- Unix non-UTF-8 path limitation;
- symlink/reparse source rejection where supported;
- missing source retained at startup and rejected at explicit Restore; and
- deterministic Linux persistence/no-spawn tests without a Linux live-provider
  claim.

## Live-validation plan

No model/provider live execution is required for this milestone. The useful
manual host/GUI proof is a lifecycle check of the explicit boundaries:

1. Start Desktop with a valid provider-only Trusted Profile and no connection.
2. Choose it and persist the path.
3. Close Desktop and verify the provider child is not running.
4. Restart Desktop and observe `Remembered — not restored`.
5. Verify no provider spawn, handshake, external Effective Authority, or
   runtime Tool advertisement occurred during startup.
6. Click Restore and verify static/non-spawning validation only.
7. Click Connect and verify that provider activation begins only now.
8. Verify fresh effective/advertised/current state after successful connection.
9. Disconnect and verify provider child cleanup/reaping.
10. Repeat with a changed, missing, or replaced profile source and verify the
    failure remains inert until the explicit action that encounters it.

This is host/GUI lifecycle evidence, not model Tool execution certification.
It must not reopen Task 207's external model-selection limitation. Any live
provider lifecycle evidence remains subject to the existing hidden nonce and
complete lifecycle gate.

## Authority / ADR decision

**No new authority class. No new ADR.**

ADR 0011 remains sufficient because it defines the Trusted Profile as a
host-owned composition boundary for existing approved capabilities and
providers, with explicit host selection, static validation, capability-specific
policy, fresh composition, and atomic publication. This task stores only the
source locator that may later be supplied to that existing boundary.

The file is not an authorization store. It does not grant a permission,
select a repository, authorize a Tool call, activate a provider, or revoke an
active composition. Existing repository observation, bounded worktree/file/
directory mutation, index mutation, reviewed commit, local provider, and
runtime authorities remain separate.

If implementation pressure would require automatic composition, automatic
reconnect, hot reload/revocation, provider installation, credential/session
persistence, model-facing profile selection, or profile-controlled repository
selection, implementation must stop and request a separate authority/security
decision and ADR review. Those behaviors are outside Task 213.

## Implementation boundaries

Task 213 itself makes no implementation changes beyond this research plan.

The implementation must keep these layers separate:

```text
JSON remembered path
  -> remembered host preference
  -> explicit Restore/Choose static selection
  -> selected/configured inert intent
  -> explicit Connect fresh composition
  -> Effective registry
  -> runtime advertisement
  -> per-operation ToolRegistry/policy authorization
```

No layer may skip to a later state based solely on data from an earlier state.
In particular:

- `Remembered` is not `Selected`;
- `Selected` is not `Statically Validated` after source mutation;
- `Configured` is not `Effective`;
- `Effective` is not `Advertised`;
- `Advertised` is not `Current`; and
- `Current` is not per-operation authorization.

## Proposed Task 214 scope

**Task 214 — Persist Inert Trusted Profile Preference** should be narrow:

- extend the private closed preference data model to v3;
- add the optional `trusted_profile.path` field with the exact bounds and
  platform representation rules above;
- preserve strict duplicate/unknown-field and whole-file fail-closed parsing;
- preserve valid v1/v2 reads and perform v3 canonical writes only after a
  successful mutation;
- add host-owned read/save/forget operations that preserve unrelated model and
  identity preferences;
- retain remembered path on startup without reading or validating the profile
  source;
- provide the inert remembered state needed by the later explicit Restore
  integration; and
- add deterministic persistence, migration, size, warning, and atomic-failure
  tests.

Task 214 must not:

- activate or reconnect providers;
- change the shared profile composer;
- alter Connect's fresh source reload;
- add profile hot reload or a watcher;
- add model-visible commands or metadata;
- change repository selection/canonicalization;
- store profile contents, permissions, executables, Tool inventory, or
  authority/generation records; or
- redesign the Effective Authority panel.

The explicit Restore command's integration with the process-local selected
profile, profile generation, stale-source presentation, and existing
selection/Connect gates should be completed in the separately sequenced
Task 215/216 work, using this document as the security contract. If Task 214
must expose an API for Restore, that API must remain static, non-spawning,
host-only, and must not alter effective/provider state.

## Explicit non-goals

This research does not authorize:

- automatic inert configured restore at startup;
- startup profile source reads or static source validation;
- automatic provider activation, reconnect, restart, or shutdown;
- profile contents, Tool inventory, permissions, executable identities,
  credentials, tokens, endpoint sessions, runtime aliases, or authority cache
  persistence;
- profile watching, hot reload, dynamic authority replacement, or revocation;
- profile-controlled repository selection or repository-scoped migration;
- conversation/profile coupling or authority restoration through Resume;
- model-facing profile selection, Restore, Forget, or Clear;
- network MCP, Streamable HTTP, PluginManager installation/update, generic
  shell/process/filesystem/Git authority, OS sandboxing, network isolation,
  rollback, or replay; or
- starting Task 214 or any later task automatically.
