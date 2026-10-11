# RAH v0.7 Scope and Authority Roadmap

Status: Task 070 research/design only

Baseline: RAH v0.6.0 (`v0.6.0`, annotated tag object
`0a31db7ede796051a026e79187417c7759d349d3`, peeled release commit
`6326c18937bbcfd1e515001692a2c88c6884d552`), followed only by release
bookkeeping commit `ac84f06eb66de1a02df97138564fa9e7034911eb`. The reviewed
adapter baseline is `codex-cli 0.149.0`.

## Decision

**RECOMMENDED: B — multiple exact replacements in one existing tracked file,
as a bounded `repo.patch` v0.7 extension.**

This is the smallest new useful coding step. It lets an agent make a coherent
source-file change (for example, adjust imports, an implementation, and tests
within one module) while preserving the existing repository identity,
single-file lease, full-file preimage, direct replacement, verification, and
no-replay model. It adds no new target class: it neither creates a path nor
deletes a path, changes the index, or changes history.

It does **not** make RAH a complete autonomous coding environment. The next
practical gaps remain cross-file work and creation, but closing either one
changes the authority and failure model materially more than v0.7 should.

## Current workflow and product gap

The released workflow is:

```text
inspect status -> inspect file -> inspect unstaged/staged diff
-> one exact replacement -> inspect result
```

The missing steps have different authority costs:

| Missing step | Coding value | New authority surface | v0.7 disposition |
| --- | --- | --- | --- |
| Several coordinated edits in one source file | High | Same existing tracked file; richer transform only | Adopt |
| Create a source file | High | Persistent model-selected pathname that did not exist | Defer |
| Edit several files | Very high | Multi-target commit/partial-failure semantics | Defer |
| Delete/rename | Medium | Destructive namespace and recovery authority | Defer |
| Commit | Medium | Index/ref/history/hooks/identity authority | Defer |

Therefore the highest practical value per authority increment is several exact
replacements in one existing file. `fs.write` is rejected: it loses repository
identity, tracked-target proof, strict preconditions, mutation lease, and
post-effect verification. `shell.exec` is not a substitute: model-selected
program, argv, cwd, and environment would be ambient execution authority.

## Scoring method

Each criterion uses 1--5. Higher is better for product value, workflow,
reuse, deterministic testability, and portability; higher is worse for the
risk/complexity columns. Score is:

```text
2 * product value + 2 * coding workflow + reuse + testability + portability
- security - Windows risk - live-Codex complexity - rollback risk
- scope size - technical debt
```

`New authority` is qualitative and is not hidden in the score. Scores compare
v0.7 suitability, not lifetime product desirability.

| Candidate | Product value | Security complexity | Architecture reuse | Testability | New authority | Score | Recommendation |
| --- | ---: | ---: | ---: | ---: | --- | ---: | --- |
| A. bounded file creation | 5 | 4 | 3 | 3 | Persistent new path/content | 4 | Defer; research separately |
| B. multi-replacement one file | 5 | 2 | 5 | 5 | No new target class | 22 | **Recommended** |
| C. repository edit transaction | 5 | 5 | 2 | 2 | Multi-file/create/delete transaction | -8 | Long-term design only |
| D. unified patch/hunks | 4 | 4 | 3 | 3 | Patch parser/path language | 2 | Defer; no fuzzy apply |
| E. Git commit/history | 3 | 5 | 2 | 3 | Index/ref/history mutation | -5 | Separate future milestone |
| F. session/workflow persistence | 3 | 4 | 3 | 3 | Durable authority/transcript state | 1 | Separate product track |
| G. network MCP/HTTP | 3 | 5 | 2 | 2 | Network/identity/credential authority | -9 | Defer |
| H. PluginManager/lifecycle | 3 | 5 | 2 | 2 | Installation/executable lifecycle | -10 | Defer |
| I. dynamic profile reload | 2 | 5 | 2 | 2 | Live authority replacement | -12 | Defer |
| J. cross-platform hardening | 4 | 2 | 4 | 4 | No new authority | 13 | Required release gate, not primary |

