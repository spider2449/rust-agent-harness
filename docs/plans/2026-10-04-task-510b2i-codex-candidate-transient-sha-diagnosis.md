# Task 510B2I — Codex candidate transient SHA diagnosis

Status: stopped — **G, ARTIFACT IDENTITY INDETERMINATE**.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`. Existing dirty
Task 510B–510B2H WIP was preserved. No production commit, push, tag, release,
version, admission, or preferred-baseline change was made.

Candidate path for both measurements:
`C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`.
Expected version: `0.160.0`; size: `326872368`.
Expected SHA-256:
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.

The full unexpected SHA recovered from Task 510B2H is
`3e4520220a642fbd7c80dad7c99579afb5bb515b83fd2a0127d47b811d7f8f91`.
The preserved report associates it with the later post-launch measurement of
the path above, after an expected pre-launch hash. It records no timestamp,
size, file ID, bytes-read count, independent hash, or writer snapshot for that
failed instant. The run reached the real adapter, app-server, neutral runtime,
and Desktop composition; cleanup was not reached after the mismatch.

## Audit and controls

`ExactCodexCandidate::verify` in
`crates/rah-runtime-codex/src/certification_support.rs` validates the stored
absolute `candidate.path`, clones that same `PathBuf` into `spawn_blocking`,
opens it with read-only `std::fs::File::open`, and streams 64 KiB reads into
Rust `sha2::Sha256`. The version probe uses the same path and only
`--version`; the factory retains that path. No shadow path or artifact
write/create/truncate access is present. The loop does not record bytes read or
assert bytes read equals file length.

On 2026-10-04, six sequential idle PowerShell `Get-FileHash` measurements all
returned the expected SHA, size `326872368`, creation
`2026-10-02T11:18:50.0330586Z`, last-write `2026-10-02T11:18:58.4901097Z`, and
`Archive` attributes. Independent .NET SHA-256 hashes of the same path agreed
on all six measurements. The current NTFS file ID was
`0x0000000000000000001d00000000b96d`.

The exact candidate `--version` control returned `codex-cli 0.160.0` and
exited successfully. No app-server or Desktop identity control was run. No
cleanup, full validation, or inference ran.

The candidate is a normal file. Its inspected package parent had no
symlink/junction, `.tmp`, staging, or update entry. A pre-existing process set
was present: `node.exe` PID 3928 launched installed `codex.exe` PID 3848,
which launched `codex-code-mode-host.exe` PID 11516. This is process evidence
only; no writer, replacement, or RAH write access was established. The
processes were not terminated.

## Classification

H1, H2, H3, H4/H5, and H6 are not established. Idle PowerShell and .NET hashes
agree, but the failed instant lacks the metadata needed to attribute the
mismatch. The required result is **G — INDETERMINATE**.

The installed npm path is not acceptable as a certification boundary while the
mismatch remains unexplained. The next authorized correction should add
bytes-read and Windows file-identity logging, then run bounded process and
app-server controls without cleanup. If path mutability is established, use
the separately authorized exact-byte snapshot strategy. Do not retry until
expected, majority-vote hashes, or weaken immutability checks.

Inference count: 0. Tool execution count: 0. Codex 0.160.0 is not certified.
Admission and preferred baseline are unchanged. ADR 0005 and ADR 0030 are
unchanged. No production commit, push, tag, release, workspace suite,
canonical Desktop suite, Tauri inventory, HostExplicit gate, or CI run was
performed. Cleanup correction and Task 510B certification remain paused.

Task 510B2J added certification-only structured artifact measurement and bounded
identity controls. See
`docs/plans/2026-10-04-task-510b2j-codex-artifact-identity-instrumentation-and-controls.md`.
