# RAH v0.11 Scope and Authority Roadmap

**Task 132 — research / roadmap only**

## Decision

**Recommend v0.11: Bounded Repository Commit Authority.** The user-facing outcome is one deliberate, locally durable commit of the already reviewed and already staged state in the selected repository. It is not general Git history, ref, branch, remote, credential, shell, or filesystem authority.

This recommendation introduces a new narrow authority and therefore needs a new ADR before implementation (proposed future ADR 0016). No authority is implemented by this document.

## Authoritative baseline and audit

- Requested post-release starting checkpoint: 0133933d866239732c5fe79e36b1394c963ecc47 (docs: mark RAH v0.10.0 released).
- Immutable v0.10 release: annotated tag v0.10.0, object d340120e5b316265d6a4cd83bdf08eb73d712d1a, peeled commit 9f4947ce4e37e9ce5b1e49330ab5327c1bd61ffa.
- Product baseline: 12 packages at 0.10.0, Rust edition 2024, certified Codex baseline exactly codex-cli 0.149.0.

At Task 132 inspection, both HEAD and origin/master were f9612ec138c381cfc8e684e4f2617999f019a586 rather than the requested checkpoint. Its sole effect is a 100%-similarity relocation of two v0.1 documents into docs/; it changes no runtime, authority, dependency, or v0.10 claim. This roadmap assesses the requested released baseline while being authored on the current branch. The only initial worktree entry was untracked .vscode/, left untouched.

Audited: CHANGELOG.md, README.md, docs/ARCHITECTURE.md, docs/SECURITY.md, accepted ADRs 0001–0015, and v0.10 scope, milestone, release, and post-release documents.

## Actual v0.10 product capability

### Repository workflow

RAH already supports:

    select host-owned repository
      -> inspect (fs.read, repo.file-info, repo.status, repo.diff, repo.diff-staged)
      -> bounded edit/create (repo.patch, repo.create-file, repo.edit-files)
      -> inspect resulting diff
      -> bounded host Git index mutation (stage/unstage)

The workflow stops at **the authorized index**. RAH cannot create a commit, advance a branch, switch a branch, or make network Git requests. A user must leave the product to turn reviewed staged content into history.

### Provider/runtime and Desktop

- Codex is an optional AgentRuntime adapter; the Generic Tool Bridge keeps public boundaries RAH-owned.
- Local stdio MCP and Process Plugin providers adapt through Tool and ToolRegistry; Trusted Profile v1 composes fixed host-selected providers.
- Desktop discovers/selects certified Codex, configures one bounded llama.cpp endpoint, discovers native Git, binds a selected canonical repository, isolates launch CWD/AGENTS.md, and retains repository-scoped conversations.
- Resume/replay is explicit and bounded. SQLite is durable private conversation persistence, not a generic data or authority surface.

Task 120 successful remote/non-loopback llama.cpp generation remains **DEFERRED / NOT VALIDATED**. Redirect, proxy, DNS, effective destination, peer identity, and TLS transport confinement remain **NOT CLAIMED**.

## Current hard limits

Absent authorities include Git commit; branch creation/switching; arbitrary ref update; merge/rebase/cherry-pick; reset/clean/stash; push/pull/fetch; credentialed Git; generic repository delete/rename; generic filesystem write; generic shell/process; network MCP/Streamable HTTP; provider installation/download; profile hot reload; dynamic authority restoration; OS sandbox/network isolation; and rollback. Timeout, cancellation, or disconnect never proves rollback; uncertain external effects are not replayed.

## Candidate assessment

Scores use 1 (low/unfavourable) to 5 (high/favourable). For New auth, Risk, X-platform, Size, Recovery, and Lock-in, 5 means more cost/exposure. Test and Live mean feasibility, where 5 is easier.

