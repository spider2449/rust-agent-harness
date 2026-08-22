# RAH v0.5 repository worktree content-mutation research

Status: Research complete; ADR 0012 is proposed and no implementation is authorized by this document
Date: 2026-08-22
Scope: bounded worktree text mutation after RAH v0.4.0. This document does not change Rust code, Cargo manifests, public APIs, trusted-profile schemas, tool registration, Codex bridges, or live examples.

## 1. Decision

Proceed to an ADR-led, narrow worktree-content authority. The first candidate is provisionally named repo.patch and is a one-file, conditional, text-only replacement capability. It is not fs.write, a shell command, or a Git mutation capability.

The capability needs a new private, host-owned RepositoryWorktreeMutationPolicy. It is a distinct state-plane authority:

    repository worktree content != Git index != Git history/refs

The policy must be accepted and implemented independently from the existing index-only RepositoryMutationPolicy. ADR 0010 remains unchanged and does not authorize byte mutation in a worktree. ADR 0011 remains a trusted-host composition boundary: a profile may eventually compose an approved capability, but cannot grant this authority or weaken its policy.

## 2. Authority boundary

RepositoryWorktreeMutationPolicy is private implementation policy in rah-tools. Trusted host construction selects one canonical, non-bare repository root and fixed nonzero limits. A model call is only a request to apply one operation inside that already selected root.

### Exact authority granted

For one accepted request, the policy may:

1. read and validate one existing, bounded, Git-tracked regular worktree file;
2. decode its bounded bytes as supported UTF-8 text;
3. verify the complete-file precondition, the unique literal old-text precondition, repository/parent/target identity, and index/HEAD/ref invariants;
4. create one host-named, exclusive, same-directory temporary regular file containing the complete constructed replacement, then attempt one replacement of the authorized target; and
5. retain bounded, host-private preimage/audit evidence and report a bounded, redacted outcome.

The temporary file is an internal implementation artifact, not model authority to create a user-selected file. Its name, directory, contents, lifetime, and cleanup are host-owned. A failed cleanup is reported; it is never hidden as a successful user-file creation.

### Boundaries that remain separate

| Boundary | Why it does not authorize repo.patch, or vice versa |
| --- | --- |
| RepositoryMutationPolicy / ADR 0010 | It authorizes an index-only, host-targeted mutation prototype and later stage/unstage semantics. It must not gain worktree-byte semantics. repo.patch must preserve the index. |
| HostExecutionPolicy / ADR 0009 | It constrains one host-selected child process. repo.patch directly mutates a validated file and grants no executable, argv, cwd, environment, shell, or process authority. A later fixed Git observer, if needed to prove tracking, remains separately constrained by ADR 0009. |
| WorkspacePolicy | It supplies existing read-oriented path-validation lessons only. It is not a write authorization, identity lease, replacement policy, or OS sandbox. |
| ExternalToolPermissionPolicy | It assigns a RAH permission to an admitted external-tool identity. It neither grants a built-in worktree edit nor lets an external tool inherit this private policy. |
| Git history/ref mutation | repo.patch must not intentionally mutate the index, HEAD, refs, reflogs, object database, hooks, author identity, or signatures. |
| Network Git | No fetch, pull, push, remote selection, credential helper, or network authority follows from editing bytes. |

PermissionLevel::Execute may remain the coarse outer runtime gate if the implementation plan proves that no public permission change is needed. It is necessary at most, never sufficient: the private policy is the actual capability-specific authorization.

## 3. Initial capability envelope

The provisional model-facing operation is a single literal replacement, not a unified diff and not a full-file write:

    path + expected_file_sha256 + expected_file_byte_length
         + expected_old_text -> replacement_text

The host fixes the digest algorithm (SHA-256 for the first design), all limits, the repository root, audit/preimage retention, and result vocabulary. The request carries no absolute native path, executable, command, temporary path, backup path, Git revision, policy override, or retry instruction.

