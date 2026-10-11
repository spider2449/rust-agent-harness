# RAH v0.6 repository observer research

Status: Task 058 research complete; no implementation is authorized by this document
Date: 2026-08-23
Baseline: RAH v0.5.1; Task 057 `d2a2f2a`; Windows live baseline; `codex-cli 0.149.0`

## Decision

Proceed with four host-configured, bounded, read-only observers: `repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.file-info`. They are fixed-command capabilities. Model input cannot select Git subcommands, executable, argv, repository, cwd, revision, environment, pager, configuration override, helper, network operation, timeout, or result limit. Only `repo.file-info` accepts one strictly validated logical repository path, rendered as one literal pathspec.

Reuse the existing private `HostExecutionPolicy`, native-executable/repository identity validation, per-repository lease, `PermissionLevel::Execute`, trusted-profile composition, `ToolRegistry`, and Generic Tool Bridge. Add only a private `rah-tools` observer helper; do not create a parallel Git executor or a generic Git capability.

**ADR decision: A, no new ADR.** This is bounded observation under established host-fixed process/repository identity boundaries, not a material new authority. Stop for an ADR before code if implementation accepts model-selected Git syntax/configuration, permits helpers, gives a write, or exposes a generic repository/process public boundary.

## Existing primitives and common hardening

Released `host.git.status` already provides the correct outer pattern: empty input, absolute native Git executable, canonical repository root and `.git` identity, a cleared child environment, and an exact porcelain command. It returns raw process text, so it is not the v0.6 normalized contract. The observer helper must consume bounded process bytes and create a closed `ToolContent::Json`.

`HostExecutionPolicy` supplies direct argv spawning, fixed canonical cwd, executable identity revalidation, output limits, timeout/cancellation, and Windows job supervision. Use the current exclusive per-root RAH lease for each short observer call. This stops another RAH mutation interleaving a multi-command observation, but is neither a cross-process lock nor a transaction.

Every child starts with an empty environment and receives exactly:

    GIT_CONFIG_NOSYSTEM=1
    GIT_CONFIG_GLOBAL=<NUL on Windows; /dev/null elsewhere>
    GIT_CONFIG_COUNT=2
    GIT_CONFIG_KEY_0=core.fsmonitor
    GIT_CONFIG_VALUE_0=false
    GIT_CONFIG_KEY_1=core.untrackedCache
    GIT_CONFIG_VALUE_1=false
    GIT_OPTIONAL_LOCKS=0
    GIT_TERMINAL_PROMPT=0

Do not inherit `PATH`, `HOME`, `USERPROFILE`, `XDG_*`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, object-directory variables, `GIT_EXTERNAL_DIFF`, pager/proxy/SSH-agent/credential/trace variables. Use Git's global `--no-pager`; every diff adds `--no-color --no-ext-diff --no-textconv`. Never set `safe.directory=*`: dubious-ownership failures are fail-closed.

Repository-local config and attributes remain Git semantic input, but external diff drivers and textconv are explicitly disabled. Clean/smudge filters are not invoked by these read-only commands. Keep hostile-filter marker tests and do not claim OS network isolation. Do not override `core.autocrlf`: a worktree-to-index comparison intentionally has Git's configured/attributed conversion semantics; RAH never rewrites patch bytes. Status uses `--ignore-submodules=all`; diff calls additionally use `--submodule=short`. They never recurse or execute inside a submodule and report only superproject gitlink index/HEAD changes. The fixed commands do not invoke hooks, transports, or credential helpers.

`GIT_OPTIONAL_LOCKS=0` prevents optional lock-taking work, including a status index refresh. The accurate release claim is **no intentional repository mutation**, not “provably zero filesystem writes”: Git, the OS, antivirus, filesystem timestamps, or future Git versions can still have incidental effects. Future implementation must prove no worktree/index/HEAD/ref change in fixtures and retain that qualification.

