# Task 217 — Trusted Profile Persistence Lifecycle Validation

## Initial validation result

**INCONCLUSIVE.** The real Windows Desktop restart and inert persistence phases
were observed. Choose, restart, missing-source startup, missing-source Restore,
and explicit Restore all behaved inertly as required. Connect crossed the
provider activation boundary and both providers were spawned and then reaped,
but Connect could not succeed because the installed certified Codex baseline
store was invalid for the current Desktop baseline contract. Therefore the
post-Connect Effective Authority, Forget-while-connected, normal Disconnect,
and restart-after-Forget phases could not be certified.

No RAH product defect was established. No product or fixture changes were made.

## Authoritative starting checkpoint

- \`HEAD\`: \`b168770350e6f3ab87df010267ad99f37993ea1f\`
- \`origin/master\`: \`b168770350e6f3ab87df010267ad99f37993ea1f\`
- prior Task 216 exact-head CI: \`33995512974 PASS\` (supplied task evidence)
- worktree was clean before validation.
- RAH v0.17.0, 13 packages, Rust edition 2024.

## Validation scope

This validation covered the host-owned Trusted Profile persistence lifecycle:
real Desktop process close/restart, remembered-only startup, missing-source
startup, explicit static Restore, provider activation at Connect, provider
cleanup, and privacy boundaries. Model-selected external Tool execution was
not required and was not attempted or claimed.

## Fixture/environment isolation

The real Windows release Desktop binary was used. Tauri's
\`app_local_data_dir()\` was verified from the Desktop source and resolved to the
app-owned \`%LOCALAPPDATA%\\org.rust-agent-harness.desktop\` directory. No
pre-existing \`desktop-preferences.json\` was present, so no user preference
backup was needed. The test-created preference was moved out of the app-owned
directory after validation and retained temporarily as recoverable evidence;
the app-owned preference path was restored to its pre-test absent state.

Fixture files were copied into one fresh temporary Task 217 directory, referred
to here as \`C:\\Temp\\rah-task217-<nonce>\\\`. The shared target directory was not
used as the live provider execution directory. The live fixture directory
contained the copied MCP and Process Plugin executables, lifecycle request
markers, and the mixed profile. Provider lifecycle files were absent before
Connect/failed Connect cleanup and were inspected after each phase.

## Mixed Trusted Profile fixture

The provider-only profile had profile ID \`task217-persistence-live\`, no
first-party capabilities, one local stdio MCP provider, and one Process Plugin
provider. Static CLI validation passed and reported two configured providers,
each with one configured Tool. The expected public identities were:

- \`mcp.task217-mcp.echo\`, permission \`Read\`;
- \`plugin.task217-plugin.echo\`, permission \`Execute\`.

The copied provider SHA-256 values were recorded privately with the fixture
setup. No provider process was started during fixture creation or static
validation.

## Deterministic validation

- \`cargo fmt --check\`: PASS.
- \`cargo check --workspace\`: PASS.
- \`cargo test --workspace\`: known Windows fixture debt only; 171 Desktop tests
  passed, 1 known Git-owner diagnostic failed, and 2 were ignored before the
  workspace test command stopped at that failure.
- \`cargo clippy --workspace --all-targets --all-features -- -D warnings\`: PASS.
- \`git diff --check\`: PASS before documentation.
- \`cargo metadata --no-deps --format-version 1\`: PASS.
- \`cargo build -p rah-desktop --release\`: PASS.
- \`cargo test -p rah-desktop\`: 171 passed, 1 known Windows Git diagnostic
  failure, 2 ignored.
- \`cargo test -p rah-profile-composition\`: PASS, 4 tests.
- \`cargo test -p rah-tools-mcp\`: PASS, 32 tests including 31 stdio tests.
- \`cargo test -p rah-tools-plugin\`: PASS, 21 tests including 15 process tests.
- \`cargo test -p rah-runtime-codex\`: PASS, 82 passed, 1 ignored.
- frontend \`node --check\` and \`status_authority_test.js\`: PASS.
- static mixed-profile validation: PASS; provider-only rule and no first-party
  capability declaration were accepted.

## Phase 1 — fresh startup

PASS. A fresh real Desktop process started with no remembered profile, no
selected profile, no Effective Authority external inventory, and no provider
lifecycle files. The UI showed the no-profile state.

## Phase 2 — Choose and persist

PASS. The native Choose Profile action selected the exact mixed fixture. The UI
showed configured providers inactive, the sanitized profile ID, two providers,
and two expected external Tools. The private v3 preference record contained
only the remembered source path in the Trusted Profile preference field plus
the inherited model preference. The raw path was not used as public evidence.

No MCP or Process Plugin lifecycle file existed after Choose. The Desktop was
closed normally and still no provider lifecycle file existed after close.

## Phase 3 — restart remembered-only

PASS. A new Desktop process was launched after the first process had exited.
The UI showed \`Remembered — not restored\`; it did not show configured profile
metadata or Effective Authority external Tools. No provider lifecycle file was
created and no provider process was started.

## Startup missing-source proof

PASS. After closing the remembered-only process, the profile source was
temporarily renamed to a missing-source filename. A further new Desktop
process started successfully and remained \`Remembered — not restored\` without
source-dependent startup failure or provider lifecycle files. Clicking Restore
while the source was missing produced a bounded failure; the remembered
preference remained and no provider was spawned. The source was restored to its
original path before continuing.

## Phase 4 — explicit Restore

PASS. With the source restored, a new Desktop process started in remembered-only
state. Explicit Restore validated the current source and changed presentation
to configured/providers inactive. Both lifecycle files remained absent. No
provider process was spawned and no authority was restored by persistence or
Restore.

## Phase 5 — Connect/provider activation

INCONCLUSIVE due to the certified Codex environment. The Connect action was
attempted in the real Desktop. Both providers did activate at this explicit
boundary: each audit recorded two bounded sequences of \`spawn\`, \`shutdown\`,
\`exit\` while the failed connection was cleaned up. No \`call\` event occurred,
which is valid for this task.

Connect did not succeed because the installed baseline store contained the
correct certified \`codex.exe\` SHA-256 but an obsolete manifest contract:
\`manifest_version\` was \`1\`, \`code_mode_host\` and
\`code_mode_host_sha256\` were absent, and \`codex-code-mode-host.exe\` was
absent. The current Desktop requires the v2 baseline manifest and the companion
code mode host. The baseline verification script reproduced the same failure.
This was an environment prerequisite failure, not a RAH lifecycle defect, and
no override or baseline mutation was used.

Consequently, successful Connect, Effective Authority publication, public Tool
inventory after successful connection, and current/active connection status
were not established.

## Phase 6 — Forget while connected

NOT ESTABLISHED. There was no successful connected/current state in which to
perform the required preference-only Forget observation.

## Phase 7 — Disconnect/provider reap

NOT ESTABLISHED as a normal Disconnect phase. Failed Connect cleanup did
produce \`spawn\`, \`shutdown\`, \`exit\` for both providers. Both copied provider
executables were then successfully renamed to \`.released\` and renamed back,
showing that cleanup left no file lock. This is cleanup evidence, not a claim
that the normal Disconnect action was certified.

## Phase 8 — restart after Forget

NOT ESTABLISHED because Phase 6 could not be performed while connected.

## Lifecycle audit evidence

Before Choose, after Choose, after each inert restart, after missing-source
startup, and after explicit Restore: no lifecycle file existed.

After the failed Connect attempt, the private fixture audits were:

\`\`\`text
MCP:
spawn
shutdown
exit
spawn
shutdown
exit

Process Plugin:
spawn
shutdown
exit
spawn
shutdown
exit
\`\`\`

The evidence contains no \`call\` event. No model Tool request, ToolStarted, or
ToolFinished claim was required or made.

## Effective Authority evidence

Not established after a successful Connect. The provider composition was
created during the attempted Connect and was cleaned up when Codex baseline
validation failed, but the Desktop never published a successful connected
Effective Authority snapshot. Therefore no public external Tool inventory or
permission presentation is claimed by this validation.

## Privacy review

- Raw remembered source path: present only in the private preference record;
  redacted from this document and public evidence.
- Raw path in normal Trusted Profile UI: not observed.
- Raw path in Effective Authority, Tool identities, activity feed, or
  conversation: not observed; no successful connection or chat occurred.
- Private provider executable paths: not included in public lifecycle evidence.
- No secrets or tokens were used.

## Windows live result

**INCONCLUSIVE.** The required real Desktop persistence, restart, missing-source,
Restore, and inert provider-boundary observations passed. Required successful
Connect and post-Connect phases were blocked by the invalid certified Codex
baseline environment.

## Linux/cross-platform result

- Linux deterministic coverage: PARTIAL. Cross-platform provider composition,
  profile parsing, and provider adapter tests are covered by the workspace
  suite; the production Desktop is Windows-gated and the relevant live path was
  not run on Linux.
- Linux live lifecycle: NOT ESTABLISHED.

## Known Windows fixture debt

\`tests::hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic\`
failed in both focused Desktop and workspace test runs because the diagnostic
did not reproduce Git's protected foreign-owner refusal. This is the known
pre-v0.17 Windows fixture debt. It was not hidden, modified, or treated as a
Task 217 lifecycle defect.

## Security claim boundary

The observed evidence supports that persistence does not restore authority:
startup retained only a remembered path, missing source did not activate
anything, Restore statically validated without spawning, and Connect was the
first observed provider activation boundary. The failed Connect cleanup also
reaped both providers.

This validation does not claim model-selected external Tool execution,
ToolRequested/ToolStarted/ToolFinished, external-effect invalidation, OS
sandboxing, network isolation, rollback, or absence of ambient provider
effects. Task 207 remains unchanged. No ADR or authority semantics changed.

## Cleanup

The Desktop process was closed normally. The source profile was restored to its
original fixture name. Both provider executables were restored after the
path-specific rename/unlock check. The newly created app-owned preference was
moved out of the app directory as recoverable Task 217 evidence; no pre-test
preference backup existed and no unrelated application data was removed.

## Final milestone-validation conclusion

Task 217 cannot claim PASS. The lifecycle observations that were reachable all
matched the contract, but a required successful certified-Codex Connect and
the connected-state phases were not observable in the supplied environment.
The correct disposition is **INCONCLUSIVE**, not PASS and not FAIL.

## Next task at the initial checkpoint

Task 218 — RAH v0.18 Milestone Audit is **not started**. First restore a valid
certified Codex 0.149.0 baseline bundle (including the current v2 manifest and
code-mode host), then rerun the missing connected-state phases under a fresh
Task 217 fixture/evidence directory. Do not treat the current result as Task
218 input sufficient for release readiness.

## Task 217A prerequisite repair

The initial result above remains **INCONCLUSIVE** and its evidence is not
rewritten. Task 217A repaired the host baseline through the explicit host-only
baseline repair workflow:

- Task 217 initial evidence checkpoint: `2c8d347c44c98d5423a640b16e6a555552b4331a`.
- Task 217A implementation/current exact head: `ea80aac6980335028418f66b958dc9b625c1cb3f`.
- Task 217A exact-head CI: `34001135249 PASS`.
- Certified baseline: `codex-cli 0.149.0`.
- Baseline manifest: version 2, including the verified code-mode host contract.
- `codex.exe` SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- `codex-code-mode-host.exe` SHA-256: `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.
- `scripts/codex-baseline.ps1 verify 0.149.0`: PASS.
- `scripts/codex-live-gate.ps1 -Version 0.149.0 -PrepareOnly`: PASS.

