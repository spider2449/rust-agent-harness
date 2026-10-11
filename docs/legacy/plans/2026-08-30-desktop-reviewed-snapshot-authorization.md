# Task 148 - Desktop reviewed snapshot authorization

## Starting point

The authorized starting commit is `cb836446593ec8f3f79178196bce90fd18c4993a`.
At inspection, both `HEAD` and `origin/master` matched it and the sole local
change was the intentionally untouched `?? .vscode/` directory. Task 145
requires direct `rah-tools` composition; Task 147/147A provides the complete,
bounded staged-review presentation and selector/digest hardening.

## Design

`rah-tools` supplies an opaque public Rust host value,
`RepositoryCommitReview`, with private fields and no serialization or public
constructor. A single-lease review operation produces the existing normalized
`repo.diff-staged` presentation together with that binding. The binding holds
the policy generation, attached branch, HEAD, staged-entry semantic digest,
tree OID, and a private presentation identity. The presentation digest is a
correspondence check only, never authority.

Compare-and-arm revokes a previous pending authorization, takes the same RAH
repository lease once, validates the opaque policy generation and semantic
snapshot, and then captures the existing ADR 0016 authorization from the
current state. A mismatch leaves no pending authorization. Raw index bytes are
intentionally not part of review matching because harmless cache refreshes may
change them; ADR 0016 still captures and revalidates raw-index SHA-256 at arm
and before spawn.

## Desktop lifecycle and persistence

Desktop will retain a composed `RepositoryCommitTool`/`RepositoryCommitControl`
only for the selected repository, configured explicit human name/email, and a
current connected runtime generation. The Tool remains absent from the Desktop
registry in Task 148. Identity is a bounded, closed, atomically saved host
preference; it is not authorization. Repository, runtime/model, identity,
refresh/state, Stage/Unstage, disconnect/reconnect, Resume, and restart all
revoke or drop pending authority. No authorization, review, repository, Git
path, or runtime authority is persisted.

The frontend receives only a Rust-generated review selector and redacted
states: `identity_not_configured`, `review_required`, `review_stale`,
`ready_to_authorize`, `authorized_pending`, `authorization_failed`, and
`authorization_revoked`. An explicit human **Authorize Commit** action calls
the host compare-and-arm command. It has no message, no Tool invocation, and
does not run `git commit`, alter refs, index, or worktree.

## Tests and validation

Focused `rah-tools` tests cover unchanged authorization, stale staged content,
and cross-policy refusal. Desktop coverage must cover closed identity
preference migration/validation, selector and generation invalidation,
no-effect authorization, restart/Resume exclusion, Stage/Unstage revocation,
and registry exclusions. Required local gates are the Task 148 serial Cargo,
frontend syntax, diff, workspace, strict Clippy, and metadata gates. Windows
smoke uses a disposable repository and never invokes `repo.commit`.

## Foundation checkpoint

The rah-tools foundation was completed before Desktop integration with no
interim commit or push; focused repository-commit tests, format, and diff
checks passed. Desktop preferences now use closed schema version 2: valid v1
restores model-only state, while v2 restores model plus optional validated
identity. Identity is atomically persisted before activation, and a successful
change advances its Rust-owned generation, drops capability/review/authorization,
and requires reconnect.

For a current selected repository, identity, and connected runtime, Desktop
retains the direct `RepositoryCommitTool::compose` pair only in Rust and never
registers the Tool. One host review observation produces both the presented
normalized diff and the opaque review binding. Rust creates an ephemeral review
selector bound to repository, observation, identity generation, and counter.
The narrow authorization command accepts only that selector and invokes
compare-and-arm with the opaque review; it has no message or Git inputs and
does not execute a commit.

Every refresh intentionally revokes pending authorization, even if unchanged.
Repository/model/identity changes, disconnect, Stage, and Unstage also revoke
or drop it. Resume and restart restore no capability, review, selector, or
authorization. The frontend receives only redacted state and cannot reconstruct
authority; Task 148 leaves `repo.commit`, `host.git.stage`, and
`host.git.unstage` outside the Desktop model registry.

## Implementation and Windows smoke chronology

Task 148 began as an uncommitted foundation spanning the `rah-tools` reviewed
snapshot binding and the Desktop host, preference, capability, and frontend
surfaces. That accumulated foundation was deliberately preserved for one final
Task 148 commit.

The first manual Windows defect was a Rust-to-JavaScript serialization mismatch:
the review fields used snake_case while the frontend read camelCase. The DTO
serialization contract was corrected before commit.

An initially exercised Desktop executable was later shown to be stale. A fresh
exact-binary rebuild, absolute-path launch, and binary/path verification
resolved that ambiguity before further source diagnosis.

The second manual defect concerned `authorized_pending` presentation and the
refresh re-arm lifecycle. It was fixed before commit so the host-generated
authorized presentation is shown and a refresh revokes the pending authority.

The third manual defect was Disconnect retaining stale authorized presentation
or lifecycle state. The final fix performs actual pending-control revocation
and presentation invalidation on disconnect.

The fourth manual defect was that a binary staged review remained authorizable.
The host-side review path now treats binary staged content as non-authorizable:
it produces no authorizable opaque review binding.

Final Windows human smoke passed: supported staged text displayed Authorize and
the authorized presentation; refresh, disconnect, identity change, restart,
and reconnect behavior revoked or did not restore authority as required; binary
staged content did not offer Authorize. `repo.commit` was never invoked and
Authorize created no Git commit.

## Boundary

No commit is performed by Task 148. There is no new authority, ADR, dependency,
version bump, Generic Tool Bridge change, TrustedStaticProfile change, release,
or tag. ADR 0016 remains authoritative; the model request is not authorization.