The input schema remains provisional until implementation, but must have only the fields above, reject unknown fields and NUL, and bound serialized request bytes before parsing. expected_old_text is nonempty and replacement_text is bounded. An empty replacement is allowed only as removal of that uniquely matched nonempty literal text; it is not a file deletion or an offset/length based arbitrary-truncate operation.

One call may touch one path and construct one complete postimage. The policy must reject:

- file creation, file deletion, rename/move, append mode, chmod/ACL/attribute repair, generic full-filesystem writes, and model-visible temp/backup paths;
- binary or malformed text; regex, glob, hunk fuzz, offset/range, multi-file, or iterative replacement; and
- .git internals, index/history/ref mutation, restore-worktree, generic shell/process execution, and network Git.

The temporary same-directory file is the only tightly bounded internal file creation necessary to avoid in-place partial content writes. It is not a general exception to the file-creation exclusion.

### Why literal replacement rather than the earlier whole-file-write baseline

Whole-file conditional write has simple content construction but makes small model changes routinely resend a file and can make accidental truncation too easy. Unified diffs, git apply, regex, and edit lists add path headers, fuzz, ambiguous offsets, partial multi-edit semantics, or Git behavior. One exact unique substring replacement has a smaller request while retaining a complete file snapshot precondition and all-or-nothing postimage construction.

## 4. Conditional precondition and stale-request model

Before an attempted replacement, under the per-repository RAH lease, the policy must recapture the target through the validated path and require all of these:

1. the raw file length and SHA-256 exactly equal the model request's expected complete-file values;
2. strict decoding succeeds under the UTF-8/BOM rule below;
3. expected_old_text has exactly one literal occurrence in the decoded matchable text; and
4. repository root, every traversed parent, the target type/identity, and the tracked/index state remain supported and unchanged.

The constructed postimage is the captured text with that one occurrence replaced. No matching uses regex, Unicode normalization, locale-specific case folding, line numbers, or byte offsets. The raw expected digest catches a stale request even if the requested old text still happens to occur once after an unrelated edit.

| Condition | Required result before the replacement commit point |
| --- | --- |
| Old text absent | failed_known; no target write. |
| Old text occurs more than once | failed_known; no guessing, first-match choice, or retry. |
| Complete digest/length mismatch, including a stale request | failed_known; the caller must read again and make a new request. |
| Identity, tracked state, parent, or supported-file check changes | failed_known when no attempt occurred; otherwise follow uncertain-outcome rules. |
| Replacement text equals old text | Verified no-op after all preconditions; do not replace the file. |

Immediately before the replacement system call, the policy must re-open or revalidate the target and parents and recapture the same complete preimage. It then rechecks the digest, unique match, and identities. This narrows races; it does not remove the interval after validation.

## 5. Tracked versus untracked files

Recommendation: require an existing Git-tracked regular file that has one normal stage-0 index entry and an entry in HEAD, with the index entry matching HEAD for the first version. The target worktree may be dirty only when its current full-file precondition matches exactly. This deliberately rejects newly added, intent-to-add, unmerged, sparse/skip-worktree, submodule/gitlink, and index-staged targets.

This is narrower than allowing any existing worktree file. It avoids quietly editing ignored or untracked local configuration, generated artifacts, credentials, scratch data, and repository-adjacent metadata; gives a clear repository-owned target population; and lets the first test matrix prove that the unchanged index and HEAD are distinct from changed worktree bytes. The cost is that an agent cannot create a new source file or modify an already staged file through v0.5. Those are deliberate future authority decisions, not fallbacks to fs.write.

Tracking proof is an internal host observation, not authority to mutate Git. If it invokes Git, it must use a separately fixed host observer under ADR 0009 with no model-controlled argv/cwd/environment and no mutating Git command.

## 6. Windows filesystem restrictions

Windows is the verified release baseline. The implementation must use native handle/identity evidence where the platform requires it rather than assuming Unix rename or string-path behavior. The following are mandatory first-version restrictions and test targets.

