# RAH v0.10 Release Gate

Release status: **RELEASE PREPARATION — NOT YET RELEASED**

## Candidate identity

- Candidate version: `0.10.0`.
- Audited starting checkpoint: `e57187afa831545888aff2418cb9e5d3668bab72`.
- Release-preparation commit: Task 129 candidate commit pending; exact SHA is
  recorded in the Task 129 completion report.
- Certified Codex baseline: exactly `codex-cli 0.149.0`.
- Verified live platform: Windows.
- Tag: **not yet created**.
- GitHub Release: **not yet published**.

## Deterministic release checklist

- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace -j 1`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] `git diff --check`
- [x] `cargo metadata --no-deps --format-version 1` confirms all 12 RAH
  workspace packages are `0.10.0`.
- [x] `node --check crates/rah-desktop/frontend/status.js`
- [x] Desktop release build: `target\\release\\rah-desktop.exe`, 16,711,168
  bytes, `2026-08-29 18:02:47 +08:00`, no adjacent `sqlite3.dll`.
- [ ] Exact-head deterministic-validation CI after push.

## Windows live evidence inventory

The following prior milestone evidence is reused; Task 129 does not represent
it as a fresh rerun:

- [x] Certified Codex Desktop connection at `codex-cli 0.149.0`.
- [x] Native Git executable discovery and selected-repository Git observation.
- [x] Selected-repository runtime CWD binding and launch-CWD/`AGENTS.md`
  isolation.
- [x] Inactive model preference persistence.
- [x] Repository A/B transcript isolation, Resume/restart behavior, true V3 to
  SQLite migration, and SQLite new-write persistence.
- [x] llama.cpp loopback readiness and selected remote routing/disclosure.
- [ ] Task 120 remote llama.cpp successful generation: **DEFERRED / NOT
  VALIDATED**.

Transport confinement is **NOT CLAIMED**. The bounded initial endpoint does not
prove direct routing, proxy avoidance, redirect confinement, DNS integrity, or
peer identity.

## Dependency and distribution record

- Desktop endpoint readiness uses `reqwest 0.13.4`; it is a bounded host-private
  `/health` request, not a generic HTTP API.
- `sha2` derives opaque repository persistence namespaces.
- `rusqlite 0.37.0` with `bundled` uses `libsqlite3-sys 0.35.0`; SQLite is
  bundled/static for the Desktop release. No external `sqlite3.dll` or SQLite
  installation is required.
- `windows-sys` uses the Registry feature for closed Git for Windows discovery.
- There is no async DB framework, connection pool, generic SQL exposure, or new
  architectural RAH crate dependency direction.

## SQLite release contract

- Database: `conversation-transcript.sqlite3`; schema version: `1`.
- Tables: `schema_metadata`, `namespaces`, `epochs`, `pairs`.
- PRAGMAs: `foreign_keys=ON`, `journal_mode=DELETE`, `synchronous=FULL`, and
  `busy_timeout=250 ms`.
- Namespaces: `repo-sha256:<canonical-root-hash>` and `neutral-v1`.
- Storage bounds: `MAX_NAMESPACES=64`, `MAX_BYTES=256 KiB`, legacy V3 JSON
  import input only, `MAX_MESSAGE_BYTES=16 KiB`, `MAX_RECORDS=79`,
  `MAX_PAIRS=64`, and `MAX_EPOCHS=16` per namespace.
- Replay remains separately bounded at `MAX_CONVERSATION_REPLAY_MESSAGES=8` and
  `MAX_CONVERSATION_REPLAY_BYTES=32 KiB`.

Migration authority is one-way: absent DB plus valid V3 migrates
transactionally; after COMMIT SQLite is immediately authoritative, including if
archiving V3 fails. Valid SQLite beats stale V3. Corrupt authoritative SQLite is
quarantined and fails closed; it never falls back to V3. V1/V2 are never guessed
into repository or neutral ownership. There is no dual write or active JSON
backend after SQLite authority.

## Authority and security invariants

The candidate adds no authority beyond ADRs `0001`–`0015`. Model requests remain
non-authoritative; endpoint, executable, CWD, repository, and desired settings
remain host-owned. SQLite is storage, not authority; Desktop UI is
presentation/control, not authority.

Absent: generic shell/process, arbitrary filesystem write, arbitrary SQL,
generic network Tool, network MCP, model-selected executable/CWD/endpoint or
credentials, automatic authority restoration, and Git commit/history/ref
mutation. Process supervision is not OS sandboxing, and uncertain external
effects have no rollback guarantee.

## Known limitations and deferred scope

No llama.cpp process management, provider/model installation, generic network
Tool, network MCP/Streamable HTTP, generic shell/process authority,
model-selected executable/CWD/endpoint, automatic authority restoration, Git
commit/ref/history authority, or generic repository delete/rename authority is
included. Repository move/rename intentionally changes the persistence
namespace. Task 120 remote-generation proof is deferred and transport
confinement is not claimed.

## Task 128A Unix CI recovery record

- First audit commit: `c8964e02e632c363a7fa544aa8f364faae48645c`.
- First recovery: `0b80cee9102a53cc371d853fae0f9264f1d672a3`.
- Final recovery: `e57187afa831545888aff2418cb9e5d3668bab72`.
- Final CI: `33244978381` — PASS.

The Unix executable/non-executable fixture had an under-specified baseline.
Recovery explicitly establishes Git/filesystem mode semantics for both `100755`
and `100644`; production `repo.patch` behavior did not change. This is a test
recovery, not an authority or product-feature change.

## Immutable release procedure

Task 130 alone may create the annotated `v0.10.0` tag and publish the GitHub
Release after this candidate's exact-head deterministic-validation CI is green.
