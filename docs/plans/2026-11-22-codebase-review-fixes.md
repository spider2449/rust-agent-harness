# Codebase review fixes

The review of `ef8f2d4` found four issues in approval handling, workspace switching, session log creation, and workspace validation. Address them in the order below. Each phase needs a deterministic regression test and can be reviewed independently. The Windows baseline was 811 passed, 0 failed, and 21 ignored.

## P1. Recheck eligibility before reusing an approval

- In `plugins/frontend-web/src/approvals.rs` and `plugins/frontend-cli/src/lib.rs`, gate the remembered approval lookup on the current `ApprovalRequest::may_remember()`. Keep the existing session and tool keys, policy decisions, and explicit one-time approval flow.
- In each frontend, remember an eligible approval and then send a request for the same session and tool at effective `execute`. Assert that it requires a fresh answer. Cover an unknown effective level and verify that an eligible subsequent request still reuses the approval. Exercise the approval entry points so the tests cover the lookup as well as cache insertion.
- ADR 0074 must apply to every request, including after a policy or tool definition changes during a session.

## P2. Complete or roll back a workspace switch after caller cancellation

- In `crates/rah-cli/src/admin.rs`, give the entire switch a distribution-owned task and return its result to the caller. Keep target snapshotting, the write guard, plugin updates, rollback, and the final consistency state in that task. Preserve the immediate refusal when a turn holds a read lease and preserve fail-closed behavior if rollback fails.
- Use the distribution scope for task ownership. Check the kernel lifecycle context before handing the switch to a new task, where the caller's task-local reentrancy guard would be lost. Keep the consistency state across a distribution restart. Check shutdown ordering against the kernel lifecycle lock so disposal cannot deadlock on a switch.
- In the Web workspace update path, keep the Web switch mutex through the corresponding saved workspace update even if the HTTP request is dropped. Use a frontend-owned task for this operation where necessary. Preserve the existing response for a completed runtime switch when saving fails.
- Add a test plugin with barriers that pauses a switch after a root update. Drop the caller, release the barrier, and verify that all affected roots agree and a new turn can acquire the workspace lease. Repeat with an injected update failure and assert restoration of the old roots. In a separate Web handler test, pause storage, drop the caller, and verify the next update saves after the first and wins. Use barriers instead of timing assumptions.
- Caller cancellation must leave either the completed new workspace or the restored old one. A failed restoration keeps new turns blocked with the existing recovery error.

## P3. Publish complete session headers atomically

- In `plugins/session-jsonl/src/lib.rs`, move publication to a blocking task that takes ownership of the writer locks. Stage a complete header with `tempfile`, already present in the workspace lockfile. Sync it before same-directory publication with `persist_noclobber`. Verify the publication primitive on Windows and Linux. Keep the parent read and fork depth validation under the same writer lock as publication.
- Remove staging files on recoverable errors. Use a staging name that cannot be listed as a session, so a file left by process loss stays outside the session index. Preserve the JSONL format and existing torn-tail handling for append operations.
- Inject a write or publication failure and assert that the final path is absent, or that an existing destination remains byte-for-byte intact. Cover a cancelled caller, a successful reopen, and a fork that retains the parent's workspace and sequence boundary. Run the existing independent-writer and torn-tail tests.
- Readers must see either no new session or a complete, readable header. This phase covers process interruption and ordinary I/O errors. Power-loss durability across filesystems requires separate validation.

## P4. Enforce the workspace folder limit on the saved result

- In `plugins/frontend-web/src/workspace.rs`, preserve the early input-length check and validate the final folder count after canonicalization, deduplication, and active-root insertion. Perform the final check before moving plugins or writing settings.
- Extend the Web integration test to cover an accepted 32-folder result and a rejected 33-folder result. Check that the rejected request leaves the active workspace and saved folders intact, and verify that an accepted result can be stored in recent preferences.
- The workspace and preferences endpoints must agree on the maximum of 32 folders.

## Final validation

Run the focused regression tests at each phase. At the final boundary, run the repository checks in sequence. On Windows, put an existing `python.exe` on the test process's `PATH` for the DCC test. The checkout's bundled Python at `.rah/graphify-runtime/python` worked for the review baseline. Serialize Cargo invocations that share the same target directory.

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

Record exact failures and ignored tests and compare the existing Windows cases with the baseline. Inspect the final Git diff for scope and compatibility. The planned work uses existing plugin boundaries, service contracts, and file and wire formats. ADR 0074 already specifies the intended approval behavior.

## Implementation validation

- Windows formatting, workspace compilation, and strict Clippy passed.
- The Windows workspace suite passed with 818 passed, 0 failed, and 21 ignored. All seven added regressions passed. The known scheduler, DCC, and Web API group failures did not recur in this run.
- All 16 session JSONL tests passed under Ubuntu 24.04 on WSL, including interrupted publication and destination collision.
