# Task 409 — Close `repo.list` Binary-File Evidence Gap

Date: 2026-09-24
Status: **STOP — required Desktop harness validation failed before Windows live certification**

## Scope and starting checkpoint

Task 409 closes only the tracked-binary structural visibility and no-content
evidence gap identified by Task 408. It does not authorize production
behavior, authority, permission, schema, dependency, version, changelog,
release-gate, tag, publication, push, or Task 410 changes.

Starting checkpoint:

```text
HEAD: fb6a62654d6c185b5c37a7a576cc465704ea5582
message: docs: audit v0.32 milestone readiness
worktree: clean
```

The expected checkpoint and clean worktree were verified before editing.

## Reconstructed Task 374 and Task 375 requirements

Task 374 states: “The capability must not include file contents.” Its minimum
candidate entry is exactly a repository-relative logical `path` and a
structural `kind = file | directory`. Its deterministic evidence table
specifies “Submodule/gitlink and binary files | No unintended child-repository
or content disclosure.” Its Windows live fixture explicitly includes “a
binary file”; the gate requires active-repository-only, sanitized,
bounded output.

Task 375 froze the successful output as one JSON object in one `ToolOutput`
JSON content item. Each entry has only `path` and `kind`; kinds are exactly
`file` or `directory`. It explicitly excludes contents, snippets, timestamps,
sizes, OIDs, private paths, and other metadata. The request is closed: `{}`
means selected repository root, with optional safe `path`; unknown fields fail
closed. The inventory is fixed Git tracked inventory projected against the
selected current worktree. Binary-specific output semantics are not part of
the contract.

## Task 408 blocking finding

Task 408 found no deterministic `repo.list` test or Task 382 live case with a
tracked binary file. Source inspection suggested the implementation emits
path/kind metadata without reading content, but this did not satisfy the
explicit evidence requirement. Task 408 classified this as a blocking
certification gap, not a production defect.

## Deterministic-test preflight and frozen test

Existing deterministic family: `crates/rah-tools/src/repository_list.rs`,
module `repository_list::tests`. It uses the existing `Fixture` temporary
native Git repository, `commit_all`, `native_git`, `Tool::execute` invocation,
and `json_output` response parser. Existing adjacent tests cover ordinary
tracked files/direct-child listing, closed request schema, ordering and
saturation, visibility and privacy sentinels, and no intentional mutation.
The owning package is `rah-tools`.

Frozen new test:

```text
repository_list::tests::tracked_binary_file_is_listed_structurally_without_disclosing_contents
```

It creates one tracked direct-child file `binary-sentinel.dat` in the existing
fixture. Bytes comprise leading NUL and `0xff`, the harmless recognizable
ASCII sentinel `RAH_REPO_LIST_BINARY_SENTINEL`, then `0x80` and trailing NUL.
After fixture commit it proves tracking with `git ls-files --error-unmatch`,
then invokes `RepositoryListTool` using request `{}`.

Required assertions: exact `{path, kind}` file entry; exact existing response
top-level field set; no entry fields for contents/bytes/body/preview/text;
serialized ToolOutput lacks the sentinel and complete byte sequence. Successful
listing of the NUL/non-UTF-8 file demonstrates no UTF-8 decoding prerequisite.
No binary-specific classification is introduced.

### Phase status

- Exact Task 374/375 wording reconstructed: complete.
- Test preflight and exact test frozen: complete.
- Deterministic test implementation/validation: passed.
- Independent evidence audit: passed.
- Deterministic evidence commit: `580880f2619de4ed1e3810aec31b2ad27d6d91c3`.
- Windows live harness validation: **failed**; no certification was run.
- Fresh Windows live binary certification: **not run; stopped at failed validation gate**.
- Final evidence audit/verdict: **STOP; Task 409 is incomplete**.

## Deterministic evidence and independent audit

Test source: `crates/rah-tools/src/repository_list.rs`, test
`repository_list::tests::tracked_binary_file_is_listed_structurally_without_disclosing_contents`.
The fixture commits the file using its existing `Fixture::commit_all()` helper;
`git ls-files --error-unmatch -- binary-sentinel.dat` must succeed and return
that exact path. Request `{}` lists the fixture repository root through
`Tool::execute` on `RepositoryListTool`, the production Tool implementation.

The response assertion requires the sole entry to equal
`{"path":"binary-sentinel.dat","kind":"file"}`. It checks the exact
top-level response field set, exactly two entry fields (`path`, `kind`), and
exactly three fixed omission-accounting fields. It rejects content-shaped
entry fields (`contents`, `bytes`, `body`, `preview`, `text`,
`decoded_text`). Successful execution with NUL and non-UTF-8 bytes proves the
listing path does not require text decoding. Serialized `ToolOutput` must not
contain the ASCII sentinel or the complete fixture byte sequence.

