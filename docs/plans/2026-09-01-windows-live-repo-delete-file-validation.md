# Task 163 — Windows live `repo.delete-file` validation

## Scope

Validate one real Windows Desktop/trusted-host/Codex runtime deletion on a
disposable Git repository, and durably record the request, private alias,
tool lifecycle, completion, filesystem, Git, and cleanup evidence required by
ADR 0017. The source repository must remain untouched.

## Starting gate

- Required `HEAD`: `977422146ca68784d81cfd509902bfaefb75a8ee`
- Required `origin/master`: the same SHA
- Initial worktree: clean
- Certified Codex: `codex-cli 0.149.0`

## Evidence status

This document is updated only with observations from the live run. Missing
observations are recorded as evidence gaps and are not inferred from tests or
model text.

## Narrow observability

If required lifecycle evidence is absent from the existing Desktop path, use
only an environment-gated, sanitized evidence sink. It may record the public
tool name, observed Codex alias, closed request preimage fields, lifecycle
events, completion, and repository refresh. It must not create authority,
change the deletion contract, or persist absolute repository paths or secrets.

## Validation chronology

1. Baseline checks passed at the required starting SHA. The certified baseline
   verifier passed for `codex-cli 0.149.0`; the native Codex SHA-256 was
   `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, and
   the companion code-mode host SHA-256 was
   `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.
2. A disposable repository was created at `D:\rah-task163-live` with native
   Git `2.54.0.windows.1`. Its baseline `HEAD` was
   `cd880d98c705e8339346d8e3c274b805d207f2cf` on `refs/heads/master`.
3. Before the live turn, `delete-target.txt` existed as a tracked regular file
   with SHA-256
   `2d7ae1968fa24a605cd7e715213d9e94f3477f740485642b7608ccc413cebe86` and 26
   bytes. `sentinel.txt` existed with SHA-256
   `ed54a6ef2da77ea8983de6f91adcd96db302458ad81d84a0c668ae89bb9f7fb0`.
   The index and worktree were clean; the stage-0 entries were recorded by
   `git ls-files --stage`.
4. The release Desktop binary was launched with the certified Codex path and
   the environment-gated evidence path. The real Desktop selected the
   disposable repository and connected to `codex-cli 0.149.0`; the UI reported
   repository tools active and chat ready.
5. The exact model-visible prompt requested one call to public
   `repo.delete-file` with path `delete-target.txt`, the SHA-256 above, and byte
   length `26`. Codex replied in the Desktop conversation that the tool was
   unavailable. No target deletion occurred and no live evidence record was
   emitted.

## Observed live result

The primary live gate is **NOT VALIDATED / BLOCKED BEFORE TOOL REQUEST**.

- Public tool label: not observed in a live lifecycle event.
- Codex-private alias: not observed.
- `ToolRequested`: not observed; required `1`, actual evidence `0`.
- `ToolStarted`: not observed; required `1`, actual evidence `0`.
- `ToolFinished`: not observed; required `1`, actual evidence `0`.
- Target: still present; no native deletion attempt is claimed.
- Sentinel: unchanged at the recorded SHA-256.
- Index/worktree/HEAD: unchanged; no unstaged deletion exists.
- No replay or retry was performed after the pre-effect failure.
- `RAH_REPO_DELETE_FILE_LIVE_OK`: not observed.

The empty evidence file is itself evidence that the newly added sink did not
observe a dynamic deletion call. It does not prove that a tool was advertised,
and deterministic bridge tests do not substitute for the missing live event.

## Evidence gap and disposition

The live Desktop connection did not expose `repo.delete-file` to the model in
the attempted turn, despite the selected repository's Rust-owned deletion
authority being constructed. This blocks Task 163 completion because the
required real Desktop/trusted-host/runtime path and exact lifecycle counts
were not exercised. No authority boundary was changed to bypass this gap.

The disposable repository remains inspectable for diagnosis. No negative stale
preimage case was started, because doing so would not repair the primary live
gate and would broaden the failed run.

## Deterministic and workspace validation

- `cargo fmt --check`: PASS after `cargo fmt`
- Focused `cargo test -p rah-runtime-codex -p rah-desktop`: PASS
- `cargo check --workspace`: PASS
- `cargo test --workspace`: PASS
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS
- `git diff --check`: PASS
- `cargo metadata --no-deps --format-version 1`: PASS

These checks prove the instrumentation and existing deterministic contracts
compile and pass; they do not upgrade the failed live observation into a live
Codex claim.

## Final result

Task 163 is not complete. Exact-head CI, push, and milestone/release work must
not be claimed from this run.
