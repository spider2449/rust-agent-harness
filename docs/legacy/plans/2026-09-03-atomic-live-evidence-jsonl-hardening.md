# Task 175E - Atomic Live-Evidence JSONL Append Hardening

## Scope

This task hardens generic live-evidence JSONL framing only. It does not change
authority, runtime execution, Tool schemas, permissions, repository mutation,
or live validation behavior.

## Triggering observation

Task 175D's substantive repository-context validation passed for repositories A
and B, but its raw evidence stream was malformed. On one physical line, the A
stream contained a `tool_requested` object immediately followed by a
`tool_finished` object whose text result was `TASK175D_SHARED_A`. The B stream
had the same shape with `TASK175D_SHARED_B`. The logical objects were valid,
but the physical JSONL records were concatenated as `}{`.

Task 175D substantive repository-context validation: **PASS**.

Task 175D formal evidence closure: **BLOCKED** by malformed JSONL framing.

Task 175: **NOT COMPLETE**.

## Root cause

`rah-runtime-codex::bridge::append_live_evidence` and
`rah-desktop::append_live_evidence` independently opened the environment-selected
path in append mode and used separate `writeln!` operations. They had no shared
lock. A producer could write serialized JSON bytes while another producer wrote
its record before the first producer's newline. Windows append mode positioned
each write at the end of the file but did not provide the process-wide complete
record guarantee the code required.

## Fix

Both producers now call `rah_protocol::live_evidence::append`. The shared helper
serializes one record, acquires one process-wide `OnceLock<Mutex<()>>`, appends
the complete JSON bytes and exactly one newline in one protected operation,
flushes, and releases the lock. The environment gate and best-effort error
handling remain unchanged.

## Security and authority impact

There is no new authority or authority broadening. There is no Tool schema,
permission, replay, repository mutation, Trusted Profile, or runtime provider
authority change. No evidence lock is held across Tool execution.

## Privacy

Absolute repository paths, prompts, tokens, and raw backend errors remain
excluded. Opaque repository fingerprints remain correlation-only and are not
authority tokens.

## Regression coverage

Deterministic tests cover sequential JSONL framing, two synchronized concurrent
producers sharing the append mechanism, independently parseable physical lines,
text results, and environment gating. Existing bridge tests continue to cover
structured JSON results, bounded request capture, and per-tool advertisement.

## Historical evidence preservation

The malformed Task 175D evidence remains historical truth and is not rewritten.
Task 175E fixes the framing prospectively; it does not retroactively repair old
evidence files.

## Live rerun

No destructive live rerun occurred during Task 175E.