| Topic | First-version rule |
| --- | --- |
| Root and path spelling | Canonicalize the host-selected repository root. Accept only a repository-relative logical path with / separators and nonempty normal components; reject current-directory and parent-directory components. Never concatenate a model path into a native absolute path without component validation. |
| Absolute, drive, UNC, verbatim, device paths | Reject a leading separator, drive-qualified/drive-relative forms, UNC (\\server), verbatim (\\?\), device (\\.\), and every native prefix. Windows path prefixes change namespace interpretation and must not cross the model boundary. |
| Case and aliases | Treat path comparison as case-insensitive on the Windows baseline, but rely on resolved handle identity rather than string casing. Reject aliases/case collisions or 8.3/other names that cannot be tied to the expected target identity. |
| Traversal, symlinks, junctions, reparse points | Reject any reparse point or symlink at the root, any traversed parent, target, and the temporary-file parent. Inspect the link object rather than transparently following it where platform APIs permit. Junctions are reparse points and can redirect a path outside the root. |
| Hard links | Require a link count of one where the filesystem reliably reports it. Reject more than one link: a replacement otherwise changes one name while aliases retain the old file, which is surprising and weakens the single-target reasoning. |
| ADS and special names | Reject : in a model path component, including alternate-data-stream syntax, and reject reserved/device path forms. The capability addresses only the default stream of one ordinary file. |
| File identity | Capture volume plus file ID and relevant regular/reparse/link-count attributes through handles before and after. A successful replacement is expected to install the temporary file's identity; postvalidation must bind that new identity to the authorized logical target, not demand the old file ID. |
| Target/parent revalidation | Revalidate root, parents, target, and parent directory identities before preparation, immediately before replacement, and after. A RAH lease serializes only RAH, not external processes. |
| Locks and interference | Sharing violations, open-file locking, antivirus, indexers, editors, and file-system filters are normal outcomes. Do not retry automatically; a known unchanged target is failure, and incomplete/contradictory observation is uncertain. |
| Temporary replacement | Create the unique temp on the same volume in the validated target parent with exclusive creation. Do not use cross-volume copy/move. Preserve only explicitly supported metadata; read-only attributes, ACLs, encryption, compression, or streams that cannot be proven preserved cause refusal or failure, never attribute repair. |
| Executable/script files | No process is started and no extension implies special execution behavior. A valid tracked UTF-8 regular script is not inherently distinguishable from source text, so it is within the content boundary if all rules pass. This is not authority to execute it; hosts needing a narrower surface must bind an allowlist in a later accepted design. |
| CRLF | Treat CRLF, LF, and mixed line endings as literal text bytes. Preserve every untouched byte and perform no implicit newline conversion. |