The live gate selected an isolated temporary `CODEX_HOME`, disabled
Codex-owned shell/file/MCP/apps/plugins/code-mode/browser/computer/image
surfaces, and retained the accepted model `gpt-5.6-terra`. No PATH fallback or
automatic repair was used.

## Resumed connected-state validation

The resumed run used a new private fixture represented publicly as
`C:\Temp\rah-task217-resume-<nonce>`. The release Desktop binary was
`target/release/rah-desktop.exe`. The actual app-owned preference location was
`%LOCALAPPDATA%\org.rust-agent-harness.desktop\desktop-preferences.json`.
It was absent before the run, so no backup was required. The preference path
was absent again after cleanup; the post-Forget document was retained only in
the private resume fixture as evidence.

The fixture contained one provider-only profile, `task217-persistence-live`,
with MCP Tool `mcp.task217-mcp.echo` at `Read` and Process Plugin Tool
`plugin.task217-plugin.echo` at `Execute`, with `capabilities: []`. Fresh
copied executables and lifecycle request markers were used. Both lifecycle
files were absent before Connect.

### Setup to restored state

PASS. Choose Profile produced `remembered=true`, `selected=true`, and
`Configured — providers inactive`; no provider lifecycle file existed. After a
normal close and new process start, the app showed `Remembered — not restored`
(`remembered=true`, `selected=false`) with no provider activation. Explicit
Restore returned to `Configured — providers inactive`, with both lifecycle
files still absent.

