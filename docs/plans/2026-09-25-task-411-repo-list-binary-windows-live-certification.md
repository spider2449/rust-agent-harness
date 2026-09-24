# Task 411 — Resume `repo.list` Binary-File Windows Live Certification

Date: 2026-09-25
Status: **PASS — REPO.LIST BINARY-FILE WINDOWS LIVE EVIDENCE CLOSED**

## Verdict

```text
PASS — REPO.LIST BINARY-FILE WINDOWS LIVE EVIDENCE CLOSED
```

Task 411 resumes and closes only Task 409's remaining Windows live binary-file
gate. The committed Windows harness passed the full pre-certification Desktop
suite and all required static checks. The exact committed harness HEAD then
passed the existing fresh Windows Desktop active-repository certification
path with a Git-tracked NUL/non-UTF-8 binary direct child. `repo.list` returned
only the closed structural entry; no payload, sentinel, or raw host path was
disclosed. No production source, authority, request/response contract, or IPC
schema changed.

## Checkpoint and evidence lineage

Task 411 starting checkpoint:

```text
HEAD: aa7f36cc13fc98330192f8ffab6e86ea922e3ac6
message: docs: disposition desktop validation failures
worktree: clean
```

The checkpoint and clean worktree were verified before work began.

Task 409 deterministic evidence remains closed at:

```text
580880f2619de4ed1e3810aec31b2ad27d6d91c3
test: cover repo.list binary no-content contract
```

The verified deterministic test is
`repository_list::tests::tracked_binary_file_is_listed_structurally_without_disclosing_contents`
in `crates/rah-tools/src/repository_list.rs`. It proves tracked binary
structural visibility, NUL/non-UTF-8 tolerance, closed metadata, and sentinel
and byte-sequence non-disclosure. Task 411 did not rewrite or rerun that test.

Task 410 disposition prerequisite is commit
`aa7f36cc13fc98330192f8ffab6e86ea922e3ac6`. Task 410 independently passed
each of the four formerly failing focused Desktop tests 3/3 and passed the
full serial Desktop suite twice at 320 passed, 18 ignored, 0 failed, 338
discovered. The original Task 409 failures remain historically recorded;
Task 410 classified them as transient and non-reproduced without claiming
they never occurred or identifying their trigger.

The Task 409 stop artifact remains unchanged. Its discarded uncommitted binary
case intent was recovered from that artifact: put a tracked binary direct
child in selected linked worktree A, record a safe path/length/fingerprint,
and assert structural active-registry output and non-disclosure. No committed
discarded diff exists in the harness file history; `git log` showed only the
existing harness extraction/maintenance history. Task 411 froze a fresh
minimal case before editing.

## Task 382 harness and invocation path

The established Task 382 harness is
`task379_windows_repository_list_live_certification` in
`crates/rah-desktop/src/main_tests.rs`, compiled as `tests` through
`crates/rah-desktop/src/main.rs:9869-9870`. It is an ignored Windows live test
with an explicit opt-in guard:

```text
RAH_RUN_TASK379_REPOSITORY_LIST_LIVE=1
```

The Task 382 harness creates a disposable native Git main worktree plus linked
worktrees A and B outside the RAH checkout. It admits all three through the
Desktop repository-membership path, selects A host-side, builds the production
`desktop_tool_registry` and effective composition for that selected member,
then dispatches `repo.list` through `ToolRegistry::execute` with a real
`ToolCall`. Task 411 changed the existing `task379_list_value` helper from
direct execution of a fetched Tool to generic registry dispatch. It does not
call `RepositoryListTool::execute` directly.

The established command is:

```powershell
$env:RAH_RUN_TASK379_REPOSITORY_LIST_LIVE = '1'
cargo test -p rah-desktop tests::task379_windows_repository_list_live_certification -- --ignored --exact --nocapture --test-threads=1
```

Existing cases naturally rerun in that harness: active A root and nested
direct-child listings, closed invalid requests, deterministic ordering and
saturation, tracked/untracked/ignored/deleted/sparse behavior, active B
isolation, submodule and nested-repository rejection, junction/reparse
rejection, optional symlink behavior, privacy, HostExplicit classification,
and before/after repository-state checks. A repo.search smoke regression also
runs. The standard repo.list matrix was rerun with the binary case; no
separate certification architecture was created.

## Frozen binary live case

Only the existing Windows certification harness source was changed. In its
existing linked-A fixture setup it now writes the direct child
`binary-sentinel.dat` with this byte shape, without printing the payload:

