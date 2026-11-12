# Managed Graphify workspace indexing

The indexing behavior below is now complemented by
[visible indexing status](2026-11-12-graphify-status.md). The managed MCP instance
initializes before its background build finishes so that status is visible;
model tool calls still wait for successful index refresh.

## Scope

Make the installed Graphify MCP instance build a workspace graph automatically
before initialization and refresh it after source changes. Keep using tools-mcp
and sandbox-process: a Node launcher and its descendants belong to the existing
sandbox scope, so workspace changes restart the managed process with the new
root, and stopping RAH ends the watcher/build/server tree. No kernel or service
contract change, dedicated Python runner, detached daemon or model API call.

## Design

- A launcher builds AST-only into a fresh staging directory on startup, then
  atomically publishes graph.json before starting Graphify's MCP server.
- Poll workspace source metadata and coalesce changes. Git workspaces use
  tracked/nonignored untracked paths; other folders use a bounded filesystem
  traversal. Exclude RAH/index/build/dependency output to prevent self-triggering.
- Serialize rebuilds and check for edits during extraction. Use fresh staging
  builds to avoid the known incremental package-node loss and handle deletions.
- Gate tool requests on a fresh index, return an error after failed updates,
  preserve the last good graph, and never silently answer from known stale data.
- Bound subprocess time/output, protocol buffering and requests. All diagnostics
  go to stderr; stdout remains MCP-only. Cancellation removes queued requests.
- Install the launcher under the user's plugin data directory, back up and
  update only the existing graphify mount, and preserve explicit read policy.

## Validation

Use deterministic Node tests for generation, coalescing, add/delete/rename,
edits during extraction, publication failures and cancellation. Add an ignored
native RAH test with real Graphify for empty-workspace startup, automatic update,
deletion and shutdown. Validate the installed configuration and managed mount.
Do not run a live model benchmark or create a commit.

## Completed implementation and installation

The launcher is in `examples/graphify/managed-mcp.mjs`, with usage and limits in
its README and deterministic tests alongside it. The deployed identical copy is
`C:\Users\spider.tp\.rah\plugins\graphify\managed-mcp.mjs`. The active configuration
`C:\Users\spider.tp\.rah\rah.toml` still mounts the same tools-mcp instance and
read-only query tool, but now starts Node with the launcher and the absolute uvx
path. Startup and call timeouts are 180000 ms. The index is checked every 2500 ms
while the instance is running and refreshed before forwarding tool calls.

The configuration backup is
`C:\Users\spider.tp\.rah\config-backups\20261112-153746-graphify-auto\rah.toml.before`.
Its hash matched the original file. Reversing only the changed Graphify config
line reproduced the original text exactly; model, root, features, permission
mode, other plugins and policy assignments were preserved. The final installed
configuration passed the RAH CLI check with a one-run mock-model override and
still contains 22 plugins.

An isolated profile using the exact deployed launcher and saved MCP settings
started against this repo, automatically rebuilt the graph to 6492 nodes, and
completed a real MCP query through the installed RAH binary with a mock model.
The summary at `.rah/graphify-auto-install/query-summary.json` records completion,
one tool call and zero tool errors. The index and profiles stay ignored under
`.rah/`. No user application process was running during installation; the next
RAH launch loads the managed mount.

## Validation

- `node --test examples/graphify/managed-mcp.test.mjs`: 11 passed. Covers initial
  generation, unchanged scans, add/edit/delete/rename, concurrent request
  coalescing, edits during a build, failure recovery, missing/corrupt index
  recovery, cancellation, watcher ownership, ignored output, and subprocess
  time/output bounds.
- `cargo test -p rah-cli --test graphify_evaluation graphify_managed_workspace_updates -- --ignored --exact --nocapture`:
  passed in 20.36 s with real Graphify and a mock model. Covers startup without
  an index, background update without a query, preserved Cargo package metadata,
  tool errors rather than stale responses after an invalid manifest, recovery,
  additions, source/manifest deletion, workspace switching, old watcher disposal
  and shutdown of the new workspace watcher.
- The two normal offline grader/usage tests passed; external/live tests remain
  ignored by default. No live-model inference was run for this task.
- `cargo fmt --check`, targeted Clippy with `-D warnings`, Node syntax checking,
  and `git diff --check` passed. No workspace release suite was required for the
  external launcher/test change.

No production Rust, kernel, ADR, dependency edge or service contract changed.
All descendants remain in the existing RAH sandbox's process tree. On Windows
the launcher's bounded subprocess cancellation additionally terminates the
child tree with fixed taskkill arguments. Watchers are not standalone services.

## Practical limits

Updates are automatic while RAH and this plugin are running. Each detected
change triggers a full AST-only rebuild in a fresh directory, so publication
follows extraction completion rather than happening immediately at the polling
interval. A stable scan is required before publication; continuous edits can
delay it. Git's ignore rules and the launcher's build/dependency exclusions
apply to change detection. The implementation is validated on Windows/Node 24.
The upstream incremental metadata bug remains, but the managed path avoids it;
source verification is still necessary for graph-inferred relationships.

The earlier manual-installation report is historical. No commit was created.