| Candidate | Value | Workflow | Frequency | New auth | Risk | X-platform | Test | Live | Fit | Size | Recovery | Lock-in | Assessment |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| A. Bounded repo.commit | 5 | 5 | 4 | 4 | 4 | 3 | 4 | 4 | 5 | 3 | 4 | 2 | **1 — adopt** |
| B. Branch/ref authority | 3 | 2 | 3 | 5 | 5 | 4 | 3 | 3 | 3 | 4 | 5 | 4 | 8 — defer |
| C. Delete/rename | 3 | 3 | 3 | 4 | 5 | 5 | 3 | 3 | 4 | 4 | 5 | 3 | 7 — defer |
| D. Desktop workflow/task UX | 4 | 3 | 5 | 1 | 1 | 2 | 5 | 4 | 5 | 3 | 1 | 1 | **2 — runner-up** |
| E. Remote llama proof / confinement | 3 | 2 | 2 | 1–4 | 3–5 | 4 | 3 | 1–3 | 4 | 3–5 | 4 | 3 | 5 — defer |
| F. Network MCP / Streamable HTTP | 4 | 2 | 3 | 5 | 5 | 5 | 2 | 2 | 3 | 5 | 5 | 5 | 9 — defer |
| G. PluginManager lifecycle | 3 | 2 | 2 | 3–5 | 4–5 | 4 | 3 | 3 | 3 | 4 | 4 | 5 | 6 — defer |
| H. Profile reload/dynamic authority | 2 | 1 | 2 | 5 | 5 | 4 | 2 | 2 | 2 | 5 | 5 | 5 | 10 — defer |
| I. Multi-repository execution | 3 | 2 | 2 | 5 | 5 | 5 | 2 | 2 | 2 | 5 | 5 | 5 | 11 — defer |

A is the only candidate that completes the existing everyday coding path without first requiring a provider, network transport, lifecycle system, or broad filesystem/ref plane. Its risk is explicit: it makes durable history and advances a branch. Existing host-owned repository identity, native Git discovery, observer methods, index mutation, diff review, static profile composition, and one-attempt uncertainty vocabulary are useful foundations.

D is the runner-up. It is valuable and should follow, but better staged-diff presentation still leaves a user in another tool to commit. It wins on authority economy, not workflow completion. E is validation debt plus a separate security-hardening programme. F–I materially enlarge authority composition and recovery complexity. C introduces destructive path semantics, Windows case/reparse complexity, and partial effects without completing the staged-authoring flow. B explicitly selects/alters refs and worktree identity, so is broader than A.

## Authority matrix

| Candidate | Existing sufficient? | New authority / owner | Model-selectable inputs | Host-fixed inputs | Side effect / replay | Rollback | ADR? |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A commit | No | one commit from fixed current index; trusted host | bounded message only | repo, Git, expected HEAD/index/branch, identity/policy | commit plus current branch ref; never replay | none | **new ADR** |
| B refs | No | ref/HEAD/worktree selection; host | names/targets if designed | repo/Git/policy | refs/worktree; never replay | none | separate ADR |
| C delete/rename | No | bounded existing-path mutation; host | validated paths/group | repo, snapshots, collision rules | filesystem/index-visible effects; never replay | none | new/successor ADR |
| D UX | Yes | none; Desktop presentation/control | display choices | existing state | no external effect | n/a | no |
| E llama | Yes for proof; No for confinement | validation only, or transport policy | none for proof | existing endpoint | requests only; no replay | none | confinement likely ADR |
| F network MCP | No | remote provider transport; host/profile | admitted fixed tool args | endpoint/auth/transport policy | remote effects; never replay | none | new ADR |
| G lifecycle | No | trusted local provider start/stop; host | none beyond tool calls | catalog/executable identity | processes/provider state | supervision only | new ADR |
| H reload | No | replace active registry; trusted host | explicit UI action at most | profile source/generation | in-flight calls/registry | none | new ADR |
| I multi-repo | No | composed repository identities; host | scoped repo choice only if designed | each repository/policy | multiple state planes | none | new ADR |

Existing stage/unstage does **not** imply commit. repo.edit-files does **not** imply stage. Commit does **not** imply branch switching. Endpoint authority does **not** imply network MCP.

## Deep analysis: safe minimum repo.commit v1

### A–D. Authority, refs, and scope