References: [Git environment](https://git-scm.com/docs/git), [status porcelain](https://git-scm.com/docs/git-status), [diff options](https://git-scm.com/docs/diff-options), and [ls-files](https://git-scm.com/docs/git-ls-files).

## Exact fixed command families

In the following, `git` is a host-selected, revalidated absolute native executable and cwd is the one revalidated repository root. No shell is involved.

### repo.status

    git --no-pager status --porcelain=v2 -z --untracked-files=normal --ignored=no --no-renames --ignore-submodules=all

Porcelain v2 plus NUL records is machine-readable, root-relative, and byte-safe. Include untracked entries at Git's `normal` directory granularity; exclude ignored entries. `--no-renames` removes configuration-dependent pairing and rename-cost amplification. Omit branch/upstream/ahead-behind data because it is unnecessary and stale immediately.

### repo.diff: worktree versus index

Run these three fixed calls in one common repository/lease/hardening envelope:

    git --no-pager diff --raw -z --no-abbrev --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
    git --no-pager diff --numstat -z --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
    git --no-pager diff --patch --no-color --no-prefix --full-index --no-renames --no-relative --no-ext-diff --no-textconv --diff-algorithm=myers --no-indent-heuristic --inter-hunk-context=0 --unified=3 --ignore-submodules=all --submodule=short

The comparison is tracked worktree content versus the index; untracked files never appear. Renames/copies deliberately become independent add/delete changes. Raw output provides exact paths, full object IDs, modes, and change kind; numstat supplies line counts/binary indication; the patch call is presentation only.

### repo.diff-staged: index versus HEAD

Use the same three calls with `--cached` immediately after `diff`. With a normal HEAD this is index versus HEAD. With an unborn HEAD, Git's `--cached` comparison is index versus the empty tree; return `base:"empty_tree"`, never an invented HEAD. If the index is unmerged, reject the entire result with `reason:"unmerged_index"` and no partial patch.

### repo.file-info

Its sole input is `{"path":"<logical repository path>"}`. After validation the host supplies that one value only after `--` under `--literal-pathspecs`:

    git --no-pager --literal-pathspecs ls-files --stage -v -z --full-name --no-abbrev -- <path>
    git --no-pager rev-parse --verify -q HEAD
    git --no-pager --literal-pathspecs ls-tree -z -l HEAD -- <path>
    git --no-pager --literal-pathspecs status --porcelain=v2 -z --untracked-files=all --ignored=no --no-renames --ignore-submodules=all -- <path>

Skip `ls-tree` when `rev-parse` identifies an unborn HEAD. The host may use `symlink_metadata` only for this validated logical entry to report physical presence, kind, size, and a bounded regular-file digest. Reject traversal, absolute paths, NUL, backslash separators, `.git`, and link/reparse ancestors before a direct read. This is repository-semantic observation, not a generic filesystem-stat API.

## Paths, binary data, and limits

The first three tools accept exactly `{}`. `repo.file-info.path` is a required nonempty UTF-8 logical Git path, at most 1,024 bytes (whole request at most 4 KiB), slash-separated, with no `.`, `..`, `.git`, NUL, backslash, or absolute component. It is never canonicalized into a model-visible physical path.

Parse Git paths as NUL-delimited bytes. A returned path or patch is a tagged closed value:

    {"encoding":"utf8","value":"src/lib.rs"}
    {"encoding":"base64","value":"bmFtZV8uLi4="}

Use UTF-8 only when bytes validate; otherwise base64 exact bytes. This handles spaces, tabs, newlines, non-ASCII names, and invalid UTF-8 Unix names without human Git quoting. Windows-invalid names cannot be constructed there; Unix must test case collisions and normalization-distinct names. Invalid-byte paths can be reported but cannot be selected by `repo.file-info` in v0.6.

Never return binary file bytes or Git binary patches. Binary is structural (`binary:true`, `patch:null`). A textual section with invalid UTF-8 is base64. This intentionally does not inherit `repo.patch` strict UTF-8 content rules: observers describe state rather than mutation eligibility.

## Closed normalized schemas

Every success is one JSON object with `status:"ok"` and `consistency:"best_effort"`. A rejection is an error `ToolOutput` with a closed status/reason only: no partial semantic entries and no raw stderr. A host audit may separately retain a root-redacted UTF-8-lossy 8 KiB stderr tail.

### repo.status

    {
      "status":"ok", "consistency":"best_effort",
      "entries":[{"path":{"encoding":"utf8","value":"src/lib.rs"},"tracked":true,
        "index_state":"modified","worktree_state":"unmodified",
        "conflict_state":"none","submodule_state":"none"}],
      "sparse_index_flags":"not_enumerated"
    }

Normalize porcelain v2 X/Y to `unmodified`, `added`, `modified`, `deleted`, `renamed`, `copied`, `type_changed`, or `unmerged`. Untracked entries use `tracked:false` and both states `untracked`. Normalize the seven unmerged pairs to `both_added`, `both_deleted`, `added_by_us`, `deleted_by_them`, `added_by_them`, `deleted_by_us`, or `both_modified`. Sort by raw path bytes, then record kind.

Status does not enumerate clean index entries; it must not claim skip-worktree/assume-unchanged facts for an omitted path. The explicit `sparse_index_flags:"not_enumerated"` prevents that overclaim; use `repo.file-info` for one logical entry.

### shared diff result

Both diff tools use one schema, differing only by `comparison` (`worktree_to_index` or `index_to_head`) and `base` (`index`, `head`, or `empty_tree`):

    {
      "status":"ok", "consistency":"best_effort",
      "comparison":"worktree_to_index", "base":"index",
      "files":[{"old_path":{"encoding":"utf8","value":"src/lib.rs"},
        "new_path":{"encoding":"utf8","value":"src/lib.rs"},
        "change_kind":"modified","old_mode":"100644","new_mode":"100644",
        "binary":false,"added_lines":1,"deleted_lines":1,
        "patch":{"encoding":"utf8","value":"diff --git ..."}}]
    }

`change_kind` is `added`, `deleted`, `modified`, `type_changed`, `unmerged`, or `gitlink_changed`; rename/copy are never emitted. Use null old/new path for creation/deletion. Implementation may only split the fixed patch stream at verified top-level `diff --git ` boundaries and require one-to-one agreement with raw records. It does not parse hunk grammar, patch header paths, or model data.

**Raw-vs-structured recommendation: C, hybrid structured metadata plus bounded opaque patch text.** Raw diff alone loses safe path/mode/binary semantics; fully parsed hunks introduce avoidable grammar, quoting, binary, and future-Git parser risk.

### repo.file-info

    {
      "status":"ok", "consistency":"best_effort",
      "path":{"encoding":"utf8","value":"src/lib.rs"},
      "head":{"present":true,"mode":"100644","object_id":"..."},
      "index":{"tracked":true,"entries":[{"stage":0,"mode":"100644","object_id":"..."}],
        "intent_to_add":false,"assume_unchanged":false,"skip_worktree":false,"conflicted":false},
      "worktree":{"present":true,"kind":"regular_file","size_bytes":123},
      "sparse_state":"normal","staged_vs_head":false,"worktree_modified_vs_index":true,
      "content":{"byte_length":123,"sha256":"..."}
    }

`ls-files --stage -v -z` provides every index stage, full object ID, mode, the skip-worktree tag (S, case-insensitive), and a lower-case assume-unchanged tag. A zero-ID stage-0 entry is `intent_to_add:true`, never an ordinary staged blob. Stages 1, 2, or 3 mean `conflicted:true`; return all stages and make simple comparison booleans null. Modes normalize to regular (`100644`/`100755`), symlink (`120000`), gitlink (`160000`), or other; executable is true only for `100755`.

`sparse_state` is `normal`, `skip_worktree_present`, `sparse_omitted`, `skip_worktree_unknown`, `not_tracked`, or `conflicted`. `sparse_omitted` means the skip-worktree flag and no physical entry; Git has no separate sparse-omitted index bit. This closes Task 057's sparse/skip-worktree gap.

For normal stage 0, compare mode/object ID with HEAD for `staged_vs_head`; for unborn HEAD compare with `empty_tree`. Per-path porcelain provides `worktree_modified_vs_index`; a missing skip-worktree entry is not a deletion. `content` exists only for a direct, present, non-link regular file at most 1 MiB and supplies digest/length, never bytes. Binary/text classification is excluded because it is a diff property, not generic file reading.

## Bounded result policy

| Tool | Timeout | Count | Result/output bound | Per item |
| --- | ---: | ---: | ---: | ---: |
| repo.status | 10 s | 10,000 entries | 4 MiB stdout; 8 KiB audit stderr | 4 KiB path |
| repo.diff | 15 s total | 256 files | 1 MiB normalized result; 8 KiB audit stderr | 256 KiB patch; 4 KiB path |
| repo.diff-staged | 15 s total | 256 files | 1 MiB normalized result; 8 KiB audit stderr | 256 KiB patch; 4 KiB path |
| repo.file-info | 5 s total | one path, at most 3 stages | 128 KiB result; 8 KiB audit stderr | 1 KiB input path; 1 MiB digest file |

Process captures need independent caps no larger than result capacity plus base64 overhead. On cap breach, timeout, malformed NUL records, raw/numstat/patch mismatch, unsupported Git record, identity change, or unmerged diff, reject with no partial list or patch. This fail-closed policy is preferable to silent truncation or a misleading partial result.

## Lease, staleness, and abuse conclusions

Outputs are point-in-time **best effort**. Do not add a purported repository consistency digest: external actors can change index, HEAD, or worktree between multi-command normalization despite any cheap pre/post token. The existing lease serializes RAH calls only. Re-observe immediately before conditional `repo.patch`, which keeps its own precondition checking.

Required adversarial handling: hostile configuration/environment/executable is controlled by fixed construction and revalidation; external diff/textconv by mandatory flags and cleared environment; crafted paths by NUL parsing/tagged encoding; huge repositories/diffs by caps and `--no-renames`; submodules/links by no recursion and no link traversal; conflicts/sparse state by explicit normalization; stderr by audit-only redaction; and root replacement by identity checks. Residual external TOCTOU and lack of OS sandbox/network isolation must be stated precisely.

## Future validation matrix

Deterministic tests use owned temporary repositories and native Git, with no model, credentials, network, or paid service.

| Windows scenario | Required proof |
| --- | --- |
| clean, mixed staged/unstaged, untracked, ignored | normalized ordering and states |
| conflict/unmerged | all file-info stages; diff rejection/no partial patch |
| CRLF, Unicode, spaces/tabs/newlines | Git conversion semantics and byte-safe paths |
| symlink/reparse, sparse/skip-worktree, assume-unchanged | modes/kinds, digest refusal, sparse distinction |
| binary, deletion, mode, gitlink, rename-like | structural binary/no bytes; add/delete only; no recursion |
| large output, timeout, malformed records | closed failure/no partial output |
| external-diff/textconv/filter attempt | no side-effect marker; exact environment/argv audit |
| optional locks/concurrent change | no status refresh; lease behavior; best-effort claim |

Windows live evidence, if later run, must use the trusted effective profile and real Generic Tool Bridge at `codex-cli 0.149.0`, and prove no worktree/index/HEAD/ref mutation. It is distinct from native deterministic evidence.

Actual Unix CI/runtime must additionally validate invalid UTF-8 filenames/base64 output, executable mode, symlink behavior, case-sensitive and normalization-distinct paths, native diff section splitting/binary behavior, sparse flags, and hostile-helper tests. This is deterministic CI/runtime validation. A Unix live Codex claim needs a separately verified executable/schema baseline and is not made here.

## Permission, impact, and Task 059

All observers use `PermissionLevel::Execute`: it remains the outer gate for a host-fixed subprocess, while capability policy limits real authority. A future read-process permission would improve approval vocabulary but not materially reduce model authority under this closed design; do not add one now.

Task 058 changes no Rust, Cargo manifest, dependency, trusted-profile schema, bridge, public API, or ADR. Future code stays in `rah-tools`, with no Codex/provider dependency and no generic Git/shell capability.

**Recommended Task 059:** implement a private deterministic observer foundation and `repo.file-info` first, with adversarial temporary-repository tests. It establishes path-byte encoding, identity/lease reuse, stage/sparse/intent-to-add normalization, bounds, and safe digest behavior before status and shared diff work. Add status next, then shared hybrid diff logic; do not begin profile/bridge or live-example work in the first increment.
