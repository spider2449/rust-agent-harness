# Graphify MCP installation

This records the initial manual-index installation. It has since been upgraded
to [managed automatic indexing](2026-11-12-graphify-auto-index.md); the current
mount builds and updates its own workspace graph.

Mount the existing tools-mcp plugin as `graphify` in the user's active RAH
configuration, with an explicit read assignment for its query tool. Preserve
all existing settings and keep an exact backup before replacing the file.

Build an AST-only graph for this repository through RAH's shell/sandbox path.
Store the index in the workspace's ignored `.rah/graphify-out/` directory.
Use a workspace-relative graph argument so another workspace cannot silently
query this repository's graph. Other workspaces need their own graph.

Pin Graphify 0.9.107 and use the existing uv cache offline. Configure only the
MCP instance's environment rather than changing the shared sandbox environment.
Do not install assistant hooks or change the model, permission mode or root.

Validate configuration parsing, MCP startup, a successful query through the
RAH loop and the source path in its result. Use an isolated mock-model profile
for validation, without starting background jobs or unrelated personal plugins.
No RAH process is currently running, so the saved mount takes effect on the
next application launch. Record the backup and validation results below.

## Installed configuration and validation

- Active file: `C:\Users\spider.tp\.rah\rah.toml`.
- Backup: `C:\Users\spider.tp\.rah\config-backups\20261112-150700-graphify\rah.toml.before`.
  The backup hash matched the pre-install file. Reversing only the mount and
  its added tool assignment reproduced the exact original text.
- Mount: `tools-mcp`, instance/server `graphify`, exposing only
  `mcp.graphify.query_graph` with an explicit read assignment. uvx uses the
  pinned cached `graphifyy[mcp]==0.9.107` package offline.
- Graph argument: `.rah/graphify-out/graph.json`, relative to the sandbox's
  workspace root. No global root, model or permission-mode change was made.
- The graph was built through the installed RAH CLI's mock-model request to
  `shell.exec`, supervised by the sandbox service, with fixed program/arguments,
  a 180-second limit and bounded output. Extraction succeeded on 277 classified
  code/configuration files: 6,468 nodes and 18,257 edges. The 44 symbol-free-file
  warning is the same as in the earlier evaluation.
- An isolated read-only RAH profile using the exact saved MCP mount completed
  a mock-model `mcp.graphify.query_graph` call and returned
  `crates/rah-api/src/tools.rs` for `ToolInvoker`.
- The installed RAH CLI accepted the final active configuration (22 plugins)
  with a one-run mock-model override. Its normal model remains `llamacpp`.
  No live-model inference or unrelated background/personal-plugin startup was
  needed for validation.

The graph and installation-check profiles/summaries are ignored under this
workspace's `.rah/`. Source edits are not automatically re-indexed; other
workspaces need their own graph at the same relative path. This installation
does not add graph-first prompting, watcher processes or assistant hooks.
The current graph is a fresh extraction to avoid the known unchanged-build
package-metadata loss. There were no production Rust, ADR or contract changes,
and no commit was created.