```text
NUL, 0xff, ASCII sentinel RAH_REPO_LIST_BINARY_SENTINEL, 0x80, NUL
```

The harness stages and commits the file as part of the existing A fixture
commit, then runs `git ls-files --error-unmatch -- binary-sentinel.dat` from
A and requires the exact path line. The live run recorded:

```text
relative path: binary-sentinel.dat
byte length: 33
SHA-256: 086264dbbcb7090ff2d5e1dd74d3437a003562da32aa5a73047eb397a97a5ea0
Git tracked output: binary-sentinel.dat
```

The actual live root request was exactly:

```json
{}
```

The request ran after host selection of linked A through the committed
Desktop registry path. Main and linked B carry independent sentinels; the A
root result must exclude `main-only.txt` and `b-only.txt`.

## Pre-certification validation and independent harness review

Final validation was run against the exact harness source later committed as
`64467c9d8dfbb0cb465a8be72e3c312bc34c876a`:

```text
cargo fmt --check
  PASS

cargo test -p rah-desktop -- --test-threads=1
  PASS: 320 passed, 0 failed, 18 ignored, 338 discovered
  duration: 851.32s

cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings
  PASS

git diff --check
  PASS
```

The first intermediate compile attempt exposed a harness key-comparison type
mismatch before tests started. It was corrected in the harness; no test began
or failed in that attempt. The final full-suite run above is the required
pre-certification result. In that final run all four Task 409 failure names
passed; no new test failures appeared.

Independent review verdict:

```text
PASS — REPO.LIST BINARY WINDOWS HARNESS READY
```

Review confirmed the file is genuinely binary (NUL and invalid UTF-8), is
tracked inside selected A, and is a root direct child for request `{}`. The
assertion requires the returned exact path with `kind=file`; it cannot pass
without the binary entry. The entry has exactly `path` and `kind`, and the
response has exactly the existing seven top-level structural fields. The
sentinel and payload fingerprint must be absent from the serialized result.
All captured A/main/B roots and private/common Git directories must be absent
from it. The A result must exclude main/B sentinels. Existing before/after
checks compare HEAD, branch, porcelain-v2 state, and semantic cached diff for
main/A/B. The harness contains no production code change.

## Harness commit and certification checkpoint

The only harness file changed was:

```text
crates/rah-desktop/src/main_tests.rs
```

No fixture-support file, production source, manifest, dependency, authority,
or schema file changed. Harness commit:

```text
64467c9d8dfbb0cb465a8be72e3c312bc34c876a
test: add repo.list binary Windows certification case
```

Immediately after commit and again immediately before live execution:

```text
TASK411_CERTIFICATION_HEAD=64467c9d8dfbb0cb465a8be72e3c312bc34c876a
worktree: clean
```

The test binary used was
`target/debug/deps/rah_desktop-0bf2d3ba1e599be0.exe`.

## Certification environment and live result

Fresh environment capture immediately before certification:

```text
Windows caption: Microsoft Windows 11 IoT Enterprise LTSC
Windows version: 10.0.26100
Windows build: 26100 (UBR 9457)
Windows display version: 24H2
Architecture: x64 / 64-bit
Registry product name: Windows 10 IoT Enterprise LTSC 2024
Edition ID: IoTEnterpriseS
rustc: 1.98.1 (48a229cea 2026-09-01)
cargo: 1.98.1 (797e8a9bc 2026-08-05)
Git: 2.55.0.windows.5
Git executable: C:\Program Files\Git\cmd\git.exe
```

The Task 382 real-model baseline is not required for this registry-driven
case. Ambient `codex-cli 0.156.1` was observed but not used; no Codex runtime,
model inference, network, or credentials were involved. Task 382's historical
nonclaim about its unavailable pinned `codex-cli 0.149.0` remains intact.

Exact live invocation:

```powershell
$env:RAH_RUN_TASK379_REPOSITORY_LIST_LIVE = '1'
cargo test -p rah-desktop tests::task379_windows_repository_list_live_certification -- --ignored --exact --nocapture --test-threads=1
```

Result on certification HEAD `64467c9d8dfbb0cb465a8be72e3c312bc34c876a`:

```text
1 passed; 0 failed; 0 ignored; 337 filtered out
finished in 43.96s
RAH_V032_REPOSITORY_LIST_LIVE_OK
```

Sanitized live evidence printed by the harness:

