# Task 174 — Desktop repository file rename integration

## Scope

Integrate the existing host-owned `RepositoryFileRenameAuthority` into the
selected-repository Desktop composition path. This task is deterministic
Desktop integration only; Windows live Codex validation is deferred to Task
175.

## Host and runtime contract

Desktop constructs the opaque authority from the selected canonical repository
and host-selected Git identity. It stores that authority in
`DesktopRepository`, validates its resource binding, and passes it through the
fresh selected-repository `ToolRegistry`. The Generic Codex Tool Bridge routes
the canonical public tool name `repo.rename-file`; Desktop adds no alternate
Tool implementation or private alias.

The closed Task 171 request remains exactly four fields:
`source_path`, `destination_path`, `expected_source_file_sha256`, and
`expected_source_file_byte_length`. Frontend state is presentation only and
cannot construct or serialize authority, native paths, overwrite policy, or
fallback controls.

Missing repository or failed rename-authority construction leaves the rename
tool absent while preserving the existing unrelated composition behavior.
Repository selection and runtime/generation replacement create fresh context;
stale context cannot be reused across repositories or reconnects.

## Presentation and review behavior

Successful rename is classified with the existing Desktop structural-mutation
refresh path. The selected repository presentation is refreshed from observed
Git state, so the source disappears, the destination appears, and the
unstaged structural change remains unstaged. Desktop does not claim an index
rename and does not stage or commit.

The existing mutation review invalidation path revokes stale reviewed-commit
authorization after a successful rename. Failed precondition results do not
manufacture a successful refresh or mutation presentation.

## Deterministic coverage

Coverage includes selected-host authority composition, missing authority and
no-repository fail-closed behavior, repository/generation separation, the
exact public schema, same-directory rename, cross-directory move with an
existing parent, stale preimage, destination collision, unchanged index and
HEAD, no staging or commit, review revocation, Windows case-only rejection via
the existing core policy, generic bridge structured JSON results, and bridge
activity refresh classification. No live Codex run is performed.

## Future live evidence

Task 175 must use a disposable Windows Git repository with a tracked source
whose raw bytes exactly match HEAD, an existing destination parent, an absent
destination, and an unchanged sentinel. It must explicitly verify the
certified Codex baseline, exact bytes, source absence, destination presence,
unchanged index/HEAD/refs/history/sentinel, unstaged structural worktree
change, one `ToolRequested`/`ToolStarted`/`ToolFinished` lifecycle, durable
`renamed_verified` structured result, no replay, completion marker, and child
cleanup. Core autocrlf handling must preserve the exact raw-byte preimage.

