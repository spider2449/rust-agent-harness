# Task 087: Certified Live Codex Validation for `repo.create-file`

## Certified configuration

- Native `codex-cli`: `0.149.0`
- SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- Isolated `CODEX_HOME`; user configuration, MCP servers, plugins, apps, code
  mode, browser/computer use, image generation, and Codex-owned authority are
  disabled or unavailable to the RAH request.
- Model: `gpt-5.4`; reasoning: `medium`.
- Effective configuration fingerprint:
  `d967dc569062346bb9dd3084fef0f004842e36044a301d49e936a84b31ad0f7d`.

## Live fixture and assertions

`live_trusted_profile_create_file_bridge.rs` creates a fresh Git repository for
each run. It commits a sentinel and existing `src` parent, records raw
index bytes, HEAD, refs, and sentinel content, then verifies target absence from
HEAD, index, worktree, and ignore rules before the live turn.

The actual `TrustedStaticProfile::load -> rah_cli::profile_composition::compose`
path produces a fresh registry containing only `repo.create-file`,
`repo.file-info`, and `repo.status`, all Execute-gated. The example records the
canonical-to-private-alias mapping without assuming a numeric alias.

The host requires exactly one create request/start/finish with exact path and
UTF-8 content. It proves the regular non-reparse target's exact length/SHA and
untracked status through observers, while raw index bytes, HEAD, refs, and the
sentinel remain unchanged. Any alternate tool, duplicate call, approval, failed
observer, missing `Completed`, staging, or unexpected worktree path fails.
Ordinary Git diff is intentionally not used to verify untracked content.

The host emits `RAH_CREATE_FILE_LIVE_OK` only after all assertions and
app-server shutdown/reap pass. `FINAL_ASSISTANT_TEXT_DIAGNOSTIC` is logged only
for diagnosis; model prose cannot certify the gate.

## Results and limitations

Pre-commit result: 3/3 fresh successful runs through
`scripts/codex-live-gate.ps1`; each advertised `repo.create-file -> rah_tool_0`,
`repo.file-info -> rah_tool_1`, and `repo.status -> rah_tool_2`, with one
request/start/finish per tool. The post-commit three-run result is recorded in
the Task 087 completion report after exact-head Ubuntu CI.

The capability provides no overwrite, mkdir, delete, rename, append, binary
file creation, multi-file transaction, staging, commit/history/ref mutation,
rollback, or automatic replay.
