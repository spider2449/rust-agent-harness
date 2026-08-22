# ADR 0012 — Repository worktree content mutation authority

Status: Proposed

## Context

ADR 0010 constrains intentional repository mutation to an index-only private policy. It deliberately defers file-content authoring. ADR 0011 makes trusted-capability profiles a host-owned composition boundary, but explicitly says a profile cannot create a new class of authority.

After the released v0.4.0 baseline, RAH needs a narrow way to make a deliberate source change without promoting generic filesystem write, shell/process, Git restore, or Git history authority. The v0.5 worktree-mutation research finds that an existing tracked worktree file can be edited safely enough for a first capability only when exact content preconditions, host identity checks, bounded direct replacement, and conservative uncertain-effect handling are part of a new private authority policy.

## Decision

RAH introduces a separate, private, host-owned RepositoryWorktreeMutationPolicy for bounded repository worktree content mutation. The provisional first capability name is repo.patch. This ADR authorizes no code, public API, trusted-profile schema, tool registration, Codex bridge, or live example; acceptance and a separate implementation task remain required.

PermissionLevel::Execute may remain an outer runtime gate only if the later implementation can do so without a public permission change. It is not sufficient authorization. A model ToolCall is a request; the policy remains the private host authorization.

### Authority granted

For exactly one accepted call, the policy may read one bounded target and attempt one direct worktree replacement. Trusted host construction owns the canonical non-bare repository root, limits, internal audit/preimage retention, and all internal temporary paths. The model supplies only a constrained logical relative path, a complete-file SHA-256 and byte-length precondition, literal expected old text, and replacement text.

The policy may create a uniquely named, exclusive temporary regular file in the validated target parent solely to construct and validate the complete postimage on the same filesystem before attempting replacement. This is not authority to create a model-selected or persistent user file. The temporary artifact is host-named, bounded, cleaned up when possible, and audited if cleanup is not proven.

### Authority excluded

This policy does not authorize:

- generic fs.write, arbitrary full-file writes, append, arbitrary truncation, file creation/deletion, rename/move, binary edits, alternate streams, or permissions/ACL/attribute changes;
- regex, glob, line/range, unified-diff, hunk-fuzz, multi-edit, multi-file, or automatic retry behavior;
- generic shell/process execution or any model-selected executable, argv, cwd, environment, timeout, temporary path, or backup path;
- .git internals, git add, git restore, checkout/switch, reset, clean, stash, index mutation, commit/amend, refs/history/reflogs/object database, merge/rebase, hooks, signing, editors, templates, identity, credentials, or network Git; and
- OS sandboxing, cross-process exclusivity, transactional rollback, automatic recovery, or automatic replay.

## Initial capability scope

repo.patch is one existing-file conditional literal replacement per call. It must have a bounded strict schema equivalent to:

    path
    expected_file_sha256
    expected_file_byte_length
    expected_old_text
    replacement_text

All limits are host-fixed and nonzero. The schema must reject unknown fields, NUL, oversized serialized input, and all model-supplied native/absolute paths. The only supported transformation is to replace one exact, nonempty expected_old_text occurrence with replacement_text in a captured decoded file. An empty replacement is only exact text removal; it does not delete the file or introduce an offset/length truncate primitive.

The request is a verified no-op when old and replacement text are equal; RAH must not replace the file in that case. The result is bounded and redacted: it contains status and minimal logical target/effect indicators, never private absolute paths, preimage text, temporary names, backup locations, or secrets.

## Preconditions and target restrictions

The target must be an existing regular file beneath the canonical host-selected repository root. The first version requires that it is Git-tracked in HEAD and has exactly one normal stage-0 index entry equal to its HEAD entry. This rejects untracked, ignored, added, intent-to-add, unmerged, staged, sparse, skip-worktree, submodule, gitlink, and nested-repository targets.

This tracked-only rule is deliberately narrower than all existing worktree files. It keeps local configuration/generated/scratch material outside the first content-mutation authority and makes the index/HEAD versus worktree boundary testable. A later extension to untracked/new files or staged targets requires a new authority decision.

The model path is a logical relative path with / separators and only nonempty normal components. The policy rejects absolute, drive-relative, drive-qualified, UNC, verbatim, device, . and .., backslash, colon/ADS, empty, and case-insensitive .git components. It rejects a target outside the canonical root and every link/reparse/special-file condition described below.

Under the per-repository RAH lease, before any commit attempt the policy must revalidate root/parent/target identities and require:

1. raw file length and SHA-256 exactly match the expected complete-file values;
2. strict supported UTF-8 decoding succeeds;
3. the literal expected old text occurs exactly once; and
4. the repository, parent, target, tracking, index, HEAD, and ref observations remain supported and unchanged.

Absent or multiple expected-text matches, a stale digest/length, or any failed precondition is a known refusal with no target write. The implementation must revalidate immediately before the replacement call. It must never choose a first match, apply fuzzy matching, silently refresh the request, or retry.

## Encoding and newline rules

The policy supports only bounded strict UTF-8 files without NUL. Malformed UTF-8, binary data, unsupported encodings, and oversized raw files are refused. One leading UTF-8 BOM is accepted as transport metadata, excluded from text matching, and preserved exactly. No request field may add or remove it.

Matching operates over decoded Unicode scalar values after the optional BOM; it uses no Unicode normalization, case folding, regex, or byte offsets. The whole-file digest/length covers the original raw bytes including BOM and newlines. RAH performs no implicit CRLF/LF/CR normalization. Unchanged text remains byte-identical and replacement newlines are literal request content.

