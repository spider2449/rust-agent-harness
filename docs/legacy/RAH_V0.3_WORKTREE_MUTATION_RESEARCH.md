# RAH v0.3 worktree-mutation authority research

Status: Research complete; no implementation is authorized by this document
Date: 2026-08-22
Scope: after verified `host.git.stage` and `host.git.unstage`; production code is unchanged.

## Decision

**Stop the v0.3 capability line at index-only mutation.** Release v0.3 with
the hardened Execute foundation, `host.cargo.version`, `host.git.status`,
`RepositoryMutationPolicy`, and the live-verified `host.git.stage` and
`host.git.unstage` capabilities.

`host.git.restore-worktree` is a useful future capability, but it is a **new
authority class**: it can overwrite or remove a user's uncommitted worktree
bytes. It is not the index-only inverse of staging. `host.git.commit` is still
larger: it adds history, refs, reflogs, object creation, identity/signing, and
hook authority. No smaller worktree mutation offers enough value to justify
crossing the destructive-content boundary during v0.3.

Before any worktree overwrite is implemented, write and accept **ADR 0011**.
ADR 0010 deliberately says mutation is not atomic and promises no rollback;
it authorizes the index-only prototype's security model, not a contract for
destroying recoverable user bytes. A new ADR must decide the destructive
authorization, backup/recovery, and uncertain-result semantics before code is
written.

