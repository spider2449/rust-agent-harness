# Graphify tool selection guidance

## Problem and scope

Automatic indexing does not imply automatic model tool selection. The managed
query tool describes query syntax and evidence limits but does not explicitly
tell the model when to prefer it over broad source searches.

Update the managed Graphify tool description to request first use for repository
exploration, unfamiliar implementations, callers, dependencies and cross-file
impact, without requiring the user to name Graphify. Exclude known-file edits,
exact text lookups and unrelated tasks. Preserve source verification and the
fallback when the graph is unavailable. No kernel, service contract or policy
changes are required. Tool descriptions guide selection; they cannot guarantee
that a model will call a tool.

## Validation

Add deterministic discovery assertions for selection triggers, exclusions and
fallback. Run the managed Graphify JavaScript tests and diff whitespace checks.
Live model selection is not measured by these tests. Existing installations
need refreshed embedded assets and an MCP restart to receive the description.

All 32 tests passed across artifact, managed MCP, status and graph-view suites.
`git diff --check` passed.

## Local deployment and live selection check

Built CLI and Desktop in release mode. Updated both installed executables in
`C:\Users\spider.tp\AppData\Local\Programs\RAH` and the active Graphify
`artifact.mjs` in `C:\Users\spider.tp\.rah\plugins\graphify`. All three
deployment hashes match their sources. Preserved configuration and backed up
replaced files under
`C:\Users\spider.tp\.rah\config-backups\20261113-141458-graphify-tool-selection`.
No user application was running or stopped. Five Graphify CLI library tests
passed, including embedded installation and configuration preservation.

Ran the installed CLI with the configured local llamacpp model (ornith 1.5
35B Q4_K_M), read-only permissions and this repository as workspace. The user
prompt asked in Chinese to trace model tool calls through permission checks
to implementation, without mentioning Graphify. Session
`b53ed182-8f06-47cc-af85-e2e4d324dd9d` records the tool sequence:
`repo.list`, `mcp.graphify.query_graph`, then `repo.list`. The graph query
succeeded with source evidence (6658 nodes, 18758 edges). This is one observed
automatic selection, not proof of a selection-rate improvement or strict
graph-first behavior. No baseline comparison was run.

Interrupted the test after observing successful graph use; final answer quality
and subsequent source verification were not evaluated. The run did not produce
its end-of-turn summary. The CLI and owned Graphify publisher exited. Start
Desktop normally to use the updated installation. No ADR, contract or permission
changes, commit or push were made.