Independent audit against the untouched starting `HEAD`
`fb6a62654d6c185b5c37a7a576cc465704ea5582`:

| Audit question | Finding |
| --- | --- |
| Is the file tracked? | Yes. The test commits it and independently runs `git ls-files --error-unmatch`, checking exact path output. |
| Is it binary/non-text? | Yes. Its byte sequence starts NUL/`0xff`, contains a recognizable sentinel, and ends `0x80`/NUL; it is not valid UTF-8. |
| Is it in the listed directory? | Yes. It is a root direct child and the exact request is `{}` (root). |
| Does it invoke real `repo.list` logic? | Yes. It constructs `RepositoryListTool` and calls its `Tool::execute`; no helper/projection shortcut is used. |
| Is structural visibility proved? | Yes. The only returned entry equals the exact safe path and ordinary `file` kind. |
| Is no-content proved? | Yes. The success response has the exact closed top-level fields, entry fields, and omission fields; no content field exists. Both sentinel and full byte sequence are absent from serialized `ToolOutput`. |
| Could the test pass without exercising the binary entry? | No. The exact sole-entry assertion requires this tracked direct child to be returned as `kind=file`. |
| Did production Rust change? | No. The diff against the starting commit is confined to a new test in the existing test module; the `RepositoryListTool` implementation is unchanged. |
| Did authority or schema behavior change? | No. No authority, permission, response/request schema, profile, or registry source changed. |

Audit verdict:

```text
PASS — REPO.LIST BINARY NO-CONTENT DETERMINISTIC EVIDENCE VERIFIED
```

Validation against the final deterministic test source:

```text
cargo test -p rah-tools repository_list::tests::tracked_binary_file_is_listed_structurally_without_disclosing_contents -- --exact
  PASS: 1 passed, 0 failed
cargo fmt --check
  PASS
cargo test -p rah-tools -- --test-threads=1
  PASS: 340 library unit tests; all package integration suites passed; 2 existing ignored integration tests; 0 failures
cargo clippy -p rah-tools --all-targets --all-features -- -D warnings
  PASS
git diff --check
  PASS
```

The deterministic test and this audit record were committed as:

```text
580880f2619de4ed1e3810aec31b2ad27d6d91c3
test: cover repo.list binary no-content contract
```

## Required Windows harness validation stop

The existing ignored Task 379/382 Windows certification harness was inspected
as the planned live path. A binary-only test-harness addition was prepared in
`crates/rah-desktop/src/main_tests.rs`: add and track the binary file in the
existing linked-A fixture, record a relative path/length/SHA-256, and assert
closed structural output through the active-A registry path. Formatting passed,
but the required `cargo test -p rah-desktop -- --test-threads=1` package
validation failed. In accordance with Task 409's stop rule, the live
certification was not run. The uncommitted harness change was removed; no
production code or committed harness code changed.

Exact package result:

```text
316 passed; 4 failed; 18 ignored; 0 measured
```

Failures:

```text
tests::task349_close_clears_pending_no_effect_commit_authorization
  crates/rah-desktop/src/main_tests.rs:9542
  staged repository should have an authorizable review

tests::task_321_c_authorization_preparation_cannot_rearm_after_activation
  crates/rah-desktop/src/main_tests.rs:10887
  fresh review observes: StagedDiffExecution

tests::task_321_e_unstage_reservation_wins_over_activation
  crates/rah-desktop/src/main_tests.rs:11385
  A should expose an Unstage action

tests::task_321_i_real_stage_reservation_rejects_real_connect_publication
  crates/rah-desktop/src/main_tests.rs:9047
  workflow should expose the requested real index action
```

None of the four reported failure names concerns the new binary fixture.
This does not establish whether those pre-existing workflow failures are
reproducible independently. The Windows edition/build, rustc/Cargo/Git
versions, certification HEAD, binary fixture output/fingerprint, live request
result, live privacy/isolation, HostExplicit live result, and live no-content
assertions remain **uncertified and unrecorded**. The deterministic evidence
HEAD at the stop is the committed
`580880f2619de4ed1e3810aec31b2ad27d6d91c3`.

Required stop verdict:

```text
STOP — REPO.LIST BINARY WINDOWS CERTIFICATION NOT RUN AFTER REQUIRED DESKTOP PACKAGE VALIDATION FAILED
```

No Task 410 work is started. A follow-up must resolve or independently
disposition the four validation failures before rerunning the unchanged binary
certification gate.

## Final report

Task 409 does **not** pass. The deterministic binary no-content gate is closed,
but the required Windows live binary gate remains open because its required
Desktop package validation failed. No production Rust, authority, permission,
schema, dependency, version, changelog, release gate, or certification
harness code changed. No push or tag occurred. No Task 410 release preparation
is authorized or started.
