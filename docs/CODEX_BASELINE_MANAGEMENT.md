# Codex Baseline Management

## Purpose

RAH live and release validation must use a host-selected, immutable native Codex
binary. This avoids treating the developer's daily global `codex` installation
or ambient `PATH` order as release evidence. The baseline is release tooling;
it is not a RAH tool and creates no model authority.

Windows x64 baseline management is implemented and verified. Linux, macOS, and
Windows ARM64 require their own native acquisition and validation before being
claimed as supported.

## Tiers and authority

- **Daily:** the globally installed, current developer `codex`; it may be
  upgraded independently.
- **Candidate:** a new exact binary undergoing compatibility review.
- **Certified:** an exact native binary plus SHA-256 approved for RAH release
  and live-regression evidence.

Only the host/release owner chooses a candidate version, executable path, or
promotion decision. A model, `ToolRegistry`, capability profile, or tool input
cannot select the binary, edit a manifest, replace a baseline, or promote a
version. Hashes are integrity evidence, not a substitute for provenance review.

## Store and manifest

The default per-user store is `%LOCALAPPDATA%\codex-baselines`; set
`CODEX_BASELINE_HOME` or pass `-StorePath` for a host-controlled test/store
location. Nothing is written into the RAH repository and no administrator access
is required.

```text
codex-baselines/
  0.149.0/
    codex.exe
    manifest.json
```

`manifest.json` has a closed schema: `manifest_version`, `version`,
`reported_version`, `sha256`, `platform`, `architecture`, `binary`, `source`,
`source_package`, and `archived_at_utc`. The binary must be a regular PE
`codex.exe`, not a PowerShell/CMD/npm launcher.

If a version directory exists with the same SHA-256, `save` is idempotent. A
different SHA-256 fails closed: it is never overwritten, renamed, or promoted.

## Commands

```powershell
.\scripts\codex-baseline.ps1 save 0.149.0
.\scripts\codex-baseline.ps1 verify 0.149.0
$baseline = & .\scripts\codex-baseline.ps1 path 0.149.0
.\scripts\codex-baseline.ps1 list
.\scripts\codex-baseline.ps1 verify-all
```

`path` verifies first and writes only the native executable path to standard
output. Diagnostics use standard error. `save` prefers an isolated exact Windows
platform artifact (`npm install @openai/codex@<version>-win32-x64`) in a temporary
directory, validates its package and native binary, and removes that directory
afterwards. If isolated acquisition is unavailable, it may save only an exact
matching global npm package; it never copies an npm shim. `-SourcePath` is a
host-only recovery/test input for an already acquired native executable.

Before a global upgrade, archive and verify the certified source. Then a daily
upgrade is independent:

```powershell
npm install -g @openai/codex@latest
codex --version
& $baseline --version
.\scripts\codex-baseline.ps1 verify 0.149.0
```

Never commit or publish archived binaries as RAH artifacts.

## RAH live gates

`CodexRuntime::connect(executable)` and `connect_tool_bridge(executable, ...)`
already take an explicit host executable path. On Windows the adapter resolves
that path to a canonical native `.exe`, verifies its exact supported version, and
starts it directly without a shell. The precedence is therefore:

```text
explicit host path passed to CodexRuntime
  -> existing hardened PATH/npm discovery only when the caller passed `codex`
```

There is deliberately no new `RAH_CODEX_BIN` runtime authority. Existing live
examples use the host process variable `RAH_CODEX_EXECUTABLE`; set it from the
verified baseline path, never from model input:

```powershell
$env:RAH_CODEX_EXECUTABLE = & .\scripts\codex-baseline.ps1 path 0.149.0
cargo run -p rah-runtime-codex --example live_smoke
```

The adapter checks `codex-cli 0.149.0` at runtime before app-server startup;
baseline verification additionally checks the manifest, SHA-256, PE/native form,
and platform. Thus a certified live gate has both manifest and child-runtime
version evidence while global `codex` can be newer.

## Promotion

Certification is an explicit review, never an automatic consequence of a newer
`codex --version`:

```text
candidate version
 -> exact binary acquisition and manifest/hash
 -> app-server schema diff
 -> deterministic runtime tests
 -> live adapter smoke
 -> critical live tool gates
 -> platform review
 -> certification decision
```
