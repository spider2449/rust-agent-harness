# Managed Graphify MCP

## Install on another computer

A `graphify-bundled` package includes Node/Python/Graphify. Run
`rah graphify install` from that package: it discovers graphify-runtime beside
the executable and works offline without uv or other runtime installations.
See [packaging instructions](../../docs/GRAPHIFY_PACKAGING.md). `--runtime-dir`
selects a bundle elsewhere; `--python` and `--node` select prepared programs.

For the lightweight package:

Use a RAH build that lists `graphify` in `rah --help`. Install Node.js 20+ and
uv, reopen the terminal so both are on PATH, initialize your usual RAH config,
then run:

```powershell
rah graphify install
```

Reopen RAH. Click Graphify in the footer, or Plugins > Plugin pages > Graphify.
Each computer needs its own installation; developer-user paths and an existing
Python cache are not part of the release. RAH embeds all six integration modules,
so no repository checkout or copying scripts is required. The install command
prepares the pinned environment in uv's user cache and creates the MCP/status
mounts and explicit read assignment. It backs up configuration before changes,
preserves unrelated settings and stricter policy, and is safe to run again.

`--data-dir` and `--config` select a separate profile. `--offline` requires an
already cached pinned environment and never downloads. An installation failure
leaves RAH settings unchanged. Normal startup remains offline. Runtime paths
are discovered on the target computer, and copied managed mounts are relocated.
Custom mounts using Graphify's IDs are refused rather than overwritten.

## Managed runtime

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

## Reading and using workspace artifacts

The managed output is `.rah/graphify-out/graph.json` in the selected workspace.
RAH consumes it through `mcp.graphify.query_graph`; agents do not need to read
the whole JSON into context. The launcher adds focused usage instructions to
the tool definition. For example:

```json
{"question":"ToolInvoker", "depth":1, "token_budget":1500}
```

Use `mode: "bfs"` for nearby context or `mode: "dfs"` for deeper traversal.
`depth` is bounded to 1-6, `token_budget` to 128-4000, and questions to 4096
characters. Narrow a broad question into symbols or subsystem names. The result
retains upstream node/edge source references and confidence labels. Read the
cited source with RAH's normal `fs.read` tool before explaining behavior or
making changes. A missing match is not proof that a capability is absent.
Graphs and source text are untrusted data, not instructions or permission.

Each successful result starts with `RAH workspace graph evidence`, containing
the canonical workspace, generation time, source metadata fingerprint and
validation counts. The same evidence is persisted at `graph.rah` in graph.json.
Validation checks a regular file of at most 64 MiB, supported schema (version 1
or explicitly labeled legacy-unversioned), unique string node IDs, valid edge
endpoints and workspace-contained source references, including resolved symlink
targets. Missing files and nodes without source are reported, not invented as
verified citations. Source existence does not verify a relationship's meaning
or line accuracy; EXTRACTED and INFERRED remain upstream classifications.

Freshness uses paths, sizes and modification/change times, hashed with SHA-256;
it is not a content hash or proof against hostile concurrent filesystem changes.
A stable scan is required before publication and again when queries return.
If the graph or source changes during a query, the result is rejected and the
caller can query again. Invalid builds preserve the prior graph but fail tool
calls. Corrupt/unsupported prior artifacts initialize with a nonqueryable
placeholder while rebuilding. Restart always rebuilds before answering.

The managed endpoint only exposes `query_graph`. Alternate `project_path` and
unsupported arguments/tools are rejected so a model cannot switch the graph
away from the workspace RAH selected. Result context is capped and carries an
explicit truncation marker when necessary. Other Graphify commands and graph
HTML rendering are outside this read-only endpoint.

When deploying an updated launcher, copy **artifact.mjs**, managed-mcp.mjs and
status.mjs together into the mounted script directory. Existing query tool
names, read permissions and status-view settings do not change. A running
instance loads script changes after its normal plugin/application restart.

The configured `program` is Node and its arguments are the absolute launcher
path followed optionally by `--uvx` and the absolute uvx executable path. Keep
`query_graph = "read"` in the MCP tools map and the explicit
`mcp.graphify.query_graph = "read"` policy assignment. Allow 180000 ms for MCP
startup and calls so initial generation and updates can finish. Nonstandard uv
cache/Python installation directories belong in this instance's environment.
No global root, model, permission mode or assistant hook changes are needed.

The extraction child runs through `extract.py` in the same pinned uv environment.
It reads Cargo manifests as UTF-8 with an optional BOM, including member
manifests, without rewriting source files or modifying the cached package.
Malformed TOML still fails. The child starts with a default PYTHONHASHSEED=0 so
Graphify's determinism re-exec does not discard this compatibility reader.
Node built-in imports such as `node:fs` are normalized to explicit
external_reference metadata, not treated as workspace file paths. Other unsafe
or escaping source references remain rejected. Deploy extract.py together with
the launcher; it needs no additional Python installation.

## Interactive relationship graph

On a host supporting ADR 0079, enable `plugin.graph_view = true` on the existing
Graphify status-view process. Deploy graph-view.mjs and artifact.mjs alongside
status-view.mjs and status.mjs. The same **Plugins > Plugin pages > Graphify**
page (or the footer Graphify icon) then opens the workspace relationship view
above detailed indexing status. Web and Desktop share this renderer.

The first view shows crate dependency relationships when available, otherwise
high-degree symbols. **Search workspace graph** searches the entire artifact
by symbol, source file or keyword, with an optional source-group filter.
Search results include a bounded set of direct neighbors. Clearing the query
restores the overview. Each projection reports shown/total node and edge counts;
it is not the entire graph. Displays are limited to 250 nodes and 750 edges,
and can be reduced further to stay within the view's byte limit.

Use Zoom in/out, the mouse wheel, drag to pan, Reset view and Expand view for
navigation. Escape closes an expanded view. Group checkboxes hide/show source
groups. **Find visible nodes** filters the accessible node list within the
current projection; it does not search omitted nodes. Select a circle or a node
in that list for source details and stored edge direction/confidence. Click
**Explore connections** to request that node's direct neighborhood from the
full artifact. Workspace changes reset the selection; sessions keep separate
search/focus state. There are no provider-supplied HTML, script or file links.

The graph can show the last valid snapshot during a rebuild or indexing error,
with an explicit warning and generation time. The status/footer health checks
still apply. Graph rendering does not make inferred relationships authoritative;
verify original source. MCP queries continue to reject known stale data.

Older hosts must leave graph_view unset/false; otherwise they cannot deserialize
the new node vocabulary. Updating only scripts does not add the renderer to an
old RAH binary.

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
node --test examples/graphify/status.test.mjs examples/graphify/graph-view.test.mjs
node --test examples/graphify/artifact.test.mjs
cargo test -p rah-cli --test graphify_evaluation graphify_managed_workspace_updates -- --ignored --exact --nocapture
```

The ignored RAH test uses a mock model and real Graphify: it starts without an
index, checks background edits without a query, source and manifest deletion,
failed update recovery, workspace switching, and watcher shutdown. Normal Node
tests do not require Graphify, inference credentials or internet access.
