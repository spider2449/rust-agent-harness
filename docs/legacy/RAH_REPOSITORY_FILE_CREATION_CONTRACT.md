# Repository File Creation Contract

Status: Core implementation accepted by ADR 0013; certified live integration complete
Date: 2026-08-25
Baseline: Task 085C

## Definitive contract

The v0.8 capability is **`repo.create-file`**. It is clearer than
`repo.file-create` and follows the repository tool naming family while saying
plainly what occurs. It is one host-authorized attempt to create **one**
previously absent UTF-8 regular file in one host-authorized repository. It is
not generic `fs.write`.

One file per call is mandatory. Exclude multiple creation, `mkdir`/parent
creation, overwrite, append, rename, deletion, chmod/executable creation,
binary data, Git add/commit/history/ref authority, arbitrary workspace paths,
shell/process, network, and multi-file transactions. This grants one new
directory entry and bounded bytes, not namespace management or an edit API.

Use a model-selected **validated repository-relative path**, not a
symbolic-target-only model. A symbolic target/path prefix makes ordinary source,
test, and configuration creation profile-specific with little added protection.
The host binds the canonical non-bare repository root and private policy; the
model never supplies a root, native path, parent, temporary name, mode, or
Git executable identity. The closed request is:

```json
{"path":"src/new_module.rs","content":"pub mod new_module;\n"}
```

The host-selected repository is not an input field. The schema is closed.
`path` is 1--1024 UTF-8 bytes. `content` is UTF-8,
contains no NUL, and is 0--262144 bytes; empty content is allowed. The full
serialized JSON request is at most **320 KiB** (327680 bytes). Content is exact:
no BOM, newline, Unicode-normalization, encoding, template, append, or final-
newline transformation. The 256 KiB content limit suits source/test/config use
while remaining materially below `repo.patch`'s 1 MiB postimage limit.

## Paths and parent identity

Paths use `/`-separated, nonempty normal components only. Reject `.`, `..`,
repeated/trailing separators, backslash, NUL, colon/ADS/trailing-stream syntax,
absolute, drive-relative/drive-qualified, UNC, verbatim/long-path and device
paths, plus case-insensitive `.git` components. Reject Windows reserved device
components: `CON`, `PRN`, `AUX`, `NUL`, `COM1`--`COM9`, `LPT1`--`LPT9`, including
trailing-dot/space and extension aliases. Unicode is opaque UTF-8: RAH performs
neither normalization nor case folding; native identity checks govern. A
case-equivalent existing Windows path is `target_exists`.

The parent must already exist, be a real directory inside the canonical root,
and retain its identity until the create point. Any symlink, junction, reparse,
mount-like, or special component from root through parent is rejected
universally, even if canonical resolution is inside the repository. This
fail-closed policy also covers root and target observations. On Unix, robust
future traversal needs directory-descriptor/openat-style hardening; Rust `std`
path checks alone cannot eliminate validation/create races. This task adds no
dependency; Task 085 must decide whether platform code is needed.

## Git and repository preconditions

Reuse the existing per-repository mutation lease. It serializes RAH calls but
not external editors, Git, antivirus, or filter drivers. Pre-existing unrelated
dirty state is allowed, as for `repo.patch`; captured unrelated sentinel and
repository observations must not change. The target must be absent from HEAD,
every index entry (including intent-to-add/unmerged), and every worktree form:
regular file, directory, link, broken link, junction/reparse, untracked, or
ignored entry. Ignored targets are rejected: they are less observer-visible and
more likely to be generated artifacts or secrets.

The capability never runs `git add`; raw index bytes, HEAD, and refs remain
unchanged. Reject `.git`, linked-worktree administration, all Git metadata, and
paths entering a submodule. A submodule requires separate authorization as its
own repository resource. Sparse support is deferred: reject targets outside the
materialized worktree or with unsupported sparse/index state.

Immediately before create, while holding the lease, revalidate non-bare/canonical
repository root and identities; capability binding and workspace containment;
all parent identities/types/no-reparse facts; target absence; HEAD/index/ref
snapshots; ignore/submodule/sparse state. Repeat relevant observations after
the attempt. This mitigates, but does not eliminate, external TOCTOU races.

## Native operation, effects, and output

Use direct exclusive creation. A single native create-new open at the target,
equivalent to `O_CREAT|O_EXCL` / `CREATE_NEW`, is the mutation **commit point**.
It must avoid following the target where supported. Reject temp-plus-rename:
portable rename is often replacement-capable and Rust `std` does not provide a
validated Windows no-replace publication primitive. Direct create-new gives
exclusive target-name acquisition, not atomic all-or-nothing content.

After create, write exact bytes, flush/close as available, then verify without
following links: regular type/identity, length, SHA-256, root/parent safety,
unchanged index/HEAD/refs, and sentinel. Unix creates intended non-executable
mode `0o600`, subject to umask; Windows has no executable-bit claim. Parent ACL
inheritance is an OS property, never model-selected authority.