### Successful Connect

PASS. Connect succeeded with the repaired certified baseline. The Desktop
reported `codex-cli 0.149.0`, active profile, and connected runtime. Each
provider lifecycle file contained exactly `spawn` during the stable connected
phase; neither contained shutdown or exit. No chat prompt was sent and no
model-selected external Tool call was attempted.

### Effective Authority

PASS. The actual Desktop Effective Authority surface showed `Current` and the
connected UI showed `Disconnect Codex`. The host snapshot was
`connected_current` with captured repository/model/connection generations
`0/0/1`, and advertised runtime state. The effective external entries were:

- `mcp.task217-mcp.echo`: source `MCP`, permission `Read`, external provider;
- `plugin.task217-plugin.echo`: source `Process Plugin`, permission `Execute`,
  external provider.

Provider labels were sanitized identifiers. No executable path, Trusted
Profile source path, or private Codex alias appeared in the UI or authority
snapshot.

### Forget while connected

PASS. Forget succeeded while chat was idle and the runtime was connected/current.
Immediately afterward, `remembered=false` and `selected=true`; the profile and
connection generations were unchanged; the connection remained
`connected_current`; both external Tools and the Effective Authority snapshot
were unchanged. Both lifecycle files remained exactly `spawn` with no
shutdown/exit. The private preference document no longer contained
`trusted_profile`; its existing model preference remained.