Commit introduces Git object creation **and** changes the ref resolved by attached current HEAD. Git documents that normal commit is a direct child of HEAD and updates the current branch. It is therefore not object creation alone and cannot be treated as existing index authority. Detached HEAD commits have different ref semantics and v1 should reject them. Branch creation/switching, arbitrary ref updates, detached HEAD, merge commits, amends, fixups, cherry-picks, rebases, tags, and remote actions remain excluded.

The narrow operation can be constrained to the exact selected repository, exact current attached branch, exact expected non-unborn HEAD, and exact current index only. It must never select a ref, parent, pathspec, or alternate index.

### E–J. Metadata and execution controls

The model may propose one bounded UTF-8 message, host-policy checked for length, encoding, non-empty subject/body rules, and prohibited trailers/signoff policy. The host should also support a host-supplied message under the same checks. The model never supplies Git argv, repository, executable, CWD, env, author, committer, branch/ref, parent, hook path, signing key, config, or network target.

Use fixed normal commit invocation: no paths, -a, patch/interactive modes, amend, fixup, squash, allow-empty, author/date override, message reuse/file/template, trailers, signoff, or signing. Automatic staging is forbidden. Worktree cleanliness is not required; exact index identity is the precondition, so unrelated unstaged/untracked files may remain. The postcondition must prove the recorded index state did not change.

Author and committer come from explicit host-selected identity policy, not ambient config or model fields. Unborn HEAD is deferred because it adds initial-commit/ref-creation semantics.

Fixed argv is insufficient. Git says pre-commit and commit-msg can be skipped with --no-verify, but prepare-commit-msg is *not* skipped and post-commit runs after the commit. core.hooksPath can redirect all hooks. v1 must disable hook discovery through host-fixed per-invocation configuration and reject unsupported local state; --no-verify alone is insufficient.

Signing must be disabled (commit.gpgSign=false and no signing argv), because configuration can select GPG, X.509, or SSH signing programs. Provide a fixed noninteractive message and defensive inert editor; do not allow templates/edit modes. Git config supports includes and conditional includes, so global/system config must be skipped or replaced with host-controlled empty config sources, while invocation configuration fixes required values. Local config is a security input; v1 should use a minimal host-built configuration and reject config that can introduce helpers, hooks, signing, or external programs.

The environment must be allowlisted/minimized: clear config, dir/work-tree/index/object paths, author/committer identity/date, editor, pager, SSH/askpass, hooks/template paths, proxy, and credential controls. The fixed commit path must not run a remote command, but removal prevents accidental authority growth.

