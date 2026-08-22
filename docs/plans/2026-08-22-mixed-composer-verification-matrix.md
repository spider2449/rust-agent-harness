# Task 041 plan: mixed-composer verification matrix

## Actual seam and ordering

The exercised seam is `rah-cli/src/profile_composition.rs::compose`, used by
`rah profile validate-effective`. It builds a fresh registry in this order:
built-ins, all MCP providers, then all Process Plugin providers. The returned
aggregate owns all admitted adapters; explicit shutdown is Plugins then MCPs.

## Deterministic evidence

- Test-copied echo fixture executables opt into an adjacent bounded lifecycle
  audit only when a test-created request marker exists. Normal fixture runs do
  not write it. Tests observe `spawn`; graceful commands also record
  `shutdown`.
- After owner release, a bounded rename/unlock check against each copied
  executable proves no fixture process retains it. This avoids process-name
  polling and never targets an unrelated process.
- Mixed success proves both proxy calls, profile-owned `Read` MCP and `Execute`
  Plugin permissions, redacted inventory, provider usability while owned, and
  provider release after owner drop.
- MCP staged then Plugin schema failure, MCP plus Plugin staged then later
  Plugin failure, and MCP A then MCP B failure return only an error: neither a
  registry nor effective inventory is available. Every started fixture unlocks.
- CLI `profile validate` with both providers records zero spawns. CLI
  `validate-effective` records both spawns, prints only logical provider/tool
  state, and releases both fixtures before exit.

## Collision analysis

`ToolRegistry` deterministically rejects duplicate `ToolName` and retains the
original tool; focused coverage proves this. No external collision is
representable through the closed trusted-profile schema: provider IDs are
unique across MCP and Plugin, remote names are unique within a provider, and
adapter names are qualified as `mcp.<provider>.<tool>` or
`plugin.<provider>.<tool>`. Built-in names cannot occupy either prefix.
Therefore built-in/MCP, built-in/Plugin, MCP/MCP, Plugin/Plugin, and MCP/Plugin
collision cleanup scenarios are structurally excluded before composition.

## Redaction and remaining gate

The effective CLI inventory contains provider kind/id/status, logical tool
name, permission, and validation only; tests assert executable paths and temp
directories are absent. Late failure returns the bounded profile error only,
so it has no partial inventory/registry or provider diagnostics surface.

The Codex 0.148.0 live release gate is deliberately unresolved. Do not use the
available 0.149.0 binary as a substitute.
