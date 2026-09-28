# Task 452 — Desktop live certification harness and Task 438 disposition

Date: 2026-09-28. Disposition: **STOPPED; Task 438 NOT CERTIFIED**.

## Prior evidence and historical limit

Task 451 observed one new app-server, PID 14920, directly parented by the production Desktop test process PID 2580. It remained visible in five post-Connect samples. Its saved 0.157.1 path and SHA256 matched `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`. An unrelated Codex process existed before Desktop launch. This was Connect-only evidence.

Task 450's historical ownership failure remains unexplained. Task 451 does not prove which component of Task 450's old combined assertion failed.

## Permanent harness work in the dirty re-entry tree

The ignored Desktop live test now takes a pre-launch and pre-Connect census and five post-Connect samples. It classifies new app-server candidates by process ancestry rooted at the Desktop PID, independently counts pre-existing candidates, new candidates, Desktop-owned candidates, path matches, and hash matches, and records sanitized PID, parent PID, process name, first and last sample, final-sample liveness, and ownership. It asserts owned count, saved path, saved hash, and final liveness separately. Its deterministic structural test covers a pre-existing unrelated candidate, wrong identity, two owned children, and an unrelated new candidate with no owned child. The intended later stages use the same Desktop session for neutral Send and a read-only `repo.file-info` call against a disposable committed fixture.

Pre-live validation passed: `cargo fmt --check`, `git diff --check`, `cargo test -p rah-desktop --no-run`, PowerShell parser validation of the embedded census script, and the exact focused census test (one passed). An earlier over-specific exact test filter ran zero tests; the corrected filter ran one. No earlier zero-test invocation is counted as validation.

## One production live attempt

The exact ignored Desktop test was run once after validation. The 0.157.1 saved executable was checked against the expected SHA256 before Connect; model selection was `gpt-6-luna`. The pre-launch and pre-Connect candidate counts were both zero. The five post-Connect samples yielded one new Desktop-owned candidate, PID 6368, direct child of Desktop PID 10620, seen from first through final sample and alive at the final sample. The separate census results were:

| Count or comparison | Result |
| --- | ---: |
| Pre-existing app-server candidates | 0 |
| New app-server candidates | 1 |
| New Desktop-owned app-server candidates | 1 |
| Owned candidate saved-path matches | 0 |
| Owned candidate expected-SHA256 matches | 1 |

The first failed gate was **B — WRONG SAVED EXECUTABLE IDENTITY**, specifically the harness's saved-path comparison. The test stopped before neutral Send. The observed child had exited by a later read-only process query. The attempt did not preserve the child's executable path string, so the reason for the mismatch between the path comparison and matching hash remains unproven. It may be an actual different path or a comparison/normalization defect. This Task 452 result does not resolve Task 450's historical ambiguity.

Neutral production chat: **NOT RUN**. Real Tool bridge lifecycle, inner execution count, and result correctness: **NOT RUN**. No final deterministic regression, canonical Windows gates, pre-commit remote-master gate, commit, push, exact-head validation, or natural CI was run after the live stop. Task 438 remains **NOT CERTIFIED**. Historical release certification stays at Codex 0.149.0. RAH version 0.32.0, HostExplicit exactly 11, and no selected v0.33 product capability remain the intended unchanged boundaries. No tag or release was made.

Task 453 later measured the same saved Windows file object and matching SHA256 in one new production Connect attempt; see [Task 453 executable file identity audit](2026-09-28-task-453-windows-executable-file-identity-audit.md). This does not change Task 452's historical Gate-1 result or certify Task 438.