Git documentation supports these hazards: normal commit records the index and advances current branch; hooks are executable; core.hooksPath redirects them; config includes add sources; signing may launch external programs; identity derives from config/environment; and global/system configuration can be skipped. See [git-commit](https://git-scm.com/docs/git-commit), [githooks](https://git-scm.com/docs/githooks), [git-config](https://git-scm.com/docs/git-config), and [Git environment](https://git-scm.com/docs/git).

### K–T. Proof, races, uncertainty, minimum contract

Before spawn, validate canonical repository identity; native executable identity; no merge/cherry-pick/rebase/sequencer state; attached symbolic HEAD; expected old commit OID; current branch OID; index checksum/identity plus staged tree/diff identity; and message digest. Recheck essential state immediately before launch. This narrows but cannot eliminate races with external writers.

After exactly one launch attempt:

- **Known no effect:** branch remains at expected old HEAD and index identity is proven unchanged.
- **Committed and verified:** HEAD and prior attached branch point to one new commit with exactly expected one parent, preauthorized index tree, compliant message/identity, and matching index/worktree postconditions.
- **Uncertain external effect:** timeout, cancellation, lost process status, observer failure, changed HEAD/index/ref, or incomplete proof.

Uncertain effects are never retried, replayed, restored, reset, or compensated. Cancellation after object/ref creation remains uncertain until post-observation. Exactly-one proof means one host spawn attempt plus proof of one child of expected parent on expected branch; it does not claim no unreachable object exists.

Minimum useful safe contract: exact repo + exact Git executable + attached expected non-unborn HEAD + exact expected index + bounded host-checked message + host identity/minimized environment -> one normal non-empty non-amend index commit -> verified/redacted result.

## Cross-platform assessment

| Concern | Windows | Linux/macOS | v0.11 position |
| --- | --- | --- | --- |
| Git executable | closed Git for Windows discovery exists; verify final identity | explicit host-installed Git identity needed | no PATH-selected generic command |
| Process/cancel | Job Object supervision helps lifecycle, not sandboxing | process groups/signals differ | cancel is uncertain until observed |
| Git locks | antivirus/index/ref sharing timing notable | lockfiles/permissions differ | deterministic contention tests; no lock deletion/retry |
| Paths | canonical roots, reparse points, case rules | symlinks/case/file modes differ | selected repo only; no delete/rename |
| Config/hooks | Windows paths and .exe helpers may execute | shell/interpreter modes common | fixed config/env; hooks disabled |
| Index/tree modes | Windows normalization has test history | executable modes affect trees | verify tree/index, not assumed modes |

Windows is the certified live baseline. Linux and macOS need deterministic coverage before support claims; macOS live validation is not established.

## ADR relationship and deferrals

ADR 0010 authorizes bounded repository *index* mutation only, not commit/history/ref authority. ADRs 0012 (repo.patch), 0013 (repo.create-file), and 0014 (repo.edit-files) authorize distinct bounded worktree content changes, not staging or commit. ADR 0015 bounds one host-selected model-provider endpoint; it neither proves transport confinement nor authorizes network MCP.

Bounded commit needs a **new authority ADR**, rather than extending ADR 0010: it crosses the index-to-history/ref plane and adds hook/config/identity execution hazards.

v0.11 must not combine commit with branch/ref authority, checkout/switch, initial commits, detached HEAD, amend, merge/rebase/cherry-pick, reset/clean/stash, push/pull/fetch, remotes/credentials, tags, automatic staging, generic Git argv, delete/rename, generic shell/process, network MCP, provider installation, PluginManager download/update, profile reload, dynamic authority restoration, multi-repository execution, OS sandbox/network isolation, or rollback.

Task 120 remote successful-generation evidence remains **DEFERRED / NOT VALIDATED** and endpoint transport confinement remains **NOT CLAIMED**.

## Proposed Task 133+ sequence

1. **Task 133 — Repository Commit Authority Research.** Confirm installed Git behavior, config/hook neutralization feasibility, state model, and cross-platform fixtures. No code.
2. **Task 134 — ADR 0016: Bounded Repository Commit Authority.** Decide contract, exclusions, identities, and uncertainty/result semantics.
3. **Task 135 — Commit Policy / Native Git Foundation.** Implement private host policy and fixed native-Git foundation only after ADR acceptance.
4. **Task 136 — Deterministic Commit Hardening.** Add race, hook/config, identity, index/HEAD, cancellation, and postcondition tests.
5. **Task 137 — Trusted Profile Composition.** Compose only the accepted static host capability.
6. **Task 138 — Generic Tool Bridge Verification.** Prove neutral boundary and rejected inputs.
7. **Task 139 — Windows Certified Live Validation.** Use certified Codex and an isolated local fixture; prove one verified commit and cleanup.
8. **Task 140 — v0.11 Milestone Audit.** Reconcile ADR, limits, docs, deterministic/live evidence, dependencies, and release readiness.

## Validation strategy and non-implementation statement

Start with deterministic local Git fixtures: attached-branch success; HEAD/index race refusal; dirty-but-unstaged acceptance; hook/config/signing/editor neutralization; identity refusal; normal non-empty-only commit; cancellation/timeout classification; and no replay. The Windows live gate uses a fresh local repository and proves expected parent/tree/ref/index after one deliberate call, with no remote or credentials. Linux/macOS claims need their own evidence.

This Task 132 document makes no Rust, Cargo, public API, profile-schema, Desktop-behavior, Git-command, networking, release, or tag change. It creates no ADR and implements no new authority.
