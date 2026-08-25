# Task 086: Trusted Profile and Generic Tool Bridge Integration for `repo.create-file`

## Scope

Integrate the accepted bounded repository file-creation capability through the
existing Trusted Profile composition boundary and the unchanged Generic Tool
Bridge. This task is deterministic only and does not run Codex.

## Design

- Retain `profile_version = 1`.
- Add a closed `repo.create-file` capability declaration with only symbolic
  `executable` and `repository` resources, matching the existing repository
  capability pattern.
- Keep static validation non-mutating: it validates closed schema and symbolic
  bindings only; path and content remain runtime tool arguments.
- Bind the resolved host repository and Git executable only during effective
  composition, then register `RepositoryFileCreationTool` in a fresh registry.
- Do not add bridge production logic. Canonical names, private aliases,
  permission enforcement, dispatch, output translation, cancellation, and
  call-identity deduplication remain generic.

## Deterministic evidence

- Static loading accepts the valid symbolic capability and rejects missing,
  wrong-type, duplicate, wrong-permission, and authority-bearing extra fields.
- Effective composition exposes a redacted inventory and constructs no target.
- Mixed composition proves `repo.create-file` coexists with `repo.patch`,
  repository observers, and configured external providers without changing
  provider authority.
- Bridge tests use a real temporary Git repository and the actual effective
  composer. They verify `Execute` denial for None/Read/Write, one successful
  native creation for duplicate logical deliveries, exact output bytes/hash,
  untracked status, and unchanged index, HEAD, refs, and sentinel.
- Deterministic delegated outcomes prove uncertain and `write_failed_known`
  results are translated once and never trigger replay or cleanup.
- Code audit confirms `RepositoryFileCreationPolicy` and
  `RepositoryWorktreeMutationPolicy` both obtain their lock from the shared
  `git_stage::repository_lease` keyed by canonical repository root; no second
  mutation lock was introduced.

## Deferred

Task 087 owns the separately certified live Codex validation. This task did not
change the Codex baseline, invoke a model, or claim live evidence.
