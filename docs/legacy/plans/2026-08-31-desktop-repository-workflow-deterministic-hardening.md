# Task 150 - Desktop repository workflow deterministic hardening

Starting SHA: `9beb057377fbe529c91a260208eb83504b6d27c7` (`origin/master` identical).

## Coverage audit

| Invariant | Classification | Deterministic evidence |
| --- | --- | --- |
| ADR 0016 message bounds/schema, stale index/HEAD/branch, policy generation | Existing complete proof | `rah-tools` repository-commit tests |
| Exact-index commit, OID/parent/tree verification, unstaged and untracked preservation | Existing complete proof | `normal_dirty_and_same_file_snapshots_commit_exact_index` plus postcondition tests |
| Binary refusal, opaque reviews, one-shot, known-no-effect and uncertain no replay | Existing complete proof | Task 148 Desktop and `rah-tools` deterministic phase tests |
| Refresh, stage/unstage, repository/model/identity/disconnect/restart invalidation | Existing complete proof | Task 148 lifecycle/workflow tests and current-generation guards |
| Result taxonomy, strict parsing, lifecycle association, DTO redaction | Existing complete proof | `commit_activity_presentation` and `activity_event` tests/source audit |
| Exact Desktop ToolRegistry/control pairing and Execute-not-authorization consequence | Missing cross-layer proof | Added Desktop registry test |

## Added proof

One `RepositoryCommitTool::compose` result supplies both the registered tool and
the retained Rust-only control. The test proves unarmed Execute dispatch fails
without a Git commit; explicit review/authorize then permits exactly one
verified commit. It independently checks the resulting OID, parent, count,
reviewed index content, unstaged preservation, untracked non-inclusion, and
replay refusal.

## Impact and remaining gap

No production behavior, dependency, schema, bridge, ADR, or authority changes.
ADR 0010 and ADR 0016 remain authoritative; authorization remains opaque,
in-memory, and one-shot. Task 151 owns the remaining Windows live-validation
gap. Expected final workspace is 12 packages, v0.11.0, edition 2024, with no
dependency delta.
