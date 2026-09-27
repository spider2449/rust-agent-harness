# Task 437 — Inherited Codex Authentication / Provider Configuration Recovery and Neutral Chat Recertification

Date: 2026-09-27

Type: environment/configuration recovery and live certification

Outcome: **B — EXTERNAL CODEX CONFIGURATION BLOCKER REMAINS**

## 1. Starting checkpoint

- Required HEAD: `4c212d15c0ef72e0dcee9a019d992071e7f79649` — PASS.
- Fetched `origin/master`: `4c212d15c0ef72e0dcee9a019d992071e7f79649` — PASS.
- `origin/master...HEAD`: `0 0` — PASS.
- `git merge-base --is-ancestor origin/master HEAD`: PASS.
- `git status --short`: no entries — clean worktree and index.
- Task 436 exact-head CI `36302333148`: PASS (checkpoint supplied for this task).

## 2. Task 436 verdict and preserved conclusions

Task 436 concluded **B — POST-START FAILURE IS EXTERNAL / CONFIGURATION-OWNED**.
The certified Codex 0.149.0 app-server accepted thread and turn starts, then
reported `turn/completed(status=failed, codexErrorInfo=other)` after an upstream
HTTP 400 request rejection with authentication/account-related wording. Its
exact authentication/account subtype remained unknown. RAH correctly emitted
`AgentEvent::Failed(Internal)` and displayed `Chat failed`.

The prior conclusions about PATH Codex 0.157.1, Tauri permissions, synchronous
submission rejection, stale generations, thread/runtime startup, event
receiver lag, stream exhaustion, Tool dispatch, repository authority,
HostExplicit, and terminal mapping remain closed. Task 437 found no
contradictory evidence requiring those hypotheses to be reopened.

## 3. Certified Codex identity

The established `scripts/codex-baseline.ps1 verify 0.149.0` check passed.
The `path 0.149.0` command selected:

- Executable: `C:\Users\morefunfun\AppData\Local\codex-baselines\0.149.0\codex.exe`
- Reported version: `codex-cli 0.149.0`
- SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- Runtime selection source: `certified_baseline`.
- Baseline manifest package provenance: `@openai/codex@0.149.0-win32-x64` (`npm-isolated`).

The certified baseline was not changed. PATH Codex 0.157.1 was not used.

## 4. Authentication status method and result

The exact executable's `--help` listed `login` as the login-management
command. Its `login --help` explicitly listed `login status` as “Show login
status”; that read-only command was used. Its result was sanitized immediately
and only the status category and mechanism were retained:

- Before: **authenticated**.
- Mechanism/category: **ChatGPT account**.
- Exit code: 0.
- Expired/invalid local login: not indicated.
- Reauthentication required: not indicated by this status command.

The pinned executable's redacted `doctor --json --summary` command also exited
0 and reported the authentication and provider diagnostic checks as `ok`.
Neither surface established that this account is entitled to the inherited
model or that the account can complete a model request. No credentials or
account identifiers were inspected or recorded, and no login/recovery flow was
started because local authentication was reported as valid.

## 5. Inherited provider/model source

The inherited config root was the default `%USERPROFILE%\.codex` because
`CODEX_HOME` was not set. Inspection read only `model` and `model_provider`
assignments from `config.toml`, without printing other configuration fields:

- Model: `gpt-6-luna`, set in the default `config.toml`.
- Provider: `openai`, as observed from the effective app-server configuration
  in Task 436; no explicit `model_provider` assignment was present in the
  inspected default config, so provider identity is supplied by Codex's
  provider selection/default for this inherited model configuration.
- Model/provider environment overrides: none detected.

No alternate profile, model catalog, or account-specific model-availability
surface was established by the pinned CLI help or redacted doctor report.
Task 437 did not select a replacement model or alter user configuration.

## 6. Direct certified-Codex control test before recovery

The pinned executable's `exec --help` documented `--json`, `--ephemeral`,
`--skip-git-repo-check`, `--sandbox`, and `-C`. The direct control used the
exact certified executable with the inherited model/provider configuration,
an ephemeral session, a temporary working directory, read-only sandbox, and a
minimal no-tools prompt requesting `RAH437_CODEX_DIRECT_OK`.

