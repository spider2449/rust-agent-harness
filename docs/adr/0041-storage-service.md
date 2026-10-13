# ADR 0041: The `storage` Service

Status: Accepted

Date: 2026-10-13

## Context

The pivot plan (section 4) lists a `storage` service (`KvStore`,
`BlobStore`) provided by `storage-fs`, and the plugin-gaps plan deferred it
until a second plugin needed persistent state of its own. That is now the
case:

- `context-default` keeps memory notes in Markdown files.
- `context-compact` (G2) kept summaries in its own directory (`dir`), with
  its own file naming and atomic-write code.
- Process plugins could write files themselves, but WASM plugins have no
  WASI and so no way at all to keep state between calls or restarts.

Every plugin that persists something writes its own file code, chooses its
own directory and cannot be pointed at another backend.

## Decision

1. **`rah-api::storage`.** `STORAGE: ServiceKey<dyn KvStore>` with
   `get`, `put` (replaces atomically: a reader sees the old or the new
   value), `delete` (returns whether the key existed) and `list` (keys with
   a prefix, sorted). Values are bytes. Every call names a namespace.
   Namespaces and keys are 1 to 128 bytes of ASCII letters, digits and
   `._:-` (the plugin-id alphabet), case-sensitive; `check_name` and
   `check_prefix` enforce it, and a provider may refuse a name it cannot
   store. Errors are `StorageError` (`InvalidName`, `TooLarge`,
   `UnknownMethod`, `InvalidParams`, `Io`). `MemoryStore` is an in-memory
   implementation for tests.

2. **Namespaces.** A plugin keeps its entries in the namespace of its
   plugin id. In-process plugins are trusted code (mounting is trust) and
   pass the namespace themselves; that is a convention, not a boundary.
   Process and WASM plugins get `storage` only when it is in their
   `requires`, and their host fixes the namespace to the plugin's id, so
   they cannot read or write another plugin's entries.

3. **The out-of-process form.** `rah_api::storage::call_json` is the one
   mapping both hosts use: `get {key}` returns `{value}` (`null` when
   absent), `put {key, value}` stores any JSON value and returns `{}`,
   `delete {key}` returns `{deleted}`, `list {prefix?}` returns `{keys}`.
   The process protocol stays at version 3: `storage` is a new service
   under the existing `service/call`, and a plugin that requires it fails
   to start on a host without it, as with any unresolved `requires`.

4. **`storage-fs`** provides the service as one file per entry,
   `<dir>/<namespace>/<key>` (default `dir`: `.rah/storage`). File names
   encode names reversibly and in lowercase only (other bytes as `%xx`,
   Windows device names escaped), so case-sensitive keys work on
   case-insensitive file systems. `put` writes and flushes a temporary file
   and renames it over the entry; leftover temporary files older than an
   hour are removed at start. Values are limited by `max_value_bytes`
   (default 16 MiB). Every built-in preset mounts it.

5. **No `BlobStore`.** No plugin needs streaming or large binary values;
   `KvStore` with byte values covers the current consumers. A blob service
   is a separate contract when one does.

6. **Consumers.** `context-compact` keeps summaries in `storage` when the
   service is mounted (resolved on every use, so it is optional) and drops
   its `dir` option. `context-default` memory notes stay Markdown files:
   they are meant to be read and edited by hand, and `storage-fs` file
   names are not.

## Consequences

- `context-compact` configurations with `dir` fail to mount (unknown
  field). Summaries cached in an old `dir` are not migrated; they are a
  cache and are rebuilt on demand.
- A distribution without a `storage` provider still runs `context-compact`,
  with an in-memory cache only.
- Storage is plugin state, not a session log: it is not replayed, forked or
  shown in the Web UI. Nothing in it can grant permission; a plugin that
  reads back what a model produced (such as a summary) still treats it as
  untrusted.
- Several stores on one directory (two `rah` processes started in the same
  folder, two `storage-fs` instances, or a restart while the old instance
  still writes) are safe per call: temporary files carry the process id
  and a random per-store tag and are created exclusively, so every `put`
  replaces the entry whole and concurrent `put`s to one key end with one of
  the values (last writer wins), never a mix.
- The contract has no compare-and-swap, so a read-modify-write (`get`, then
  `put`) is not atomic, within one process or across several: concurrent
  updates of one key can lose one. Current consumers do not need it
  (`context-compact` writes content-addressed keys whose value for a key is
  interchangeable). A plugin that needs atomic updates must serialize them
  itself, and across processes that needs a conditional `put` added to the
  contract first.