The portability term includes both Windows and Unix risk. B dominates because
it directly improves coding usefulness without adding a namespace, network,
history, provider, or durable-state authority.

## Candidate impact matrix

| Candidate | ADR classification | PermissionLevel | Trusted-profile impact | Generic Tool Bridge | Effect/retry boundary |
| --- | --- | --- | --- | --- | --- |
| A | New ADR required | Reuse Execute outer gate | New closed capability; likely symbolic repository resource only | No bridge change | Final create/rename is commit point; never replay uncertainty |
| B | ADR 0012 extension likely | Reuse Execute outer gate | New closed capability binding; no schema version if existing capability list supports it | No bridge change; existing generic tests | Final single-file replacement is commit point; never replay |
| C | New ADR required | Reuse Execute could gate, but not authorize | New closed transaction capability; likely schema extension | No bridge change | Per-file commits make outcome uncertain after first visible mutation |
| D | New ADR required | Reuse Execute outer gate | New closed capability | No bridge change | Application/replacement attempt is commit point; no retry |
| E | New ADR required | Execute remains appropriate | New closed capability and symbolic repository resource | No bridge change | `git commit`/ref update is commit point; no retry |
| F | New ADR required | Existing levels do not express persistence; do not add one prematurely | Profile identity/version must persist | Deterministic tests only | Durable write may succeed before acknowledgement; no replay |
| G | New ADR required | Existing Execute is too coarse only as outer gate; avoid new level until a policy model exists | Profile schema/version expansion for endpoint identity | Provider-specific adapter work | Remote request dispatch is uncertain; never retry mutation tools |
| H | New ADR required | Existing Execute outer gate insufficient | Profile schema/version and trusted source semantics | Provider-specific/lifecycle work | Start/update/restart outcomes may be uncertain |
| I | New ADR required | Reuse existing per-tool levels | Dynamic profile semantics/version change | Deterministic bridge lifecycle tests | Registry swap/call handoff needs explicit no-replay rules |
| J | Covered by existing ADRs | None | None | Deterministic tests only | Validates existing no-replay model |

No candidate justifies a new `PermissionLevel` for v0.7. `PermissionLevel`
continues to be an outer runtime gate; narrow host-owned policy is the actual
authorization. Adding a label such as `RepositoryWrite` would not constrain
the underlying operation unless it came with an independently designed policy.

## Proposed v0.7 capability

The implementation task should retain the name `repo.patch` and evolve its
strict input from one replacement to a bounded list:

```json
{
  "path": "crates/example/src/lib.rs",
  "expected_file_sha256": "lowercase-64-hex",
  "expected_file_byte_length": 1234,
  "replacements": [
    {
      "expected_old_text": "unique old text",
      "replacement_text": "new text",
      "expected_occurrences": 1
    }
  ]
}
```

The final schema should be decided in Task 071, including whether
`expected_occurrences` is always exactly one rather than a flexible value.
The recommended v0.7 default is exactly one match per replacement. It remains
the simplest deterministic model and prevents a request from silently editing
several same-looking sites.

### Authority and preconditions

Host construction continues to own canonical non-bare repository root, limits,
lease, Git executable, temporary names, audit evidence, and all native paths.
The model supplies a logical relative `/`-separated path, full preimage digest
and length, and literal text transforms only. The target remains one existing,
regular, UTF-8, NUL-free file, tracked in `HEAD`, with one normal stage-0 index
entry equal to `HEAD`; .git, links/reparse points, hard links when detectable,
special files, nested repositories, untracked/staged/conflicted/sparse targets,
and unsupported attributes remain refused.

All present ADR 0012 checks are re-run under the same repository lease before
any replacement. The combined postimage must respect host-fixed limits. A
reasonable initial research bound is 16 replacements and a total request/text
budget no larger than the current bounded file/input limits; Task 071 must set
exact constants from the existing implementation limits and test capacity.

### Deterministic semantics

Use **one-pass snapshot semantics**:

1. Capture and validate the original bytes, digest, length, UTF-8/BOM rules,
   repository state, and target identity.