The outcomes are exactly: `ok`, `invalid_target`, `precondition_failed`,
`create_failed_known`, `write_failed_known`, and `uncertain`. `ok` requires all
postconditions. Before commit, refusal proves no creation. A native create
failure is known only if post-observation proves no RAH effect. A write failure
after commit is known only when the new target and its bounded partial state are
proved. Lost OS result, failed verification, identity change, timeout,
cancellation, disconnect, or crash after a possible create is `uncertain`.
Do not delete partial files: that would be separate deletion authority. No
automatic retry/replay is allowed after creation may have occurred; existing
bridge dedupe remains unchanged.

Cancellation/timeout before commit causes no effect only when proven. During or
after creation/write it is not rollback and uses post-observation classification.
A crash between create, write, flush, and verify can leave empty, partial, or
complete content. v0.8 accepts that bounded failure and claims neither rollback,
transactionality, crash-atomic content, durability, nor recovery—only exclusive
name acquisition and exact verified postconditions on `ok`.

The current `ToolOutput` convention remains textual JSON:

```json
{"status":"ok","path":"src/new_module.rs","length":22,"sha256":"<64 lowercase hex>"}
```

Errors expose only a bounded status/category and logical path where safe; never
absolute paths, native identities, usernames, content, or diagnostics.

## Profile, permission, bridge, and ADR

The implemented private `RepositoryFileCreationPolicy` remains separate from
ADR 0012's `RepositoryWorktreeMutationPolicy`; creation is distinct from
replacement authority. `PermissionLevel::Execute` is the declared outer
dispatcher gate, not generic writing permission. `Write` does not imply this
capability.

Trusted-profile validation and effective composition are implemented additively
under the closed `capabilities[]` schema and `profile_version = 1`. The binding
accepts only the existing symbolic repository resource and an `Execute`
permission; static validation never constructs the tool or creates a file, and
effective composition constructs/registers the same host-owned policy in a
fresh `ToolRegistry`. Inventory remains redacted.

Generic Tool Bridge composition validation and certified Codex live validation
are complete. The bridge has no production special case: alias mapping,
permission, ToolRegistry dispatch, dedupe, cancellation, and no replay remain
generic. Do not amend ADR 0012 as if it granted new-path authority.

## Test and live-validation matrix

Deterministic tests: normal/empty/nested-existing-parent success; exact hash,
length, untracked `repo.status`, unchanged raw index/HEAD/refs/sentinel;
existing regular/directory/link/broken-link targets; missing/reparse/junction
parents; absolute, `..`, `.git`, submodule, sparse, ignored, ADS, UNC,
device/verbatim, reserved-name, oversized, and permission-denied rejection.
Private test-only fault seams occur after initial validation, lease, before
create, after create, mid-write, after write, before verification, and after
verification. Exercise target/parent races, verification fault, cancellation/
timeout before/after commit, duplicate identity, and no replay.

Windows must test case collision, reserved names, ADS, reparse/junction,
native identity, race no-overwrite, and final regular verification. Ubuntu must
test symlink-parent rejection, non-executable mode, target race, permission,
and containment. Profile tests cover static/effective bindings, no composition
mutation, redaction, and malformed binding. Bridge tests cover None/Read/Write
denial, Execute, canonical name/alias, exactly-once/dedupe/no-replay, and
output translation. `repo.file-info` may report a new untracked regular file;
`repo.status` verifies untracked state; Git diff does not expose untracked
content, so host hash verification remains required.

The future certified live gate uses exact native `codex-cli 0.149.0`, certified
SHA, isolated `CODEX_HOME`, `gpt-5.4`, medium reasoning, config fingerprint,
and host structural assertions. Disable shell, unrestricted writes, Codex MCP,
arbitrary process, web/network, apps/plugins, and approval bypass. A fixture
must call creation exactly once, observers/host checks prove content, untracked
state and unchanged index/HEAD/refs/sentinel, then confirm Completed, cleanup,
and emit `RAH_CREATE_FILE_LIVE_OK`.

| Property | `repo.patch` | `repo.create-file` |
| --- | --- | --- |
| target initially | clean HEAD-tracked file | absent from HEAD/index/worktree |
| overwrite | bounded replacement | forbidden |
| index/history | no/no | no/no |
| path | validated existing path | validated new relative path |
| commit point | native replacement | exclusive create-new |
| rollback | no | no |
| uncertain effect | possible | possible, including partial new file |

Implementation: `repo.create-file` is a closed `profile_version = 1` Trusted
Profile capability. Static validation accepts only symbolic `git` executable
and repository resource references and never constructs the tool or creates a
file. Effective composition resolves those host resources, constructs the
private host-bound tool, and registers it in a fresh `ToolRegistry`; it still
does not create a target. Only ordinary `Tool` execution may perform the one
authorized native creation.

The Generic Tool Bridge remains unchanged. It exposes the canonical closed
`{path, content}` schema through its private alias mechanism and relies on its
ordinary permission, dispatch, cancellation, deduplication, and output
translation behavior. Task 086 added deterministic coverage for composition,
redacted inventory, Execute gating, one successful bridge dispatch, and
no-replay treatment of uncertain and known write-failure outcomes. Certified
live Codex validation is complete under Task 087.
