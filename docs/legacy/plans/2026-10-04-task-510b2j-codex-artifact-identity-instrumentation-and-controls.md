# Task 510B2J — Codex artifact identity instrumentation and bounded controls

Status: stopped — **G, STILL INDETERMINATE**.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`. Existing dirty
Task 510B–510B2I WIP was preserved. No admission, preferred baseline, cleanup,
production commit, push, tag, release, or CI action was performed. Codex
0.160.0 was not certified.

## Instrumentation

`crates/rah-runtime-codex/src/certification_support.rs` now records supplied
and canonical paths, SHA-256, bytes read, pre/post lengths, creation time,
last-write time, attributes, Windows volume serial, and Windows file ID/index.
The hash streams from the opened read-only handle in 64 KiB chunks. The helper
fails when bytes read differ from the initial length, or when post-read length
or canonical path changes. Windows identity comes from the hashed handle. The
existing sanitized mismatch path and exact expected SHA are unchanged.

Focused deterministic validation: 1 passed, 0 failed for the fixture test,
covering complete bytes, known SHA, identity capture, repeated identity/SHA,
and supplied/canonical path association. The new helper was individually
rustfmt-formatted. `git diff --check` passed. Full `cargo fmt --check` remains
not green because the pre-existing untracked Desktop certification test is
also unformatted.

## Controls

Candidate path:
`C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`.

Expected SHA:
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.
Historical unexpected SHA:
`3e4520220a642fbd7c80dad7c99579afb5bb515b83fd2a0127d47b811d7f8f91`.

Ten sequential idle records all had SHA
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`, bytes
read/pre-length/post-length `326872368`, creation
`2026-10-02T11:18:50.0330586Z`, last write
`2026-10-02T11:18:58.4901097Z`, attributes `Archive`, and file ID
`0x0000000000000000001d00000000b96d`. Canonical and supplied paths matched.
The first and last independent PowerShell hashes equaled the expected SHA.

Two exact version runs returned `codex-cli 0.160.0` and exit 0. PIDs were
15620 and 18692; no descendant remained. A direct exact-candidate
`app-server --stdio` initialize handshake succeeded with PID 18776, started at
`2026-10-04T11:39:30.1668356Z`; shutdown left no descendant. Inference was 0
and Tool execution was 0. Pre/post SHA, length, write time, and file ID were
unchanged. This was direct process evidence; the full `rah-runtime-codex`
composition gate was not run because the current harness is coupled to
forbidden temporary-root cleanup.

The Desktop composition control was not run, and no cleanup was run. Thus the
all-scopes identity result cannot be established here.

## Writer and classification

The only plausibly related process observed was `node.exe` PID 3928 at
`C:\Program Files\nodejs\node.exe`. No npm updater or installer subprocess
was identified, and no writer was proven. Defender or an indexer is not claimed
as a writer. Process evidence is correlation only.

No mismatch occurred during these controls. The historical mismatch remains
without bytes-read, file-ID, or writer evidence. A/B/C/D are not established;
E cannot be claimed because Desktop did not run; F is not established because
no new runtime/identity failure occurred.

Required classification: **G — STILL INDETERMINATE**.

The live npm path remains untrusted. Recommend the next task implement an
isolated exact-byte snapshot boundary:

`live npm artifact → exact expected hash verification → exact-byte snapshot copy
→ snapshot hash verification → certification executes snapshot only`.

Production admission remains exact `codex-cli 0.157.1`; the preferred baseline
is unchanged. ADR-B applies; ADR 0030 and Task 498 sanitization are unchanged.
Inference count was 0 and Tool count was 0.

Next task: implement and separately validate the isolated snapshot-copy
certification boundary without changing production admission.

Reference only: Task 510B2K attempted the isolated snapshot boundary and stopped at its first focused compilation failure, classification G. See [Task 510B2K](2026-10-04-task-510b2k-exact-byte-codex-certification-snapshot.md). Task 510B2J evidence and classification remain unchanged.
