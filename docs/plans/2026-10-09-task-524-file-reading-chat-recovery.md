# Task 524 — Desktop file reading and Chat failure recovery

Starting local/origin/live master: `80fc345b1ff63e7f7c787abf6c15b785bee5e6e0`.
Preserve five existing untracked historical plans and private Task 523/523A
evidence. Fingerprints retained under ignored `target/task524/private/`.

1. Inspect repository-scoped fs.read, host dispatch, native continuation and
   Desktop events; validate direct and composed reads with disposable fixtures.
2. Correct confirmed empty failed-response presentation only. Test failure
   before text, after partial text, sanitized diagnostics and next-turn recovery.
3. Run affected frontend, Tool, host/runtime and Windows Desktop gates. Stop at
   the first required deterministic failure and preserve its evidence.
4. Prepare normal production Desktop using existing dependencies/logging.
   Human explicitly selects the fixture repository and sends the specified
   fs.read request in a fresh conversation. Capture actual Tool/arguments,
   authorization/result, continuation, terminal and visible Activity. Human
   confirms the result. Test ambiguous references in a fresh context.
5. Correct backend behavior only if actual evidence demonstrates a defect.
   Preserve all bounds, authority, HostExplicit 11, v0.34.0 and historical evidence.
6. Commit/push only after required acceptance; verify three heads and exact-head
   CI. No version, tag, dependency or Persistent Working Memory work.

## Current disposition — E: Windows live acceptance incomplete

Frontend lifecycle regression returned 0, covering failure before text, partial
output, closed diagnostics and next-turn recovery. Presentation now creates
assistant content on the first nonempty delta and adds a visible failed/cancelled
turn message, preserving partial text as incomplete. Tool Activity is unchanged.

Added a disposable repository fixture test for direct fs.read and production
registry dispatch, including denial without Read permission. This Rust test is
NOT RUN. First required deterministic failure: `cargo fmt --check`, exit 1,
rustfmt differences in the newly added test. Stop without retry. Private failure
record: `target/task524/private/first-failure.txt`.

No backend cause established. Actual model-selected Tool, arguments,
authorization, result/error, provider continuation, terminal and visible Activity
are unobserved. Phase C/D and human confirmation are NOT RUN. No production build,
commit, push, version, tag, dependency, ADR or authority change. HostExplicit code
is unchanged. Starting exact-head CI 37901305764 was completed/success; it does not
validate this uncommitted patch. Resume requires correcting the recorded test
formatting and completing all remaining gates and human acceptance.
