# RAH v0.7 Release Gate

Status: **RELEASE PREPARATION**

Date: 2026-08-24

## Release scope

v0.7 extends the existing canonical `repo.patch` tool-input contract. The
legacy single-replacement form remains supported. The mutually exclusive
`replacements` array accepts one through sixteen exact replacements for one
existing, clean HEAD-tracked regular UTF-8 worktree file. All matches are found
in one original snapshot; duplicate or overlapping ranges fail closed, and
accepted non-overlapping ranges produce one deterministic final replacement.
Full-file SHA-256 and byte-length preconditions are required. RAH does not
stage the resulting worktree change automatically.

This is a tool-schema/contract evolution, not a public Rust API break. The
public Rust interfaces, trusted-profile binding format, `profile_version` 1,
and canonical `repo.patch` capability name remain compatible; no migration is
required.

## Milestone evidence

| Task | Evidence |
| --- | --- |
| 070 | v0.7 scope and authority roadmap |
| 071 | bounded multi-replacement contract and ADR 0012 research |
| 072 | implementation and deterministic platform coverage |
| 073 | deterministic Generic Tool Bridge validation |
| 074 | Windows native Codex live multi-replacement validation |
| 075 | Codex platform alignment audit |
| 076 | Windows x64 certified baseline management |
| 077 | milestone audit, initially **NOT RELEASE READY** because of stale README wording |
| 078 | README correction for the multi-replacement contract |
| 079 | re-audit verdict: **RELEASE READY** |
| 080 | release preparation initially blocked when its live gate exposed local Codex configuration drift |
| 080A | diagnosed the drift without changing product authority or `repo.patch` semantics |
| 080B | introduced the isolated certified Codex configuration |
| 080C | made live-gate success host-attested rather than model-text-attested |
| 080R | resumed release preparation using the corrected certified gate |

## Certified runtime

- Certified runtime: exactly `codex-cli 0.149.0`.
- Certified SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Certified live validation explicitly selects the archived host-controlled
  executable path in an isolated temporary `CODEX_HOME`. Its fixed config uses
  model `gpt-5.4`, reasoning effort `medium`, no MCP servers, and disabled
  plugins, apps, code mode, browser use, computer use, and image generation.
- The redacted certified configuration fingerprint is
  `d967dc569062346bb9dd3084fef0f004842e36044a301d49e936a84b31ad0f7d`.
  Authentication, when needed, is an ephemeral copy outside that fingerprint.
  A newer global/daily `codex` installation may differ and is recorded
  separately; it is not certified release evidence.
- Baseline management is verified for Windows x64 first. No claim is made for
  Windows ARM64 or universal binary portability.

## Deterministic evidence

Windows local deterministic release gates cover formatting, workspace check,
tests, clippy, Cargo metadata, and focused `rah-tools`, `rah-runtime-codex`,
and `rah-cli` gates. Ubuntu CI supplies deterministic cross-platform evidence
for the exact release-preparation commit. Neither class is Unix live Codex
validation.

## Live evidence

Live evidence is Windows native Codex only. Historical Task 074 evidence used
the then-current live setup; it is not retroactively claimed to have used the
later isolated configuration. The resumed gate selects the certified archived
0.149.0 executable in an isolated `CODEX_HOME`, starts a fresh app-server and
effective composition, and builds a fresh host-owned `ToolRegistry` for every
run. It uses the Generic Tool Bridge and exactly one three-replacement
`repo.patch` request, requires observers and `Completed`, and reaps the child
and fixture.

Each live multi-patch run asserts one request, start, finish, and native
mutation; three `replacements` entries; exact target postimage; unchanged HEAD,
refs, raw index, and unrelated sentinel; no automatic staging; restricted
Codex-owned write, shell, and network capabilities; and app-server cleanup.
`RAH_ECHO_BRIDGE_OK` and `RAH_MULTI_PATCH_LIVE_OK` are host-emitted only after
their structural assertions and cleanup pass; they are not trusted model
output. The observer gate analogously emits
`RAH_REPOSITORY_OBSERVERS_LIVE_OK`. The separate adapter smoke requires
`RAH_CODEX_SMOKE_OK`.

## Architecture and authority alignment

The app-server remains the primary Codex runtime boundary. RAH does not
implement an inference engine. `ToolRegistry` remains host-owned; MCP and
Process Plugin providers remain ordinary Tool providers. Codex approval does
not grant RAH authority, and Codex sandboxing does not replace RAH host policy.
RAH Sessions and Codex threads are distinct concepts.

Model output is a request, never authorization. `repo.patch` does not grant
shell/process authority, automatic rollback, a worktree transaction, or Git
history authority. Unknown or uncertain effects fail closed and are never
automatically replayed.

## Intentional limitations

- Existing tracked file only; no file creation, deletion, or rename.
- No multi-file edit transaction or arbitrary unified-patch ingestion.
- No Git commit, ref/history, or network Git authority.
- No generic shell/process authority and no automatic rollback.
- Uncertain effects are not replayed.
- Session/workflow persistence, network MCP, PluginManager, and profile reload
  remain deferred.
- Windows live validation only; Ubuntu evidence is deterministic only.