## Mutation strategy

RAH rejects in-place content modification because it can expose partial target bytes during a write failure, cancellation, or crash. The policy must first construct, size-check, encode, and flush a complete postimage to a bounded exclusive same-directory temporary regular file, then perform one best-available same-filesystem replacement attempt.

On Windows, use a tested native Unicode replacement primitive rather than a shell, Git command, or cross-volume move. A successful replacement may be an atomic name/content switch for ordinary lookup purposes; that property is not a transaction. It does not guarantee metadata preservation, cross-process exclusion, rollback, recovery from every replacement failure, or that an attempted operation had no effect. Post-observation, not the API return alone, determines the RAH outcome.

## Windows behavior and TOCTOU limits

Windows is the verified release baseline. The policy must:

- canonicalize the trusted root and compare resolved native handle identities, not rely only on case-sensitive path strings;
- reject symlinks, junctions, all reparse points, mount-like redirections, and special file types at root, every traversed parent, target, and temp parent;
- reject a target with more than one hard link where reliable platform evidence is available;
- use volume plus file identity and relevant attributes through handles, while expecting a successful replacement to install a new target identity;
- use a same-volume temp in the validated parent; reject unsupported read-only/ACL/stream/encryption/compression behavior rather than repair it;
- treat sharing violations, editors, antivirus, indexers, and filter drivers as expected possible interference; and
- allow a valid tracked UTF-8 script as text without executing it. File extensions are not an execution or safety boundary.

The policy must revalidate before preparation, immediately before replacement, and after it. Its lease serializes only RAH calls. External processes can still swap paths, alter file content, hold handles, or influence filesystem filters. This decision makes no claim of TOCTOU freedom, OS isolation, or cross-process locking. Incomplete or contradictory observation fails closed as uncertain.

## Failure, cancellation, and audit

The replacement system call is the mutation commit point.

- Before that point, a validation, preimage, temporary-file, or cancellation failure is known non-mutation only when target equality with the captured preimage is proven.
- After a reported success, RAH reports success only when the exact constructed postimage, target/parent/root checks, and unchanged index/HEAD/refs are verified.
- After a reported replacement failure, RAH reports known failure only when post-observation proves the target preimage is intact. Any target delta, incomplete post-observation, lost OS result, cleanup ambiguity, timeout, disconnect, or crash is uncertain.
- Cancellation before the commit point prevents the attempt where proven. Cancellation during or after the commit point is not rollback. The event stream remains terminally cancelled; host-private audit may record a verified postimage, but an unobserved caller must treat the effect as uncertain.
- Uncertain external effects are never automatically replayed. RAH does not automatically restore a preimage; a recovery design needs separate authority and compare-before-recovery rules.

Bounded host-private preimage and audit evidence is captured before an attempt and retained under host policy. It is not model-visible and does not itself authorize restoration. Audit must redact unnecessary native paths, temporary names, file content, and secrets.

## Relationship to ADR 0010 and ADR 0011

ADR 0010 remains the private index-mutation authority. It does not authorize worktree content mutation and must not be broadened silently. The three state planes remain separately authorized:

    worktree content mutation != index mutation != history/ref mutation

ADR 0011 remains composition-only. A trusted profile may eventually bind an already implemented and hardened repo.patch constructor, but a profile entry cannot create this policy, expand its path/text limits, bypass ToolRegistry, or turn a model call into host consent.

HostExecutionPolicy, WorkspacePolicy, and ExternalToolPermissionPolicy also remain distinct: none is a substitute for this policy, and this policy does not grant their broader/different authorities.

## Deferred capabilities

The following require separate research and approval: untracked/new files, staged files, deletion/rename/move, arbitrary full-file writes/truncation, binary and ADS edits, regex/diff/range/glob/multi-file patches, link/reparse or hard-link support, Git attributes/conversions/filters, restore-worktree, Git index/history/ref/network operations, credentials/hooks/signing, generic filesystem/process authority, automatic rollback/replay, trusted-profile composition, Codex bridge, and live examples.

## Consequences

Positive consequences:

- RAH can eventually make one intentional source edit without collapsing into a general filesystem or process authority.
- Exact whole-file and unique-literal preconditions make stale and ambiguous requests refuse deterministically.
- Same-filesystem complete-postimage replacement produces a clearer commit point than in-place writing.
- Windows-specific alias, link, identity, lock, and newline behavior is an explicit supported subset rather than an assumed Unix model.

Costs and constraints:

- The capability deliberately rejects many useful editing workflows.
- File identity/reparse/locking/replacement evidence is platform-specific and needs deterministic Windows coverage before release.
- A RAH lease cannot exclude external actors; failure semantics must remain conservative.
- Host-private preimage handling contains potentially sensitive source content and needs bounded retention/diagnostic hygiene.

## Alternatives rejected

### Broaden ADR 0010

Rejected because ADR 0010's accepted contract is index-only. Worktree bytes are a distinct destructive authority and need their own cancellation, precondition, and filesystem model.

### Allow any existing untracked worktree file

Rejected for the first version because it includes ignored local configuration, generated material, and scratch data and weakens the repository-owned target proof.

### Generic fs.write or shell/process execution

Rejected because either would create broad arbitrary-path/content or ambient process authority without the repository-specific preconditions and result proof.

### In-place write

Rejected because it makes partial target bytes a normal failure possibility.

### Unified diff, git apply, regex, or multi-edit patching

Rejected because their parsing, matching, path, conversion, and partial-failure semantics are larger than a first deterministic literal replacement.