Result: **FAIL**. The command produced five JSON events, including a failed
terminal event, and exited 1. The turn failed before any assistant output or
requested marker. Sanitized classification found authentication/account
wording. This direct CLI result did not expose a confirmed numeric HTTP status;
therefore Task 437 does not claim that the direct error independently confirmed
the HTTP 400 seen in Task 436. The effective provider/model were not included
in the emitted direct CLI fields; the process used the inherited configuration
described in section 5 without command-line overrides.

There was one preceding invocation that the CLI rejected as an unsupported
argument before a model turn started (exit 2, no JSON events). It was a local
invocation error, not a Codex/provider result. The retry used only options
listed by the exact executable's `exec --help`.

## 7. Recovery and post-recovery direct test

No recovery action was taken. `login status` and the redacted doctor report
classified local authentication as valid; neither indicated that reauthentication
was required. The account/model request still failed, and the available
supported CLI surfaces did not identify a known-good model/provider pair.

No post-recovery direct test was reached because no authentication or
configuration recovery was justified by available evidence. A successful
direct turn is a prerequisite for any RAH Desktop recertification.

## 8. Production Desktop neutral chat runs

Not reached. The direct certified Codex control failed, so Task 437 did not
build or launch Desktop for chat testing. No result is claimed for the three
neutral production conversations, their markers, `thread_start`,
`desktop_completed`, Tool activity, UI ready state, or `desktop_failure`.

## 9. Repository-context no-Tool run

Not reached. The three required neutral Desktop passes were not available, and
the direct Codex prerequisite remained failed. The Task 434 fixture was not
selected or changed.

## 10. Optional repository Tool observation

Not reached. Task 437 did not advertise or dispatch repository Tools in a live
conversation. No Tool dispatch result is claimed.

## 11. RAH source and certification scope

No RAH production source, tests, configuration defaults, certified manifest,
provider selection, or supported Codex version were changed. The expected
source outcome is documentation only. Since direct Codex did not complete, no
production Desktop build or RAH live recertification was performed.

v0.33 product capability remains **NONE SELECTED** and HostExplicit remains
exactly 11.

## 12. Privacy and security handling

No tokens, cookies, credentials, authorization headers, account identifiers,
raw configuration dump, raw CLI error text, or private upstream response text
were retained in this artifact. The local status and doctor output were used
only to retain sanitized authentication/provider check categories. The direct
failure was reduced to exit/terminal/output facts and authentication/account
wording. No permission, authority, provider trust, or repository boundary
changed.

## 13. Error-presentation observation

The generic `Chat failed` display did make diagnosis materially difficult: the
visible terminal did not distinguish an upstream model/provider request
rejection from a RAH internal runtime failure. Task 436 required separate
diagnostic instrumentation to classify the failed Codex turn. No UI change is
made here. A later bounded UX/error-classification research task may assess a
privacy-safe distinction between a model/provider request rejection and an
internal runtime failure without exposing upstream text.

## 14. Validation and Git state

Documentation-only close validation:

- `git diff --check` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages, 13 workspace members, version `0.32.0`, edition `2024`.
- RAH production source change: none.
- Documentation commit `862731d9e7792f242595c6bf44c2c783ec81616f` was pushed to
  `origin/master`; exact-head push CI `36302936176` passed all required jobs:
  formatting, workspace check, workspace tests, Clippy lint, and Desktop
  Tauri permission check.

## 15. Final outcome

**B — EXTERNAL CODEX CONFIGURATION BLOCKER REMAINS.** The certified Codex
0.149.0 CLI is locally authenticated through a ChatGPT account, but its
minimal direct model turn fails before assistant output with
authentication/account wording. The supported local status and doctor surfaces
do not establish account entitlement to `openai` / `gpt-6-luna`, and no
accepted replacement was exposed. RAH production chat recertification is
therefore not claimed.

## 16. Exact next-step recommendation

The host/account owner should verify that the authenticated ChatGPT account is
entitled to use the configured `gpt-6-luna` model with the `openai` provider,
or configure a model/provider combination explicitly shown as supported for
that account by Codex/OpenAI. Preserve the certified 0.149.0 baseline. Then
rerun `codex login status`, confirm the direct `RAH437_CODEX_DIRECT_OK` turn
completes, and only afterward resume the three neutral production Desktop
runs and repository-context gate. Do not change RAH defaults to match a local
account configuration.