Git documents that `restore` writes the worktree by default, uses the index as
its default source unless `--source` is given, and removes a tracked path that
is absent from the source. [git-restore](https://git-scm.com/docs/git-restore)

## Authority delta

The current capabilities have this bounded authority:

```text
host.git.stage / host.git.unstage
  Execute + host-owned repository/target + lease + state proof
  -> one index entry (and its temporary index lock)
  -> no worktree bytes, HEAD, refs, reflogs, object database, hooks, network,
     or credentials
```

The candidate adds a separate data-destruction capability:

```text
host.git.restore-worktree
  same foundations
  + replace or remove one host-owned worktree file from an approved Git source
  + invoke checkout conversion/filter behavior needed to materialize the file
  + preserve a recoverable preimage before the first destructive spawn
```

`PermissionLevel::Execute` remains necessary, but it is not sufficient
authorization for either intentional repository mutation or destructive byte
replacement. `RepositoryMutationPolicy` remains private and host-owned; a
model tool call is only a request.

### State-plane comparison

| State plane | Stage / unstage | One-file restore-worktree | Commit |
| --- | --- | --- | --- |
| Index | One target entry | Must remain unchanged | Read staged set; writes index lock/metadata as needed |
| Worktree bytes | Must remain unchanged | **Target may be replaced or removed** | Usually unchanged, but hooks may mutate it |
| HEAD | Unchanged | Unchanged | Advances current branch / HEAD relationship |
| Refs | Unchanged | Unchanged | Current branch ref changes |
| Reflogs | Unchanged | Unchanged | Branch/HEAD reflog entries are written |
| Object database | Unchanged | Unchanged in the normal operation | New blob/tree/commit objects may be written |
| Hooks / external helpers | No documented restore hook; existing Git process still is not a sandbox | No documented restore hook, but checkout conversion can run configured filter processes | Commit hooks, signing helpers, editors/templates, and other ambient helpers are material |
| Network / credentials | No Git network command; prompt disabled | Same, except a configured smudge filter can itself use network/credentials | Git hooks/signing/credential helpers can use ambient host authority; no remote command is required for the risk |

The table describes intended effects, not a claim that a local Git process is
filesystem- or network-isolated. Repository-local configuration and content are
untrusted process inputs.

## Candidate comparison

| Candidate | Additional authority | Assessment |
| --- | --- | --- |
| Stay index-only for v0.3 | None | **Recommended release decision.** Proven useful correction path (`stage`, `unstage`) without overwriting user bytes. |
| `host.git.restore-worktree` for one host-owned tracked regular file | Replace/remove one file's bytes from a fixed Git source | Useful later, but requires ADR 0011 and a destructive-recovery design. Defer. |
| `host.git.commit` | Create objects; move ref/HEAD; reflog; author/committer/signing/hook authority | Defer beyond v0.3. This is history creation, not a small follow-up. |
| `host.git.checkout-index` / `git checkout -- <path>` | Also materializes worktree bytes, with less clear intent or broader checkout semantics | Reject; no safety improvement over explicit `restore --worktree`. |
| File mode/readonly-bit repair | Mutates filesystem metadata and is platform-specific | Reject; little model-facing value and still a new worktree mutation class. |
| `git update-index --refresh` | Index stat-cache update only | Not useful enough to expose; index-only but no user workflow. |

## Narrow future operation (not authorized now)

If ADR 0011 accepts a narrow design, the candidate must stay host-constructed:

```text
tool name: host.git.restore-worktree
model schema: {"type":"object","properties":{},"additionalProperties":false}
trusted constructor inputs: canonical native Git executable, canonical repository,
symbolic target, one canonical tracked regular-file path, fixed source HEAD
```

The sole mutating argv should be:

```text
git --literal-pathspecs restore --worktree --source=HEAD -- <host-owned-relative-target>
```

An explicit `--source=HEAD` is safer than the default index source: it makes
the expected source stable and independently observable, and prevents an
unrelated staged version from becoming an implicit destructive source. The
model must not provide a revision, path, pathspec, executable, argv, cwd,
environment, timeout, or a confirmation token.

This command must be rejected for a target missing from `HEAD`: Git restore
would remove a tracked destination absent from the source, which is a distinct
delete authority. First scope should require one stage-0, regular `HEAD` blob
and one extant regular worktree file; no deletion, creation, mode change,
gitlink, submodule, sparse entry, linked worktree, or nested repository.

`--literal-pathspecs` and `--` are required even though the path is host-owned.
They prevent pathspec grammar from becoming a second selection language. Never
offer `--pathspec-from-file`, `--ignore-skip-worktree-bits`, `--overlay`,
`--recurse-submodules`, `--ours`, `--theirs`, `--merge`, or patch mode.

## Required destructive authorization and recovery policy

### Authorization

Do not treat `PermissionLevel::Execute` as consent to discard content. ADR 0011
should require an additional **host-owned destructive-worktree authorization**
inside the private mutation policy. It need not add a public `PermissionLevel`
or change RAH protocol types, but it must bind all of these before spawn:

- repository/root and Git metadata identities;
- one symbolic target and its canonical file identity/type;
- source `HEAD` object and target source blob/mode;
- exact target preimage hash and byte length;
- index entry, `HEAD`, refs, relevant attributes/configuration fingerprint, and
  target/parent identity observations;
- a monotonically unique authorization/lease record with a short expiry; and
- an already durable, private recovery snapshot that matches the preimage.

The authorization is single-use. Any changed observation, expired lease,
failed identity recheck, lock contention, or concurrent modification between
authorization and final spawn must refuse before destructive execution. The
in-process per-repository lease serializes RAH only; it cannot prevent an IDE,
editor, Git, antivirus, or another process from racing RAH.

### Preimage and recovery contract

Before spawn, capture the exact raw worktree bytes into a RAH-private durable
backup owned by the host, together with a hash, byte length, target identity,
authorization id, and retention deadline. Bound it to one regular file and a
strict maximum size. If the preimage cannot be durably recorded and verified,
the operation must not start.

The backup is a recovery artifact, not a new model-visible read/write surface:
the model receives only bounded status and symbolic target. It should be kept
outside the repository and workspace, access-controlled like application state,
encrypted at rest where the host's existing secret/data policy requires it,
and deleted only after the documented retention period or an explicit trusted
host cleanup action. A hash alone is not recovery.

Return a recoverable result contract, for example:

```json
{
  "status": "ok | no_op | failed_known | policy_violation | uncertain",
  "target": "host-symbolic-target",
  "changed": true,
  "recovery": "available | not_needed | unavailable",
  "authorization_consumed": true,
  "retry_permitted": false
}
```

Do not return the backup path, bytes, source path, executable path, or Git
configuration. A trusted host-facing audit record may retain those details.

Automatic rollback is never safe: it is a second destructive write that can
overwrite a newer external edit, and it may require reverse clean/smudge/
encoding conversion. Recovery must be an explicit, separately authorized,
compare-and-restore host operation that first proves the target still has the
observed postimage and same identity. If that proof fails, retain the backup and
report manual recovery required; never overwrite blindly.

Refuse a target with both staged and unstaged state in the first design. A
`HEAD` restore would discard the unstaged bytes while the index retains a
different staged version, producing a surprising three-way state. This is not
necessary for the first narrowly authorized operation and should be deferred.

## Verification model

Hold the repository mutation lease from authorization capture through durable
audit construction. Capture post-state even if process supervision reports a
failure, timeout, cancellation, overflow, disconnect, or lost response.

### Preconditions

1. Revalidate canonical root, `.git` layout/identity, native Git executable,
   target parent and target identity; reject symlinks/reparse points in the
   target path.
2. Require a non-bare, non-linked, non-sparse repository layout; one tracked
   stage-0 regular index entry; matching `HEAD` regular blob; existing regular
   target; and no unmerged entries, gitlinks, submodules, or nested repository
   under the target.
3. Capture complete index, `HEAD`, refs, and a bounded full-worktree snapshot
   (or an equivalent complete observable proof); capture target raw bytes/hash,
   metadata/identity, and source tree entry/blob id.
4. Reject any target whose index entry differs from `HEAD` (staged state), or
   whose exact worktree bytes differ from the expected `HEAD` materialization
   (unstaged state). For the initial capability this makes restore a verified
   no-op only; therefore it illustrates why v0.3 should not add it. A useful
   later design must consciously relax the latter condition while retaining the
   backup and destructive consent contract.
5. Fingerprint relevant repository-local configuration and attributes before
   source materialization; reject unsupported filters/conversions rather than
   attempting to predict them.

There are two viable later scopes. The safest first executable scope is
**no-op-only validation** (not useful enough to ship) or a target whose
preimage differs from `HEAD` only after a host has captured backup and granted
destructive authorization. The latter is the meaningful capability and is why
ADR 0011/recovery comes first.

### Success postconditions

Success requires all of the following, not just exit code zero:

- post target is the same regular-file identity/type (or an explicitly
  supported atomic replacement identity contract) and its bytes/hash equal the
  expected worktree materialization of the captured `HEAD` blob;
- complete index is byte/entry-equivalent to pre-index;
- `HEAD`, every relevant ref, and captured `HEAD`/current-branch reflog bytes
  are unchanged; no residual lock exists;
- unrelated worktree files are byte-identical and target parent/root/Git
  identities remain valid;
- the fixed source blob/mode and the repository/configuration fingerprint still
  match the authorization; and
- the preimage backup remains present and verified until retention completes.

The expected worktree bytes are not always raw blob bytes. Git can apply text
conversion, ident substitution, `working-tree-encoding`, and smudge filters
during checkout. A future implementation should either reject any target with
these attributes/configuration, or generate and verify an expected
materialization inside an isolated, host-owned scratch worktree with the same
captured Git configuration. The first is substantially safer for a v0.3-era
implementation.

The operation has no intended object-database write, but RAH should not claim
to prove the whole object database unchanged by hashing `.git/objects`: that is
expensive, races with legitimate Git maintenance, and cannot attribute a delta
to this child. ADR 0011 should specify a bounded metadata observation (for
example, no new target object is expected and selected object-store changes are
an `uncertain` result) while retaining the stronger externally visible proof:
unchanged `HEAD`, refs, reflogs, index, and unrelated worktree. A repository
whose object database cannot be observed under that policy must be refused, not
described as verified.

## Result, timeout, cancellation, and process-loss semantics

| Outcome | Meaning | Required behavior |
| --- | --- | --- |
| Known success | Process and full post-state prove exactly the authorized target bytes, all other invariants, and a valid backup | Return `ok`/`no_op`; retain recovery artifact by policy. |
| Known no-op | Exact pre/post state proves target already had the expected bytes | Return `no_op` with no backup required or remove an unused verified backup. |
| Known failure | Spawn fails before child start, or Git reports failure and post-state proves every relevant plane unchanged | Return `failed_known`; authorization is consumed if spawn was attempted. |
| Partial / policy violation | Post-state proves an unauthorized or extra delta, including external concurrent write | Return error, preserve backup/audit, prohibit retry and automatic rollback. |
| Uncertain | Timeout, cancellation, output overflow, disconnect/crash/lost result after spawn, failed post-observation, or contradictory observations | Return error with recovery available where backup exists; no retry/replay/automatic rollback. |

Worktree restore may have written the target before RAH observes the result.
Termination is only a best-effort attempt to stop a process, not rollback. A
retry can overwrite a newer user edit and is therefore prohibited even after a
known failure once spawn occurred. A fresh host authorization is required for
every new attempt.

## Filesystem and Git-configuration risks

| Risk | Required first-scope treatment |
| --- | --- |
| Symlinks, Windows junctions/reparse points, ADS, UNC/verbatim/drive aliases | Reject root/target path links and reparse points; use canonical identities, not string prefixes. Reject unsupported path components and non-regular file types. |
| Hard links | Treat as unsupported unless the host can prove and accept that overwriting the target changes every linked name. Initial scope should reject link count greater than one where reliable. |
| Concurrent editors/external writers | Lease only serializes RAH. Revalidate immediately before spawn and after; any mismatch is refusal/uncertain. Never overwrite during recovery without postimage comparison. |
| Windows sharing violations, antivirus/indexers | Report known failure only when full post-state is unchanged; otherwise uncertain. Do not retry automatically. Test locked-file and delayed-write behavior. |
| Case-insensitive equivalence / short names | Canonicalize and compare platform-aware identities; one host symbolic target maps to one canonical relative path. Reject aliases/case collisions that cannot be uniquely proven. |
| Permissions, read-only attributes, ACLs | Capture metadata. Do not broaden capability to chmod/attribute repair. Any Git-induced permission/type mismatch is failure/violation. |
| Line endings, text/ident conversion | Reject target when attributes/config make expected bytes nontrivial in first implementation. Never verify only blob object id. |
| `.gitattributes`, clean/smudge/process filters | Attributes can cause external filter processes on checkout; reject `filter`, `text`, `eol`, `ident`, and `working-tree-encoding` for first scope unless an ADR-approved hermetic materialization policy exists. |
| `core.autocrlf`, `core.eol`, `core.safecrlf` | Repository/local configuration can change checkout bytes or cause refusal. Pin/fingerprint and initially require no conversion. |
| Sparse checkout / skip-worktree | Reject. Git documents sparse restore behavior and skip-worktree state changes; never use `--ignore-skip-worktree-bits`. |
| Submodules/gitlinks | Reject; recursive worktree restore can overwrite submodule modifications and detach its `HEAD`. Never pass recurse-submodule options. |
| Hooks | `git-restore` has no documented hook. This does not cover checkout conversion filters; commit requires a separate hook authority analysis. |

Git's attribute documentation confirms that checkout smudge commands receive
the blob and produce the worktree file, and that working-tree encodings are
re-encoded on checkout. [gitattributes](https://git-scm.com/docs/gitattributes)
Git's configuration documentation also warns that line-ending conversion can
be irreversible for mixed-line-ending content. [git-config](https://git-scm.com/docs/git-config)

## ADR 0011 decision

**ADR 0011 is warranted before implementation, but should not be drafted in
this task.** It must record:

1. that worktree byte replacement/removal is a distinct authority class from
   index mutation and from broad Execute;
2. host-owned destructive authorization, single-use binding, and stale-state
   refusal requirements;
3. preimage backup location, bounds, retention, encryption/access expectations,
   and model-output redaction;
4. known/partial/uncertain result semantics, no replay, and no automatic
   rollback;
5. explicit/manual recovery authorization and compare-before-recover rule;
6. first supported filesystem/Git-layout/configuration subset; and
7. that commit/history/ref/object/reflog, hooks, signing, credentials, and
   network remain separate future authority classes.

ADR 0010 is still correct for repository mutation generally, but it does not
make a destructive data-loss contract. Extending it silently would change the
security model it was accepted to constrain.

## Release recommendation and deferred risks

Release v0.3 after the verified index-only milestone. This produces a coherent,
valuable capability set with no unbounded user-content overwrite surface.

Defer to a post-v0.3, ADR-led worktree-mutation phase:

- destructive-consent UX/host API and backup storage lifecycle;
- exact Windows identity/hard-link/reparse/ACL behavior and crash recovery;
- hermetic handling or explicit rejection of filters, encodings, and line
  endings;
- sparse checkouts, linked worktrees, submodules, missing-source deletion,
  file mode changes, and non-regular files;
- robust whole-worktree proof for repositories larger than the present bounded
  snapshot; and
- all commit/ref/history/object/reflog, signing, hook, credential, and network
  authority.