Windows file namespaces permit UNC, device, and verbatim forms; the verbatim prefix changes normal string parsing. Windows also advises applications not to assume case sensitivity. [Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file) documents both properties. Reparse points and symlinks can cause file operations to target behavior outside ordinary path expectations, so the first scope rejects them rather than trying to support an unbounded set of filter semantics. [Reparse Points and File Operations](https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points-and-file-operations) and [Symbolic Link Effects on File System Functions](https://learn.microsoft.com/en-us/windows/win32/fileio/symbolic-link-effects-on-file-systems-functions) support that conservative rule.

NTFS hard links let multiple names address one file and changes through one name appear through the others. [Microsoft's hard-link documentation](https://learn.microsoft.com/en-us/windows/win32/fileio/hard-links-and-junctions) therefore supports rejecting multi-link targets.

## 7. Mutation strategy

| Strategy | Strength | Failure surface | Decision |
| --- | --- | --- | --- |
| A. In-place content modification | No transient sibling file. | A crash, cancellation, disk error, or external observer can see a partially written file. Restoring a preimage would itself be a second destructive write. | Reject. |
| B. Construct complete postimage in a bounded same-filesystem temporary file, then replace target | The target remains untouched until the replacement call; the full postimage can be size/UTF-8/digest checked before the commit attempt. | Replacement may be blocked by sharing/ACL/filter conditions, and a failed call can still require post-observation to classify. | Recommend. |

The recommended strategy is B using the strongest supported same-volume native replacement primitive. On Windows that should be a direct Unicode native API with behavior explicitly tested on the supported filesystems, not a shell, cmd, PowerShell, git, or a cross-volume copy. ReplaceFileW is relevant because it replaces an existing file and requires all participating files to be on one volume. Its documented error cases also show that a failed replacement can leave different names/states in particular cases. [ReplaceFile documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilea)

Atomic replacement here means only a successful replacement primitive can avoid exposing an in-place sequence of partial target bytes to ordinary name lookups. It is not transactional rollback: it does not make preimage restoration automatic, preserve every metadata/stream/ACL property without proof, undo a successful replacement after cancellation, or make a failed OS operation globally all-or-nothing. RAH must classify from post-observation and never issue a compensating write automatically.

## 8. UTF-8, BOM, and newline rules

1. Read at most the policy's bounded raw-byte limit. Reject malformed UTF-8, NUL-containing data, unsupported encodings, or a file whose size cannot be fully captured within the bound.
2. Accept at most one leading UTF-8 BOM (EF BB BF). It is transport metadata: remove it before decoded-text matching, preserve it exactly if present, and do not let either request text field add/remove it.
3. Match expected_old_text and form the replacement over decoded Unicode text after that optional BOM. Text equality is exact scalar-value equality; no regex, normalization, case folding, or byte offset is used.
4. Encode the constructed text as UTF-8, prepend the preserved BOM if any, and bound the resulting raw byte length before writing the temporary file.
5. Do not normalize CRLF, LF, CR, or mixed newline styles. Unchanged regions retain their exact original code units; the replacement text's supplied newlines are literal authored content.

The full-file digest and length always cover the exact raw bytes, including a leading BOM and every newline code unit. This makes CRLF and BOM behavior observable to the stale-request guard without leaking file content in output.

## 9. Git boundary

repo.patch is direct worktree-content mutation only. It does not imply authority for any of the following:

- git add, git restore, checkout/switch, reset, clean, or stash;
- file creation/deletion/move by Git, index entries, or Git attributes/filter conversion;
- commit/amend, refs, history, reflogs, object creation, merge, rebase, or submodule operations; or
- fetch, pull, push, remotes, credentials, credential helpers, hooks, signing, editors, templates, or network Git.

The policy must pre/post-observe enough Git state to reject an unexpected index, HEAD, or ref delta. That observation is a guard, not permission to repair or roll it back. Worktree editing and index/history mutation remain separate authority classes even when a later workflow deliberately invokes both.

## 10. Failure, cancellation, and replay

The replacement system call is the mutation commit point. The policy reports bounded reason codes and host-private audit evidence, not raw path internals or preimage contents.

| Event | Required behavior |
| --- | --- |
| Validation/preimage/temp construction fails before commit point | Clean up the private temp where possible; return known failure only if the target remains the captured preimage. No target mutation. |
| Unique expected text is matched and replacement succeeds | Recapture root/parents/target, raw postimage, target type/identity, index, HEAD, and refs. Report success only if the expected postimage is proven and all excluded state is unchanged. |
| Replacement reports failure | Recapture state. If full evidence proves the target unchanged, report known failure. Any target difference, missing temp/target, cleanup failure with ambiguous state, or observation failure is uncertain. |
| OS-level outcome is lost or contradictory | Report uncertain; retain audit/preimage evidence subject to host policy. Never retry, replay, or auto-restore. |
| Cancellation before commit point | Stop before replacement, clean up best-effort temp, and report cancellation/known non-mutation only when that is proven. |
| Cancellation races or occurs during replacement | Do not assume the replacement was prevented. Attempt no replay or rollback; post-observe if lifecycle ownership permits, otherwise classify as uncertain. |
| Cancellation after commit point | Cancellation never means rollback. The AgentEvent layer remains terminally cancelled; host audit may record a verified postimage, but any caller that did not receive that proof must treat the effect as uncertain. A later attempt needs a fresh request/precondition. |

The existing RAH rule remains unchanged: uncertain external effect is never automatically replayed. Cancellation is not a transaction and does not imply rollback.

## 11. Abuse and security review

| Case | Required defense |
| --- | --- |
| Path escape / outside workspace | Host-owned canonical repository, strict relative-component grammar, parent/root identity checks, and final containment proof. |
| Symlink/reparse swap | Reject reparse points/links before and at revalidation; use handle identity where possible; fail closed on any mismatch. |
| Hard-link alias | Reject multi-link target. |
| .git or metadata edit | Case-insensitive .git component denial plus tracked regular-file/HEAD/index checks; never write metadata. |
| Oversized file/input/result | Bound raw file, request, old/replacement text, constructed postimage, audit/preimage, and serialized redacted result independently. |
| Malformed UTF-8/binary | Strict decode and NUL rejection before construction. |
| Absent/ambiguous expected text | Require exactly one literal occurrence; otherwise no attempt. |
| Stale expected content | Require exact complete raw length/digest immediately before replacement. |
| Validation-to-replacement race | Revalidate immediately before the call and postvalidate after it; classify residual races conservatively. |
| Untracked/ignored sensitive file | Require a normal tracked HEAD/index target. |
| Generic-write creep | Keep one semantic replacement operation private to its policy; forbid creation, arbitrary paths, full-file write, glob, multi-file, shell, and Git mutation. |

### TOCTOU limit

This design narrows but cannot prove away time-of-check/time-of-use races. An in-process lease serializes only RAH calls. Another process can alter a path, directory, file, handle sharing state, or filter behavior between validation and the OS replacement call. Handle identities, parent checks, same-directory temps, and post-observation make the window detectable in many cases; they do not provide cross-process exclusion, OS sandboxing, or TOCTOU freedom. The ADR must preserve this as a non-guarantee.

## 12. Required deterministic implementation evidence

Task 048 should first establish a private foundation and deterministic tests; it must not add trusted-profile composition or a live Codex bridge. Tests should cover normal one-replacement success, verified no-op, exact-file-digest stale refusal, absent/multiple old text, malformed input/UTF-8/BOM, CRLF/LF/mixed newlines, size bounds, tracked/index/HEAD requirements, path forms, .git, links/reparse points, hard links where representable, parent/target swaps, locked files, temporary-file cleanup, replacement failure, post-state violation, cancellation, and no-replay outcomes.

The normal test suite must use owned temporary repositories and no network, credentials, real model, GPU, or live Codex process. Windows tests must prove the claimed Windows behavior on the supported filesystem; Unix coverage is not evidence for Windows path, lock, or replacement semantics.

## 13. Deferred capability classes

Defer all of the following until separately researched and accepted:

- untracked/new files, existing staged targets, file deletion, rename/move, arbitrary truncation/full-file write, permissions/ACL/attribute changes, and binary or alternate-stream editing;
- regex, diff/hunk, line/range, glob, directory, batch, and multi-file edits;
- symlink, junction, reparse, hard-link, sparse, submodule, nested-repository, linked-worktree, and Git-conversion/filter support;
- restore-worktree, index mutation, commit/history/ref/reflog/object mutation, hooks/signing/identity, credentials, and network Git;
- generic filesystem write, generic process/shell execution, OS/network isolation claims, automatic rollback, and automatic replay; and
- trusted-profile schema/composition, Codex bridge, live examples, and release validation for this new capability.

## 14. ADR recommendation

The research supports creating ADR 0012 — Repository Worktree Content Mutation Authority with status Proposed. It records this new private policy and its deliberately small initial authority without changing ADR 0010 or ADR 0011. Acceptance is required before implementation.