2. Locate every replacement against the same original decoded text.
3. Require each old text to occur exactly once; duplicate old texts, repeated
   targets, empty old text, and overlapping ranges are deterministic refusals.
4. Sort located ranges by ascending byte offset and construct exactly one
   postimage. Replacement text never becomes an input to a later replacement.
5. Build/flush/revalidate one same-directory temporary file, then make one
   final replacement attempt and post-verify the exact postimage and unchanged
   repository observations.

This eliminates ordering-dependent output and makes overlap a known refusal,
not a partial operation. Existing BOM and literal newline rules are preserved;
there is no normalization, regex, glob, fuzzy matching, byte offset supplied by
the model, mode change, or binary editing.

### Failure and cancellation

The final native replacement call remains the sole content commit point. Before
it, cancellation or failed validation is a known non-mutation only when the
captured preimage is proven intact. A successful API return is not enough:
success requires post-observation of the complete expected postimage and
repository/identity invariants. After a reported replacement failure, timeout,
disconnect, cancellation at/after commit, crash, or incomplete observation,
report a known failure only if intact preimage is proven; otherwise report
`uncertain`. Never retry, replay, auto-restore, or automatically use the
captured preimage to compensate. An uncertain result requires fresh inspection
and a new model/tool request.

### Profile, bridge, tests, and live gate

The capability remains an already-authorized constructor selected only by the
trusted host profile. Its binding is a new closed capability entry, not a new
symbolic resource, profile version, or dynamic-profile behavior, assuming the
current fixed-capability representation admits it. The implementation must
verify that assumption; if it does not, Task 071 stops rather than changing the
schema incidentally.

No Generic Tool Bridge feature is required. It remains another `Tool` with a
deterministic alias, permission, deduplication, cancellation/disconnect, and
no-replay behavior. Update bridge tests only for the changed schema and
outcomes. The deterministic test plan includes strict parsing/unknown fields,
limits, duplicate and overlap refusal, one-pass ordering, duplicate old text,
CRLF/BOM preservation, every precommit race point, temporary tampering,
post-replacement identity/content races, index/HEAD races, cancellation,
Windows sharing violations/reparse/ADS/case tests, and Unix symlink/mode tests.
The live gate should use an opt-in, bounded fixture and prove exactly one
`repo.patch` call, multiple non-overlapping replacements in one tracked file,
correct final digest/content, no stage/index mutation, continued terminal
completion, and cleanup of the Codex/app-server process.

ADR 0012 explicitly excludes multi-edit. Task 071 should determine whether
the accepted decision can be amended as a narrowly specified extension or
whether repository convention requires a successor ADR. The recommendation is
an ADR 0012 amendment before implementation because its Authority excluded and
Deferred capabilities sections expressly name multi-edit; this is a decision
change, not mere code hardening. No ADR is modified in Task 070.

## Closely compared alternatives

### A. Bounded `repo.create`

ADR 0012 authorizes only a host-named temporary regular file needed to replace
an existing tracked target. It explicitly excludes model-selected persistent
file creation. Creation is therefore a materially new authority boundary, not
an implementation detail.

A future `repo.create` should require a new ADR and, at minimum: a logical path
strictly beneath canonical repository root; existing validated parent directory
(no implicit directory creation); no `.git` component, hidden control path,
ADS/colon, reserved device name, absolute/UNC/verbatim path, symlink, junction,
reparse point, or special file; a target proven absent immediately before
commit; exclusive create semantics; bounded UTF-8/NUL-free content; fixed size
limit; and no overwrite. Automatic staging should be **no**, preserving the
worktree/index/history separation. It also needs explicit treatment of a final
rename/create that succeeds before a later validation failure: that is
uncertain, not a rollback authorization. It is valuable, but should not be
bundled with B.

### C. Repository-aware edit transaction

The durable direction is a transaction language containing several existing
file edits and, eventually, controlled creates/deletes. It should not claim
filesystem all-or-nothing semantics. Same-directory replacement can make each
single target switch cleanly, but no portable atomic commit spans several
paths. A journal, RAH lease, prepare phase, and compare-before-recovery record
can improve diagnosis, not make external writers or crash recovery atomic.
After the first final rename, a failure produces a partial visible effect and
must be uncertain; automatic rollback itself requires further destructive
authority. Repository leases serialize RAH calls only, not editors, Git, or
other processes. This is v0.8+ research, after B and likely `repo.create` have
proven their isolated semantics.

