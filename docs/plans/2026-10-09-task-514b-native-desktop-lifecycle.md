# Task 514B — native Desktop conversation lifecycle

Starting HEAD/origin/master/live GitHub master: `4f6315224c2d813dad6503a890e565de52723188`.
Tracked checkout clean; preserve untracked Task 514 report unchanged and unstaged.

1. Reproduce second-turn failure with production frontend/IPC composition and trace
   backend conversation, terminal ownership, host lease, disconnect and selection.
   Preserve first failing diagnostic before source correction.
2. Correct only demonstrated lifecycle behavior; add deterministic regressions.
3. Run focused native/runtime/Desktop/frontend checks, then required workspace gates.
4. Build Windows Desktop and provide manual acceptance if WebView controls remain
   inaccessible. Never infer UI PASS from deterministic/backend evidence.
5. Commit bounded files, push master, verify exact-head natural CI. No release/tag.

Actual prior manual results: llama.cpp connect/first turn PASS; second turn,
disconnect/reconnect and provider switch FAIL. Tool/cancel/missing-key UI untested;
OpenAI live unverified.

## Preserved diagnosis

`target/task514b/first-failing-frontend.log`: production submit handler plus
production terminal-event handler, completion delivered before send_chat IPC
reply; exit 1, `chatRunning` true instead of false. The next submit follows the
cancel path, Disconnect is disabled, and disconnected provider controls remain
busy. IPC dispatch returns after spawning the chat task; event/reply ordering is
not guaranteed. This is a frontend ownership-ordering defect, not evidence of a
stale host lease.

`backend-before-fix.log` and `backend-diagnostic.log` each ran one production
Desktop diagnostic successfully. First command began before the new test file
was written, but the compiler included the test (one test actually ran); it is
not a zero-test PASS. Later strengthened coverage supersedes these diagnostics.
Backend send_chat -> neutral conversation -> native llama.cpp fixture completes,
commits history, returns Desktop coordinator/chat to Idle, and admits another
turn. Disconnect revokes admission, shuts down the runtime and drains the scope;
configuration is accepted after disconnection. This intermediate permissive
fixture did not reproduce the real llama.cpp request rejection. The actual
Windows diagnosis and stricter failing fixture below supersede that nonclaim.

## Bounded correction and regressions

Reserve frontend chat ownership before sending IPC; never reassert it after the
reply. Start rejection clears it and refreshes controls. A status-refresh failure
after accepted dispatch does not masquerade as a rejected turn.

Frontend tests execute the actual production handlers and control-rendering
statements: completion/failure/cancellation before and after IPC reply, rejected
start, second-message Send dispatch, enabled Disconnect and recoverable provider
controls. The CI workflow runs this regression.

Final focused native Desktop fixture: six completed turns across three distinct
runtime/conversation handles; direct reconnect, then OpenAI missing-key rejection
and return to llama.cpp. Wire input proves retained intended history and model
configuration changes invalidate old history. Active Desktop ownership is absent
after completion; retained Tool ports fail admission after Disconnect. A test-only
conversation ID accessor supports identity checks without changing runtime APIs.
Configured developer credentials are excluded in an isolated child test process,
without modifying the parent environment or sending an OpenAI request.

Intermediate fixture PASS (1/0); native adapter PASS (22/0); neutral runtime PASS (18/0);
existing OpenAI production composition fixture PASS (1/0). Frontend suites,
including browser layout, and Tauri command inventory PASS (49 commands).

Remaining gates and actual Windows acceptance are recorded below before commit.

### Additional preserved harness failure

An explicit child-process credential-isolation check exited 101: its inner test
timed out at the ten-second Idle wait (`credential-isolation.*.log`). The ongoing
workspace build was stopped with an identity-verified runner-tree termination;
`validation-stop-owned-processes.json` preserves PID, parent and creation-time
ownership. Workspace/quality/build/UI/publication gates were not declared PASS.

The test was continuously polling Idle with yield_now while its HTTP fixture
needed the same current-thread Tokio IO driver. Tokio explicitly does not
guarantee IO-driver progress at every yield. This is an inadequate synchronization
strategy; the timeout alone does not prove a provider or credential defect.
The corrected test first awaits the actual terminal UI event on an owned channel,
then checks Idle, active ownership and completed history. The ten-second deadline,
six completion assertions, wire history, identity and revocation checks are kept;
failure/cancellation now fail immediately. Stage/event diagnostics are retained.
No causal claim about machine load is made from the timeout.

Event-driven focused coverage passed (1/0, six completed turns). The same explicit
credential-isolation child path passed with the corrected synchronization (exit
zero). Original failing logs remain untouched; assertions/deadline were not
relaxed. Validation resumed from the interrupted workspace gate, not by replaying
earlier adapter gates.

For actual UI acceptance, Microsoft documents a child-process
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=...` route:
[WebView2 debugging](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code).
If available, acceptance uses real production DOM controls and CDP mouse clicks,
rendered transcript/control observations and screenshots, not backend IPC calls.
No registry, host configuration, installed dependencies or `.vscode` changes.

## Quality gates measured

- `cargo test --workspace`: exit zero, 1073 passed / 0 failed / 18 ignored.
  Its Windows Desktop component passed 335 / 0 / 13.
- `cargo fmt --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `git diff --check`: each exit zero (`gate-exits.txt`).
- Final focused production fixture with admitted repository: 1 / 0; six
  responses, three fresh conversations, three successful host-authorized
  repo.status observations followed by retained-port rejection/revocation.
