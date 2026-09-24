# Task 409 — Close `repo.list` Binary-File Evidence Gap

Date: 2026-09-24
Status: **IN PROGRESS — deterministic gate passed; Windows live gate pending**

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
- Deterministic evidence commit: pending.
- Fresh Windows live binary certification: pending.
- Final evidence audit/verdict: pending.

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

The bounded deterministic evidence commit and exact resulting `HEAD` will be
recorded here before any live certification.

## Final report

To be completed after all three mandatory gates. No release or publication
work is part of this artifact.