### D. Unified patch/hunks

Unified patches are model-friendly and interoperable, but their useful form
introduces a patch language: path headers, file creation/deletion, rename/mode
markers, binary markers, line endings, offset context, and ambiguity. Fuzzy
hunk application must be rejected: it turns a stale model proposal into an
unverifiable mutation. A future strict patch could be an interchange syntax
that compiles to independently preconditioned repository operations, but it is
not a v0.7 parser project.

### E. `repo.commit-staged`

Commit/history authority is separate from worktree content. A narrowly scoped
future command would commit exactly the already-staged index under expected
HEAD and index-tree preconditions, with no auto-stage, amend, branch/ref choice,
history rewrite, network action, or arbitrary metadata. It nevertheless writes
objects and refs/reflogs and must specify fixed identity, hooks, signing,
GPG/SSH helpers, editor/template suppression, and failures after object/ref
updates. It needs a new ADR and should be its own milestone.

### F. Session/workflow persistence

Persistence can improve resumability but does not unblock source changes as
directly. A design must separately decide persistence of RAH session/thread
mapping, transcript, tool results, dedupe/no-replay state, pending approvals,
trusted-profile identity, and external provider state. It must not persist
secrets casually or resume stale authority after profile changes. Schema
migration, encryption/retention, crash consistency, and model-visible audit
require a new ADR.

### G--I. Network MCP, PluginManager, dynamic profile reload

Network MCP adds endpoint/DNS/TLS/redirect/proxy/credential/OAuth/server-
identity/schema-drift/reconnect and uncertain remote-effect policy; it is a
large authority increase. A PluginManager is distinct from running one
host-configured Process Plugin: discovery, install/update, executable
replacement, restart/supervision, provenance, and profile interaction are new
trusted lifecycle authority. Dynamic reload changes a static trust boundary:
trigger authorization, atomic registry handoff, in-flight calls, provider
shutdown/startup, session authority drift, replacement races, and auditability
all need fresh design. Each requires a new ADR and is deflected.

### J. Cross-platform/release hardening

This has real value and must be a release gate for B, but is not enough alone
to define v0.7 after a product-facing repository-observer release. Required
coverage includes Ubuntu live Codex validation, Linux process-group and
cancellation behavior, macOS evidence when available, Git subprocess
cancellation, symlink/mode cases, Windows sharing/reparse/ADS/reserved-name and
case-identity cases, plus external-writer stress tests. It remains a workstream
inside the B milestone, not another authority class.

## Platform implications

Windows is the primary implementation risk: native replacement can fail due to
open handles, sharing rules, antivirus/indexers, and filters; canonical string
paths are insufficient on a case-insensitive filesystem; reparse points,
junctions, ADS, reserved names, and volume/file identities require explicit
refusal and observation. No assumption of Unix `rename` behavior is valid.
Directory creation/removal and rollback are outside B.

On Unix, reject symlinks throughout traversal, preserve case-sensitive identity
and executable mode (B must not change modes), and refuse unsupported invalid
UTF-8 path bytes at the JSON/logical-path boundary. Rename can be atomic within
a filesystem for a name switch, but it is neither durability nor a multi-file
transaction; fsync/durability guarantees must be documented precisely rather
than implied. Permission-bit changes remain excluded.

## Release boundary and next task

v0.7 should contain only the ADR/design decision for B, implementation,
deterministic Windows and Ubuntu coverage, trusted-profile composition,
existing Generic Tool Bridge coverage, one bounded live Codex validation, and
security/release review. It should not combine A, C--I, or a general hardening
release.

Expected Task 071 is research/design only: specify the exact multi-replacement
schema, limits, outcome taxonomy, test matrix, and the precise ADR 0012 change
needed before implementation. It must stop rather than implement unless a
separate task explicitly authorizes implementation after that design is
accepted.