### Normal Disconnect

PASS. The normal connected Disconnect action returned the Desktop to
`not connected` / configured providers inactive and withdrew the effective
external inventory. Both providers produced exactly:

```text
spawn
shutdown
exit
```

No additional spawn sequence and no Tool call occurred.

### Executable unlock

PASS. After normal Disconnect, `task217-mcp.exe` and `task217-plugin.exe` were
each renamed to `.released` successfully and then restored to their original
fixture names. No global process-name termination was used as evidence.

### Restart after Forget

PASS. A brand-new Desktop process started after Forget and normal cleanup.
It reported `remembered=false`, `selected=false`, `profileStatus=not loaded`,
`profile_generation=0` equivalent through the unselected initial state, no
provider activation, no external Tool inventory, no implicit Restore, and no
implicit Connect. The lifecycle files remained at the single completed
`spawn/shutdown/exit` sequence from the prior normal connection.

## Final Task 217 result

**Task 217: PASS.**

The initial attempt was INCONCLUSIVE because the certified 0.149.0 baseline
store was legacy v1 and lacked the current v2 companion code-mode-host
contract. Task 217A repaired that environment through the explicit host-only
baseline repair workflow. The resumed Task 217 run then established the
missing connected lifecycle without changing product code or authority
semantics.

Exact claim:

**Windows Trusted Profile persistence lifecycle: PASS.**

Verified: persistence restores no provider authority; restart is
remembered-only; Restore is non-spawning; Connect is the first effective
activation boundary; successful Connect activates the admitted MCP + Process
Plugin composition; Forget while connected changes only durable preference;
normal Disconnect reaps providers; and restart after Forget restores no profile
preference or runtime authority.

Still not claimed: model-selected external Tool execution,
`ToolRequested`/`ToolStarted`/`ToolFinished` external execution, external
repository-effect live validation, OS sandboxing, network isolation, rollback,
absence of provider ambient effects, or Linux Desktop live validation. Task 207
remains historically unchanged. Task 218 is not started.

## Deterministic validation for the resumed completion

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test -p rah-profile-composition`: PASS, 4 tests.
- `cargo test -p rah-tools-mcp`: PASS, 32 tests including 31 stdio tests.
- `cargo test -p rah-tools-plugin`: PASS, 21 tests including 15 process tests.
- `cargo test -p rah-runtime-codex`: PASS, 82 passed, 1 ignored.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS.
- `cargo build -p rah-desktop --release`: PASS.
- `node --check` for `status.js` and `status_authority_test.js`: PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js`: PASS.
- `scripts/test-codex-baseline.ps1` with the installed native Codex: PASS.
- `scripts/codex-baseline.ps1 verify 0.149.0`: PASS.
- `git diff --check`: PASS.

`cargo test -p rah-desktop` and `cargo test --workspace` each reproduced the
same known Windows fixture debt:
`tests::hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic`
failed because the local diagnostic did not reproduce Git's protected
foreign-owner refusal. The focused Desktop result was 171 passed, 1 failed,
2 ignored. This was not hidden, modified, or treated as a Task 217 lifecycle
defect.
