# Task 175 — Windows live `repo.rename-file` validation

## Result

**PASS.** The historical attempts below remain preserved, including the
pre-tool no-effect automation failure. Task 175G completed the final fresh
destructive Windows live gate and the RAH source checkout remained unchanged.

## Authoritative starting state

- RAH `HEAD`: `8d24341bfd37d690b0b8f2e02c4fe986f03290fd`
- `origin/master`: `8d24341bfd37d690b0b8f2e02c4fe986f03290fd`
- Worktree: clean
- Desktop binary: `target/release/rah-desktop.exe`
- Workspace packages: 12, all `0.13.0`
- `v0.13` tag: untouched

## Native Codex baseline

The machine's PATH-installed Codex reported `codex-cli 0.152.1`. The actual
Desktop run used the established certified 0.13 baseline instead:

- Version: `codex-cli 0.149.0`
- Executable: certified baseline `codex.exe`
- Codex SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- Required code-mode host SHA-256: `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`
- Desktop displayed `Codex source: Explicit override` and
  `Codex version: codex-cli 0.149.0`.

The installed 0.152.1 version was not claimed as live evidence because this
0.13-pinned Desktop/runtime rejects versions other than 0.149.0.

## Disposable fixture baseline

The fixture was created outside the RAH checkout at `D:\rah-task175-live` with
`core.autocrlf=false`, an attached `master` branch, and a clean worktree/index.

- `rename-source.txt`: present, 23 bytes,
  SHA-256 `de5c37be4e2c9fec1f13719173a0a844ea33426aee992829b9de1fb3c71a60f5`
- `destination\`: present
- `destination\renamed-target.txt`: absent
- `sentinel.txt`: present, 18 bytes,
  SHA-256 `db4a34d2f228aec398094f60caeaecf8c63bb5a23bd24f46c75b558db8572091`
- Baseline `HEAD`: `507f1a9b805870d390cde511978d4df9617d3d60`
- Baseline ref: `507f1a9b805870d390cde511978d4df9617d3d60 refs/heads/master`
- Cached diff: empty
- Worktree diff: empty

## Live chronology and observations

1. The first Desktop session selected the fixture after connecting. Desktop
   correctly displayed `Repository tools: reconnect required`; the normal
   disconnect/reconnect workflow was then used. The first prompt attempt was
   malformed by GUI automation and `send_chat` returned a frontend error.
   The source remained present and the destination absent.
2. The fresh reconnect evidence log then recorded the authorized registry:
   `selected_repository=true`, `rename_authority_present=true`, and
   `repo.rename-file` in the relevant public tool list.
3. A fresh Desktop session was started with a fresh evidence path. The fixture
   was selected before connection, and reconnect completed normally. The
   evidence log again recorded `repo.rename-file` advertised in the composed
   registry.
4. Native mouse clicks reached the Desktop controls, but native keyboard and
   clipboard input did not reach the WebView textarea. The only resulting
   frontend action was the expected empty-prompt validation. No model turn was
   started.

Durable evidence files retained outside the fixture:

- `rah-task175-rename-live-failed-pre-effect.jsonl`: first pre-effect attempt
- `rah-task175-rename-live-2.jsonl`: fresh pre-effect attempt

The fresh log contains no `tool_requested`, `tool_started`, or `tool_finished`
records. Therefore the required alias, exact request, lifecycle `1/1/1`,
structured `result.status=renamed_verified`, and completion marker
`RAH_REPO_RENAME_FILE_LIVE_OK` were not observed.

## Independent post-effect and cleanup proof

- Source remained present.
- Destination remained absent.
- Sentinel remained present and unchanged.
- Fixture `HEAD`, branch, refs, index, and history remained unchanged.
- No stage, commit, Git move, copy/delete fallback, or replay occurred.
- Desktop was closed through its normal window-close path.
- No Task 175-owned Desktop or code-mode-host child remained. An unrelated
  pre-existing `codex-code-mode-host` process was not treated as owned by this
  run.

## Historical structured-result distinction

Task 163/v0.13 historical evidence recorded structured Tool output as
`result:null`. Task 173 fixed structured JSON capture prospectively. This Task
175 attempt did not reach Tool execution, so it provides no evidence for or
against the Task 173 structured-result path.

## Closure status

No product code, dependency, version, ADR, tag, or release-facing source was
changed. The historical observations above remain preserved; Task 175G below
is the separate final certification run.

## Task 175G final destructive certification

The final run used a new disposable repository at `D:\rah-task175g-live` and
the exact release Desktop binary from the authoritative Task 175F HEAD. The
certified native Codex pair was verified before launch:

- Version: `codex-cli 0.149.0`
- `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`

The fresh fixture baseline was:

- `rename-source.txt`: 22 bytes,
  SHA-256 `523bc87ce68215c12e2a7f5b15d4c288de0584b763bd49fc43dbccaec3c071dd`
- `sentinel.txt`: 17 bytes,
  SHA-256 `b830c670d9b2065fafe658509aa86fc8bf90cfdd4a93507a78d037e1e4717f53`
- Baseline `HEAD`:
  `1763d3026e1779eba396cafc346161b77e2d414b`
- Baseline ref:
  `1763d3026e1779eba396cafc346161b77e2d414b refs/heads/master`
- `destination\\renamed-target.txt`: absent
- Cached diff and worktree diff: empty

The fresh raw evidence sink was `C:\Temp\rah-task175g-rename-live.jsonl`.
All final live gates passed:

- `repo.rename-file` was advertised with
  `dynamic_definition_emitted=true`; actual alias: `rah_tool_10`.
- The advertised alias equaled the requested alias.
- The non-null request contained exactly the four required fields with
  `source_path=rename-source.txt`,
  `destination_path=destination/renamed-target.txt`, and the exact baseline
  SHA-256 and byte length.
- Lifecycle count was exactly Requested `1`, Started `1`, Finished `1`.
- The structured result was `is_error=false`,
  `status=renamed_verified`,
  `path=destination/renamed-target.txt`, and `uncertain=false`.
- Repository context remained internally consistent:
  `repository_generation=1`,
  `repository_fingerprint=repo-context:28b65edff3165a97fbc060987d2ba419153a6b04b9bc3182cbcd33800a146307`,
  `connection_generation=1`, `runtime_generation=1`, and
  `session_generation=1`.
- Completion evidence contained `marker_observed=true` for
  `RAH_REPO_RENAME_FILE_LIVE_OK`; no `desktop_failure` was present.
- Raw JSONL parsing passed with zero errors: `RAH_JSONL_PARSE_OK`.
- Independent filesystem proof showed source absent, destination present with
  the exact source SHA-256 and 22-byte length, and sentinel unchanged.
- Independent Git proof showed the expected unstaged source deletion and
  untracked destination directory; index, HEAD, branch, refs, and history were
  unchanged. There was no staging, commit, `git mv`, fallback mutation, or
  replay.
- Evidence showed no invocation of `repo.delete-file`, `repo.create-file`,
  `repo.edit-files`, `repo.patch`, or `repo.commit`.
- Desktop was closed normally and no Task 175G-owned Desktop/Codex child
  remained.

## Final Task 175 classification

Task 175 — Windows Live Codex Validation for `repo.rename-file`:

**PASS**

Windows `repo.rename-file` live certification:

**PASS**

Certified native Codex baseline: `codex-cli 0.149.0`

This certifies Windows live validation only. Linux remains
deterministic-validation only; no broader platform support promise is made.