```text
RAH_V032_BINARY_FIXTURE_PATH=binary-sentinel.dat
RAH_V032_BINARY_FIXTURE_BYTES=33
RAH_V032_BINARY_FIXTURE_SHA256=086264dbbcb7090ff2d5e1dd74d3437a003562da32aa5a73047eb397a97a5ea0
RAH_V032_BINARY_FIXTURE_GIT_TRACKED=binary-sentinel.dat
RAH_V032_BINARY_REPO_LIST_REQUEST={}
RAH_V032_BINARY_TRACKED_ENTRY_PRESENT=PASS
RAH_V032_BINARY_RESPONSE_ENTRY={"kind":"file","path":"binary-sentinel.dat"}
RAH_V032_BINARY_KIND_FILE=PASS
RAH_V032_BINARY_CLOSED_SCHEMA_NO_CONTENT=PASS
RAH_V032_BINARY_SENTINEL_ABSENT=PASS
RAH_V032_BINARY_RAW_HOST_PATH_ABSENT=PASS
RAH_V032_BINARY_ACTIVE_REPOSITORY_ISOLATION=PASS
```

Required assertion results:

| Requirement | Result |
| --- | --- |
| Tracked binary direct child structurally present | PASS |
| Exact repository-relative path and `kind=file` | PASS |
| Existing closed response schema; entry fields only `path`, `kind` | PASS |
| No content/contents/body/bytes/preview/text field | PASS |
| Sentinel and payload fingerprint absent | PASS |
| Raw Windows fixture/root/Git paths absent | PASS |
| Selected linked-A isolation; main/B sentinels absent | PASS |
| No intentional mutation across main/A/B | PASS |

The test's before/after repository-state assertion passed. Standard Task 382
cases also completed successfully. Windows symlink creation was unavailable
in this configuration, so the harness reported
`RAH_V032_WINDOWS_SYMLINK_LIVE_NONCLAIM=1`; Task 411 makes no Windows symlink
claim.

## Authority, schema, and change boundaries

The existing live effective entry remains:

```text
EffectClass=ReadOnly
AuthorityCategory=RepositoryObservation
PermissionLevel=Execute
repository_bound=true
source_kind=RepositoryHost
```

The existing HostExplicit check passed with exactly 11 host kinds, while
`host_kind("repo.list") == None`. No `RepositoryObservation` classification,
PermissionLevel, active-repository ownership, ToolRegistry authority, or
HostExplicit behavior changed. No authority/security delta occurred.

No request/response, IPC, or serialized schema change occurred. There is no
new response field or binary-specific classification. There is no production
source change, dependency change, ADR change, or release preparation.

## Task 409 evidence-chain closure

```text
Task 374 binary no-content requirement
  -> Task 409 deterministic tracked-binary test
  -> independent deterministic evidence audit
  -> deterministic commit 580880f2619de4ed1e3810aec31b2ad27d6d91c3
  -> Task 410 Desktop failures independently dispositioned as non-reproduced
  -> Task 411 committed Windows harness 64467c9d8dfbb0cb465a8be72e3c312bc34c876a
  -> exact-head live Desktop ToolRegistry::execute invocation
  -> tracked binary structurally visible as path/kind=file
  -> no content, sentinel, fingerprint, or raw path disclosed
```

Therefore:

```text
DETERMINISTIC BINARY EVIDENCE: CLOSED
WINDOWS LIVE BINARY EVIDENCE: CLOSED
```

Task 409's original stop remains valid historical evidence. This task resumes
and closes its remaining live gate; Task 408, Task 409, and Task 410 artifacts
were not rewritten.

## Explicit nonclaims

Preserve the v0.32 nonclaims:

- no arbitrary filesystem enumeration;
- no untracked/ignored discovery;
- no recursive browse;
- no content-reading capability;
- no mutation authority;
- no HostExplicit expansion;
- no multiple-active-repository support;
- no model-selected repository;
- no general Windows symlink certification;
- no Codex model-inference certification;
- no arbitrary-byte semantics beyond structural non-disclosure;
- no race-free or transactional observation guarantee, rollback, or replay;
- no claim of zero filesystem writes.

## Commits, final state, and next task

Harness certification commit:

```text
64467c9d8dfbb0cb465a8be72e3c312bc34c876a
test: add repo.list binary Windows certification case
```

This Task 411 artifact is the only post-certification documentation change.
Its commit necessarily follows the certification HEAD and is not itself the
executed live-certification HEAD. It is committed separately as:

```text
docs: record repo.list binary Windows certification
```

Final status is verified clean after that documentation commit. No push or
tag occurred. Do not start the next task automatically. Recommended next
task:

```text
Task 412 — RAH v0.32.0 Release Preparation
```
