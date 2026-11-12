# Managed Graphify MCP

`managed-mcp.mjs` runs as the existing `tools-mcp` instance's program. It needs
Node.js and the cached `graphifyy[mcp]==0.9.107` package. RAH's sandbox supervises
the launcher and its complete process tree; there is no detached watcher or
additional Python evaluation runner. The installed Windows configuration uses
an absolute launcher path outside the workspace, so workspace switching works.

The launcher uses the sandbox working directory as its workspace. MCP initializes
against a prior graph or a marked placeholder while a fresh, local AST-only
build runs in the background. The UI can open before indexing finishes; model
tool calls still wait for a successfully refreshed graph. It polls for changes
every 2.5 seconds and refreshes before forwarding tool calls. Adds, edits, renames,
deletions and Cargo manifest changes are included. Cargo introspection is used
only when the workspace has a root Cargo.toml. The watcher excludes `.rah`,
dependency and build output. Git workspaces respect Git's tracked/nonignored
untracked file list; other folders use a bounded filesystem traversal. Source
symlinks are not followed by change detection.

Each extraction uses a fresh staging directory, avoiding Graphify 0.9.107's
incremental package-metadata loss. Publication replaces only
`.rah/graphify-out/graph.json` atomically after validation and a second source
scan. Requests wait for a fresh graph; an update failure returns a tool error
instead of a known stale answer. The last good graph is preserved for recovery.
Query output still needs source verification: this does not fix AST resolution
limits or make model answers authoritative.

The configured `program` is Node and its arguments are the absolute launcher
path followed optionally by `--uvx` and the absolute uvx executable path. Keep
`query_graph = "read"` in the MCP tools map and the explicit
`mcp.graphify.query_graph = "read"` policy assignment. Allow 180000 ms for MCP
startup and calls so initial generation and updates can finish. Nonstandard uv
cache/Python installation directories belong in this instance's environment.
No global root, model, permission mode or assistant hook changes are needed.

## Live status page

The companion `status-view.mjs` is a read-only RAH process plugin providing the
existing `ui-views` service. Install it and `status.mjs` next to the launcher,
and set `RAH_GRAPHIFY_STATUS_DIR` in the MCP instance's environment. Give the
view's `initialize.config.status_dir` the same absolute directory. It has no
tools, host-service requirements or actions, and does not change tool policy.

Open **Plugins > Plugin pages > Graphify** in RAH. The page refreshes on
`ui/changed` notifications and shows state, stage, source files processed,
nodes/edges, elapsed or build time, last successful indexing time and last error.
States include Waiting, Building, Ready, Update failed, Stopped and Not responding.
On a host implementing ADR 0078, set the view plugin's `plugin.status_bar = true`
to show Graphify in the bottom status bar. The graph icon pulses while building
and includes file progress when available; green means ready, red means failed,
and a warning indicates missing status or a lost connection. Hover for details
and click to open the status page. Omit the flag on older hosts.
Finishing AST parsing is shown as Resolving relationships until publication
really succeeds. A failed update keeps the prior successful time/counts.

Status documents are published atomically in the workspace and in the configured
runtime directory. Runtime filenames include the parent RAH process id so two
RAH instances do not share a workspace status. Each document is limited to
16 KiB. Heartbeats are written every five seconds; dead publishers and heartbeats
older than fifteen seconds are not displayed as actively watching. The view
process polls once per second and stops with its owning plugin.

The implementation is validated on Windows with Node 24. RAH's existing sandbox
owns lifecycle on all supported platforms; Windows subprocess cancellation also
uses taskkill with fixed arguments to terminate descendants. The launcher is not
an OS sandbox. Full rebuilds favor correctness over update throughput and can
take several seconds on a large repository. Editing continuously can postpone
publication until a scan is stable.

Validation:

```powershell
node --test examples/graphify/managed-mcp.test.mjs
node --test examples/graphify/status.test.mjs
cargo test -p rah-cli --test graphify_evaluation graphify_managed_workspace_updates -- --ignored --exact --nocapture
```

The ignored RAH test uses a mock model and real Graphify: it starts without an
index, checks background edits without a query, source and manifest deletion,
failed update recovery, workspace switching, and watcher shutdown. Normal Node
tests do not require Graphify, inference credentials or internet access.
