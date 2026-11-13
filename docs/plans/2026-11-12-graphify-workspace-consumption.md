# Verified consumption of workspace Graphify artifacts

## Requirements

Use the existing managed MCP plugin to discover and consume the active
workspace's graph. Validate bounded JSON, supported schema, unique node IDs,
edge endpoints and workspace-contained source references before publication.
Record generation provenance and freshness evidence in the published artifact.
Keep legacy schema-less Graphify output readable but identify it explicitly.

Expose bounded graph queries with model-visible instructions to use graph
results for navigation and verify source before making behavioral claims or
edits. Return workspace/provenance/validation evidence alongside results.
Reject model-supplied alternate project paths and unsupported tools. Never
serve placeholders, invalid builds or known stale results. Preserve the last
good graph and existing status UI and scope ownership.

## Implementation and validation

Extend examples/graphify rather than the kernel, API or generic MCP plugin.
Add deterministic artifact and protocol tests, and extend the native RAH
integration test to prove discovery, query evidence and workspace isolation.
Run Node tests, the cached real Graphify integration with a mock model,
formatting, targeted Rust checks and diff checks. Document query usage,
validation meaning, limitations and deployment. No commit or host configuration
change is needed for repository implementation.

## Result

Implemented artifact.mjs and integrated it with managed-mcp.mjs. Publication
validates the graph and persists graph.rah evidence. MCP discovery supplies
bounded query guidance, removes alternate project selection, and only exposes
the read-only query_graph endpoint. Requests and responses carry workspace
freshness checks; a changed snapshot produces a tool error rather than stale
context. Successful results include validation/provenance and source-reading
guidance, with a bounded graph excerpt. Existing status UI remains unchanged.

The native RAH test verifies tool discovery, provenance, a graph query followed
by policy-checked fs.read, cross-workspace argument rejection, source updates,
failed rebuild recovery, source deletion, workspace switching and lifecycle.
No kernel, production Rust, service contract, protocol, ADR or dependency edge
changed. Model compliance with source-reading guidance is not enforced by a
new policy; tests exercise the supported navigation/read workflow with a mock.

Validation executed:

- 26 deterministic Node tests passed across artifact, managed indexing and
  status suites, including bounded output and stale-result rejection.
- The cached real Graphify managed-workspace RAH integration passed (mock
  model, no inference API).
- Two normal graphify_evaluation tests passed; three external/live tests are
  ignored by default. The managed integration was explicitly run separately.
- Targeted Clippy (`cargo clippy -p rah-cli --test graphify_evaluation -- -D warnings`),
  formatting and diff checks passed. Full workspace/release tests were not run.
- The actual workspace artifact passed validation: 6565 nodes, 18503 edges,
  283 referenced source files, zero missing source files and 1123 nodes without
  source. This older artifact is explicitly legacy-unversioned, not a claim
  that its contents are current; the managed instance rebuilds before queries.

The installed launcher and its new artifact module were backed up/deployed
under C:\Users\spider.tp\.rah\plugins\graphify, with source/deployment hashes
checked. The original launcher backup is in
C:\Users\spider.tp\.rah\config-backups\20261112-171137-graphify-consumption.
status.mjs already matches the repository. rah.toml was unchanged. The running
Desktop (PID 14392 at deployment) was preserved; it loads the new scripts on
its next normal plugin/application restart. No commit was created.

Remaining limits: source metadata fingerprints are not content hashes; graph
relationships and line references still require source inspection. This change
consumes the managed workspace graph, not arbitrary external project graphs or
HTML. Suggested next interactive check: after normal restart, ask RAH to locate
a symbol using Graphify and verify its cited source.
