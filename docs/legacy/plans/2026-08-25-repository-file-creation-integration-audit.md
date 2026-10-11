# Repository File Creation Integration Audit

## Scope and result

Task 085C audited `repo.create-file` at `fce4fa5` plus the narrowly scoped
audit corrections. Result: accepted. ADR 0013 is Accepted. This audit adds no
trusted-profile binding, Generic Tool Bridge special case, live Codex path, or
additional mutation authority.

## Contract checklist

| Area | Classification | Evidence |
| --- | --- | --- |
| Canonical name, one file, Execute declaration, closed `path`/`content` schema | IMPLEMENTED + TESTED | `repository_create_file.rs`; `repository_create_file.rs` integration tests |
| Host-bound root/Git identity, private policy, lease, pre-create revalidation | IMPLEMENTED + TESTED | policy construction and fault tests |
| Relative-path, parent, link/reparse, HEAD/index/worktree, ignore, submodule, sparse checks | IMPLEMENTED + TESTED | policy checks; real Git fixtures |
| UTF-8/NUL/empty/content/request/path byte limits | IMPLEMENTED + TESTED | parser and boundary tests |
| Exclusive native create, no overwrite/stage/delete/retry, non-executable Unix mode | IMPLEMENTED + TESTED | native platform module and fault tests |
| Postcondition, raw index/HEAD/ref invariants, known partial-write certification | IMPLEMENTED + TESTED | `verify_post`, `verify_known_partial`, snapshots |
| Redacted model-visible status output | IMPLEMENTED + TESTED | closed-schema/redaction test |
| Trusted Profile, Generic Tool Bridge, certified live Codex | NOT IMPLEMENTED | explicitly deferred to Tasks 086 and 087 |

No implementation is broader than the accepted authority contract. During this
audit the path bound was narrowed from 4096 to 1024 UTF-8 bytes, raw index-byte
postcondition comparison replaced a semantic index listing, and known
partial-write classification now requires postcondition certification.

## Native security verdict

Windows uses handle-relative `NtCreateFile`, opens every parent relative to the
already verified handle with reparse rejection, and uses `FILE_CREATE` for the
final non-directory target. Unix uses directory FDs, `openat`, `O_NOFOLLOW`,
`O_CREAT | O_EXCL`, and requested mode `0o600` subject to umask. Native tests
prove exclusive target preservation and parent pathname replacement cannot
redirect a creation through the retained parent handle.

## Failure taxonomy

| Status | Phase | Mutation possible | Replay |
| --- | --- | --- | --- |
| `ok` | certified postcondition | committed and verified | no |
| `invalid_target` | closed request/path/content validation | no | not applicable |
| `precondition_failed` | Git/filesystem/policy validation before create | no | caller may submit a new request |
| `create_failed_known` | exclusive native create reports no acquired target | no RAH effect | no automatic retry |
| `write_failed_known` | after create, retained regular target and bounded prefix plus Git invariants certify | yes | never |
| `uncertain` | any post-commit certification failure | possible | never |

Partial-write residual effect is a retained new file containing a bounded prefix
of the requested bytes. `uncertain` cannot be upgraded because the host cannot
prove the committed target identity and all required postconditions.

## Coverage inventory and limitations

On Windows, the native inventory is: `exclusive_create_preserves_racing_target_and_exact_content`,
`target_race_and_partial_write_never_cleanup_or_retry`,
`parent_replacement_after_handle_open_cannot_redirect_creation`,
`windows_handle_walk_rejects_junction_parent`, and the Tool fault-mapping tests
`target_race_uses_one_exclusive_create_without_overwrite_or_retry`,
`partial_write_is_retained_and_reported_once`, and
`lost_post_create_certification_is_uncertain_without_replay_or_cleanup`.
The schema path matrix covers drive-qualified, UNC, verbatim/device, ADS,
reserved-name, and backslash forms on every platform. Ubuntu CI executes the
Unix native descriptor tests, symlink-parent rejection, non-executable mode,
tool success, target race, partial write, uncertain result, real submodule,
sparse rejection, intent-to-add, all conflict stages, and request-boundary
fixtures. Ubuntu is not claimed as evidence for Windows junction behavior.

The direct Tool tests deliberately do not invent permission dispatch: the Tool
definition declares Execute, while dispatcher enforcement is a Task 086 bridge
composition concern. The private policy is still independently required; a
Tool cannot be constructed without host-supplied absolute canonical repository
and Git identities.

## API and dependency audit

New public symbols since Task 084 are
`REPOSITORY_CREATE_FILE_TOOL_NAME` and `RepositoryFileCreationTool`; both are
necessary host-construction API. `RepositoryFileCreationPolicy` and native
types are crate-private; test hooks are test-only. No crate dependency was
added. Task 085A enabled existing `windows-sys` package features
`Win32_System_IO`, `Wdk_Foundation`, and `Wdk_Storage_FileSystem` only.

## Deferred integration

`repo.create-file` is not registered through trusted-profile effective
composition and has no Generic Tool Bridge production special case. There is no
certified live Codex invocation claim. Task 086 is the next task; Task 087 is
the separately authorized live gate.
