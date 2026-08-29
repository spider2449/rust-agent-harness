# Task 133 — Repository Commit Authority Research

Status: research complete; no implementation

## Baseline and method

- Starting `HEAD`: `f8c2e4da835f0167a3ad35440fa825d501ba1bde`
- Starting `origin/master`: `f8c2e4da835f0167a3ad35440fa825d501ba1bde`
- Initial worktree: only untracked `.vscode/`, left untouched.
- Workspace: 12 Cargo packages, version `0.10.0`, edition `2024`.
- Native Git selected on this Windows host: `C:\Program Files\Git\cmd\git.exe`, Git for Windows `2.55.0.windows.4`.
- CI workflow uses `ubuntu-latest` and does not pin or print a Git version. No recent-run artifact in this checkout establishes an exact Ubuntu Git version; Task 134/implementation must not depend on one.

This document distinguishes **DOCUMENTED** behavior from the linked official
manuals, **OBSERVED** behavior from disposable local Git-for-Windows fixtures,
and **INFERRED** design conclusions. Fixtures were created below `C:\Temp`, had
no remotes or credentials, and never used the RAH checkout.

Official sources: [git-commit](https://git-scm.com/docs/git-commit),
[git-config](https://git-scm.com/docs/git-config),
[githooks](https://git-scm.com/docs/githooks),
[git](https://git-scm.com/docs/git),
[git-symbolic-ref](https://git-scm.com/docs/git-symbolic-ref),
[git-update-ref](https://git-scm.com/docs/git-update-ref),
[git-write-tree](https://git-scm.com/docs/git-write-tree),
[git-diff](https://git-scm.com/docs/git-diff),
[git-ls-files](https://git-scm.com/docs/git-ls-files), and
[git-show](https://git-scm.com/docs/git-show).

## Existing RAH foundations

Reusable unchanged:

- `HostExecutionPolicy` already binds an absolute native executable, its file
  identity, canonical CWD, exact argv, cleared/replaced child environment,
  timeout, bounded output, and revalidation. It explicitly does not claim an
  OS or network sandbox.
- `RepositoryObserver::RepositoryIdentity` captures canonical root plus native
  filesystem identities for root and `.git`, revalidating before use.
- `git_support` already disables system/global config, fsmonitor and untracked
  cache, optional locks, and terminal prompting; observer-only configuration
  adds exactly one canonical `safe.directory` value.
- `GitIndexMutationPolicy` owns the existing per-canonical-root async lease;
  stage, unstage, observers, patch, create-file, and edit-files participate.
- Existing mutation tools already use pre/post observation and conservative
  `ok` / known failure / uncertainty patterns.

Needed later, not present now: a private commit-specific policy, a commit
state snapshot/parser, commit identity policy, an owned empty hooks directory,
commit-specific config/environment, checked command shapes, commit result
vocabulary, and Trusted Profile composition. `git_environment()` cannot simply
be reused: it intentionally allows local config and does not neutralize every
commit-specific option. The observer safe-directory addition is reusable only
after the commit policy independently confirms it remains exactly scoped.

## Normal attached `git commit` model

**DOCUMENTED.** A normal commit creates a commit from the index and replaces the
tip of the current branch. Empty-tree commits are refused unless
`--allow-empty` is requested. `write-tree` creates tree objects from a fully
merged index.

**OBSERVED (Git 2.55.0.windows.4).** In a disposable attached `master`
repository, a normal `git commit -m normal` advanced `refs/heads/master` to a
new OID; `.git/HEAD` remained the same symbolic `ref: refs/heads/master` text;
the new commit's sole parent was the old OID; its tree equalled the prior
`git write-tree` output; raw `.git/index` bytes changed; staged worktree bytes
and an untracked file remained unchanged; `logs/HEAD` and
`logs/refs/heads/master` gained commit entries.

Thus the authorized effects are: object-database writes (tree(s), commit, and
possibly objects made by prior `write-tree`), one attached branch-ref update,
and normal HEAD/branch reflog metadata. The HEAD *symbolic ref itself* does not
change on an attached normal commit, though resolving `HEAD` changes because
its branch changes. The normal operation does not intentionally change the
worktree. It may rewrite the index file/cache metadata; it must not be described
as index-byte-preserving.

Git object insertion precedes the ref becoming reachable in the normal object
model, but this is not a promise that RAH can observe each internal step. A
failure, termination, or crash can leave unreachable tree/commit objects and
lock files. RAH can prove selected refs, commit parent/tree/message/identity,
and post-observed index semantics; it cannot prove absence of unreachable
objects or every incidental filesystem write. It never removes another actor's
Git lock file.

## Exact index and HEAD/branch admission

### Index identity

`git write-tree` is the required semantic identity: it is precisely the tree
the commit must record. It is intentionally a non-ref object-database write
when the tree is not already present, so precondition observation has that
limited incidental effect. This is acceptable only when ADR 0016 explicitly
authorizes it; it is not a read-only check.

Use a compound snapshot:

1. raw SHA-256 of the selected real index file, captured and rechecked directly
   before spawn;
2. canonical `git ls-files --stage -z --no-abbrev` digest for diagnostic,
   cross-platform semantic evidence; and
3. `git write-tree` OID as the authorized commit tree.

Raw bytes catch the exact-index race, while stage stream/tree prove the recorded
content. A tree alone is insufficient as an authorization token because it
forgets index-only distinctions and admission flags; index metadata alone is
unstable and cannot prove what was committed. v1 rejects unmerged entries,
intent-to-add, sparse index/checkout, alternate-index use, and any stage other
than zero, making the compound model sufficient without claiming all index
extensions are semantic.

Require `git diff --cached --quiet HEAD` to exit nonzero before spawn (with
controlled configuration) and recheck it immediately before the one attempt.
This correctly treats executable-bit and gitlink OID/mode differences as staged
tree changes. It forbids `--allow-empty`; metadata-only index differences are
not committable.

### HEAD and repository identity

Require, and recheck immediately before spawn:

- captured canonical repository/root/.git identities and exact native Git;
- non-bare selected worktree with normal `.git` directory (not a `.git` file);
- `git symbolic-ref -q HEAD` succeeds and returns the exact expected
  `refs/heads/<validated-name>`;
- `git rev-parse --verify HEAD` succeeds with an existing commit OID;
- that branch ref resolves to exactly the same expected old OID; and
- no special state or index admission failure below.

This rejects detached and unborn HEAD, a changed symbolic ref, a branch switch,
and an externally advanced branch. Linked worktrees are **deferred in v1**:
they add `.git` indirection, `commondir`, per-worktree config/index/ref details,
and cross-worktree races beyond the present repository identity policy.

## Rejected repository states

Fail closed on: bare repository; linked worktree; detached/unborn HEAD; any
`MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `BISECT_*`, or sequencer/rebase
directory/file; merge/squash message state; unmerged/conflicted index; sparse
checkout or sparse index; intent-to-add; alternate `GIT_INDEX_FILE`; and any
index entry not a stage-zero normal tracked file or explicitly admitted gitlink.

Recommendation: permit stage-zero gitlinks only after deterministic fixtures
prove their OID/mode identity and no recursive submodule action. Permit no
submodule command. Rejecting gitlinks in the first implementation is simpler
and preferred unless product evidence requires them. Allow ordinary unstaged
and untracked worktree dirtiness, including staged-and-unstaged changes to the
same path: commit records the index, not those worktree bytes. Reverify index
tree after the attempt; a concurrent worktree write is not caused by RAH and
may make the result uncertain if necessary proof is lost.

## Hooks, config, signing, editor, helpers, network

### Hooks

**DOCUMENTED.** `--no-verify` bypasses `pre-commit` and `commit-msg`, not
`prepare-commit-msg`; `post-commit` runs after a commit. Hooks are found in
`$GIT_DIR/hooks` unless `core.hooksPath` selects another directory.

**OBSERVED.** Default fixture hooks executed
`pre,prepare,msg,post`; `--no-verify` executed `prepare,post`; fixed
`-c core.hooksPath=<host-owned-empty-directory>` with `--no-verify` executed
none. A repository-local hostile `core.hooksPath` was overridden by the same
command-scope value.

The v1 invocation must have both `--no-verify` and command-scope
`-c core.hooksPath=<canonical-empty-host-directory>`; the latter is the actual
complete hook-discovery control. Its directory must be host-created, empty,
canonical, immutable for the process lifetime, and revalidated. This neutralizes
Git hook execution for this command, but is not an OS execution sandbox.

### Config and signing

**DOCUMENTED.** System, global/XDG, local, worktree, and command scopes exist;
includes and `includeIf` can extend file configuration. `GIT_CONFIG_NOSYSTEM`,
`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, and numbered `GIT_CONFIG_*` pairs
control sources/command-scope injection. Command `-c` overrides injected and
file configuration. Local configuration is still read for normal repository
operation; no documented switch makes a normal repository command ignore only
local config while retaining repository discovery.

Use `env_clear`, then a host allowlist: `GIT_CONFIG_NOSYSTEM=1`,
`GIT_CONFIG_GLOBAL=<null device>`, command configuration only for
`safe.directory=<canonical root>`, `core.hooksPath=<empty host directory>`,
`commit.gpgSign=false`, `user.useConfigOnly=true`, fixed host identity, and
other required safe operational values. Explicit command-scope values must
override local values. Treat the repository config as untrusted input and
reject known state/config entries that request special commit flows; do not
claim it is wholly ignored. `GIT_CONFIG_COUNT` must be host-constructed, with
no inherited numbered variables.

**OBSERVED.** Local `commit.gpgSign=true` with `gpg.program=cmd.exe` made a
commit fail attempting to sign (exit 128). Command `-c commit.gpgSign=false`
then succeeded. v1 must set that false value and omit every signing argv option;
the model cannot choose `--gpg-sign`, `user.signingKey`, `gpg.format`,
`gpg.program`, or SSH/X.509 program configuration. This controls Git-initiated
signing, not arbitrary hostile code already running on the host.

### Message/editor/helpers/network

Use fixed argv conceptually equivalent to:

`git -c core.hooksPath=<empty> -c commit.gpgSign=false ... commit --no-verify --cleanup=verbatim -m <one validated UTF-8 message>`.

`-m` means no message file path or editor is selected by the model. Clear
`GIT_EDITOR`, `GIT_SEQUENCE_EDITOR`, `VISUAL`, `EDITOR`, `GIT_PAGER`, and
`PAGER`; command config fixes an inert editor defensively and rejects templates.
Forbid `-F`, `-t`, `-C`, `-c`, `--reuse-message`, amend/fixup/squash, trailer,
signoff, patch/interactive, pathspec, and automatic staging flags. `verbatim`
allows exact postcondition comparison; host policy must reject leading/trailing
undesired layout itself.

Directly executing the host-selected native executable with the builtin
`commit` bypasses shell aliases, Git aliases, pager presentation, PATH-selected
Git subcommands, and remote Git transport. Residual process-launch risks are
hooks, signing, editor, and configuration-controlled helpers, addressed above.
The fixed operation contains no network Git action and neutralizes known Git
controlled launch paths; it does **not** provide OS-level network blocking.

## Message and identity contract

Accept exactly one UTF-8 string of at most **16 KiB** (policy choice; tests
must cover boundary), reject NUL and empty/whitespace-only values, and require
a nonempty first line. Permit body text only under host-defined newline/layout
rules; prohibit model-selected trailers and signoff. `--allow-empty-message`
and `--allow-empty` remain forbidden. The message may be model-proposed but
must pass host policy; a host-supplied override uses the same validation.

Identity is explicit host policy: required name and email, supplied as fixed
command configuration (or equivalent host-controlled environment), never from
repository/global config, OS username/hostname fallback, mailmap, or model
input. Set `user.useConfigOnly=true` defensively. Git-generated current dates
are preferred: they accurately represent the operation time and avoid exposing
model-controlled history timestamps. Author and committer are the same host
identity in v1; external author/date overrides are excluded.

## Environment policy

`env_clear` is required, then only host-selected values are added. Clear:

- repository routing: `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`,
  `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`;
- configuration: every `GIT_CONFIG_*`, `HOME`, `XDG_CONFIG_HOME`;
- identity/time: every `GIT_AUTHOR_*`, `GIT_COMMITTER_*`;
- interaction: editors, pager variables, `GIT_ASKPASS`, `SSH_ASKPASS`,
  `GIT_TERMINAL_PROMPT` (replace with `0`);
- transport/credential/proxy: `GIT_SSH`, `GIT_SSH_COMMAND`, `GIT_PROXY_*`,
  `http_proxy`/`https_proxy` variants, credential-helper environment; and
- output parsing influences: locale/timezone variables (set a host fixed
  `LC_ALL=C` only where textual output is parsed; prefer NUL/OID formats).

`PATH` is irrelevant to the initial absolute Git executable but may matter to
Git-launched helpers; do not inherit it unless a narrowly justified host value
is needed. Clearing environment is process minimization, not a sandbox.

## One-attempt, races, and result taxonomy

Acquire the existing per-repository RAH mutation lease across capture,
revalidation, one spawn, and post-observation. This serializes RAH stage,
unstage, worktree mutations, and future commits; it does not exclude external
Git/process writers.

Race model: before spawn, changed identity/HEAD/branch/index/message/state is
**precondition failed** (no commit spawn). Spawn failure is **known no effect**
only if fresh observation proves branch still old and authorized index snapshot
is intact. Lock contention/nonzero exit has that same classification only with
that proof. Once spawned, timeout, cancellation, output overflow, lost exit
status, observer failure, changed ref/index, or incomplete postcondition is
**uncertain**, never replayed. External branch movement before ref update may
cause Git refusal; after object creation but before ref update can leave
unreachable objects; after ref update but before observation can already be a
commit. RAH neither rolls back nor repairs locks.

Result vocabulary:

- `invalid_input` — malformed message/token;
- `precondition_failed` — state/config/identity/admission mismatch before spawn;
- `known_no_effect` — no spawn, or non-success with post-proof that expected
  branch and index remain intact;
- `committed_verified` — all proof below succeeds; and
- `uncertain` — any possible effect lacking complete proof.

## Postcondition proof and reflogs

For `committed_verified`, fixed read-only Git observations must prove: expected
symbolic branch still selected; its new OID differs from old; new commit has
exactly one parent equal to old; new commit tree equals authorized tree; exact
verbatim message equals the normalized authorized message; author/committer
meet host policy; no `gpgsig` header; and a fresh `write-tree` from the selected
index equals the authorized tree. Report a redacted new OID/branch/message
digest, never ambient config or identity details unnecessarily. Worktree is not
changed intentionally; do not claim concurrent actors did not change it.

Branch and HEAD reflogs are authorized incidental history metadata for attached
normal commit. They must be acknowledged in ADR 0016. Do not claim “one ref
only” without this qualification. No postcondition can prove that no unrelated
external actor mutated another ref or that no unreachable object exists.

## Public boundary and host review TOCTOU

The narrow public shape is conceptually `{ message }`; expected head/index are
host-attested review-snapshot fields, not model authority. Prefer a host-created
opaque single-use authorization token bound to canonical repository identity,
native Git identity, attached branch/ref, old HEAD, compound index snapshot,
message-policy generation, and expiry. The host captures it immediately after
the user reviews the staged diff; `repo.commit` consumes it with the message.
This better represents human authorization than asking a model to echo OIDs or
hashes it previously observed. If Task 134 chooses visible expected values,
they must be opaque attestations verified by the host, never model-selected
repository/ref/executable/CWD/config values.

## Cross-platform fixture plan

Required deterministic Windows/Linux/macOS tests: successful attached commit;
every rejected special state; HEAD/index/ref races; no staged delta; dirty and
untracked worktree preservation; staged+unstaged same path; executable mode and
gitlink policy; hook precedence/default/local/global/system isolation; signing,
editor/template, config/include, alternate-environment refusal; lock contention;
timeout/cancel result classifications; raw-index/tree proof; reflog effects;
and shared RAH lease serialization.

Windows additionally needs native Git-for-Windows identity, locked index/ref
and antivirus/sharing behavior, canonical/reparse identity, local path/config,
and Job Object cancellation fixtures. Linux needs shell hooks, signals,
symlinks, executable modes, and lock behavior. macOS needs Unix-like fixtures,
case-insensitive filesystem variants, symlinks, and executable modes. Windows
is the only certified live baseline. Linux/macOS receive deterministic coverage;
their live validation is deferred until explicitly authorized.

## Proposed ADR 0016 inputs and decision

**GO — evidence supports a bounded `repo.commit` v1 ADR.** Proposed title:
**ADR 0016 — Bounded Repository Commit Authority**. Concise authority: “A
trusted host may cause exactly one normal, non-empty, unsigned, hook-disabled
Git commit of one reviewed, host-attested index tree onto one unchanged attached
branch in one selected repository, then report only verified or uncertain
outcomes.”

ADR decisions: host owns repository, Git executable, branch/old HEAD/index
attestation, identity, message limits, environment/config/hooks path, timeout,
and lease. Authorized side effects are object writes, one current branch update,
normal HEAD/branch reflogs, and possible index metadata rewrite; no worktree
write is intended. Require attached non-unborn HEAD, normal branch, stage-zero
fully merged index, nonempty tree delta, and admitted non-special repository.
Reject all excluded history/ref/worktree/remote/credential/shell actions,
signing, hooks, editor/template, automatic staging, rollback, replay, linked
worktrees, sparse/alternate index, and unproven gitlinks. Use a host snapshot
token, fixed native argv, controlled config/environment, one spawn, no replay,
the five-result taxonomy, and the stated verified proof.

Unresolved for Task 134: final public token lifecycle/expiry and redaction;
whether initial v1 rejects or admits gitlinks; exact host identity storage UX;
whether the empty hooks directory is profile resource or private host runtime
asset; exact message layout/trailer policy; and supported Git-version range.

No Rust implementation, ADR, authority, version bump, tag, or release is made
by this research.