- All nine frontend suites passed; Tauri inventory passed with 49 commands.
- `cargo build -p rah-desktop --release`: exit zero. Existing default-build
  dead-code warning groups in uncompiled legacy branches remain; all-features
  warnings-denied clippy passed. Normal artifact has no fixture feature.

Production executable: `target/release/rah-desktop.exe`; SHA-256
`D58AD4FB473FA35BCF1E08061859C90331B7BEAED323E8A52651D4AE1E72FB3F`.
No Cargo manifest/lock, dependency edge, accepted ADR, public runtime contract,
Tool inventory, authorization, credential handling or version change.
HostExplicit exact allowlist test passed: eleven existing names.

The pre-existing untracked Task 514 report remains unstaged and byte-identical:
SHA-256 `DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43`.

## Actual Windows failure and provider encoding correction

The first real Windows WebView run connected and completed its first response,
but the second request failed with provider_rejection. The corrected frontend
returned to Idle with Send/Disconnect enabled. Original actual evidence and
screenshots remain in target/task514b/pre-native-fix (and original failure files).
A bounded exact replay of the second request to the configured loopback server
returned HTTP 400: Cannot determine type of 'item'. Its replayed assistant item
was {role: assistant, content: string} without a type discriminator.

The official llama.cpp Responses converter requires role assistant plus type
message for replayed output. Reference:
https://github.com/ggml-org/llama.cpp/blob/master/tools/server/server-chat.cpp
The Task514B production-composition fixture was strengthened to reject the same
invalid assistant encoding. Before correction it failed on the second terminal
(provider_rejection, expected completed), exit 101; preserved in
llama-contract-before-fix.log. After correction the same fixture completed all
six turns, exit zero (llama-contract-after-fix.log).

The adapter now adds type message only to llama.cpp assistant replay and
rechecks the existing payload bound before host lease admission. Official
OpenAI wire encoding stays unchanged, proven by a provider-specific request
regression. No lease, runtime abstraction, credential or authority relaxation.
The frontend correction handles the independently demonstrated terminal-before-
IPC-reply ordering defect. Thus there was no demonstrated single stale-lease
cause of all three originally reported workflows. Real workflows must be
retested using the corrected normal production release executable.

Earlier quality counts and executable hash above describe the frontend-only
intermediate build. Final post-provider-fix gates use final-*.log;
actual acceptance uses final-ui/ and does not overwrite the failed diagnostic.

## Final local gates and actual Windows acceptance

Normal production release SHA-256:
49B5FA68D4E13D71ECDEC6CBA42C6C3B046D28BF2B29393DE323C973CF6BD577
Executable: target/release/rah-desktop.exe (normal features, no fixture).

All final sequential gates exited zero: native adapters (23 passed), neutral
runtime (18 passed), helper build, existing production OpenAI composition
fixture (1 passed), workspace (1074 passed / 0 failed / 18 ignored), fmt,
workspace check, all-target/all-feature warnings-denied clippy, release build,
and diff check. Canonical Desktop within workspace: 335 passed / 0 failed /
13 ignored. All nine frontend suites and 49-command Tauri inventory passed.
HostExplicit exact allowlist and eleven-kind composition tests passed.

Actual Windows acceptance used the owned production WebView2 target in a visible
RAH window, DOM controls and CDP mouse input. Child OpenAI key absent. Four
rendered/streamed responses and four corresponding host desktop_completed
events: first and second share runtime generation 1, reconnect uses 2, and
return after OpenAI failure uses 4. Disconnect observations show zero effective
Tools and unlocked provider controls; completion shows Send, enabled prompt,
chatRunning false and enabled Disconnect. The second response recalled the
first marker. All three requested actual UI workflows PASS.

Evidence: target/task514b/final-ui/ui-evidence.json, ui-host-events.jsonl,
ui-listener-ownership.json, ui-launch.json, workflows.log and four screenshots.
The launch wrapper initially supplied an incorrect Node output-directory
argument and failed before UI interaction; acceptance.log preserves that
harness error. Correcting only that argument allowed the same owned window to
run the workflows once, exit zero. cleanup.json proves identity-checked normal
window close. No product failure was retried to obtain this PASS.

Concise manual verification with the tested executable:
1. With local llama.cpp serving its model, select llama.cpp and Connect.
2. Send Reply with exactly: RAH514B_FIRST_OK; after completion ask which marker
   was requested. Confirm the second completed answer and enabled Send.
3. Disconnect, Connect, request RAH514B_RECONNECT_OK and confirm response.
4. Disconnect, select OpenAI with a model and no OPENAI_API_KEY, Connect.
   Confirm the missing-credential message and editable provider controls.
5. Select llama.cpp, Connect, request RAH514B_SWITCH_OK, confirm response,
   then Disconnect.

Actual Tool round-trip and cancellation remain NOT TESTED. Live OpenAI remains
NOT VERIFIED. The missing-key UI recovery is now PASS; it does not establish
live OpenAI success. No release preparation, tag or version change.

Commit/push/natural exact-head CI are the remaining publication steps at this
report's commit boundary; final closure records their exact results separately.
Classification A and permission to begin Task514C require their successful
completion in addition to the local and actual UI evidence above.
