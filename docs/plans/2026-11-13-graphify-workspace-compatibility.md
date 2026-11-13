# Graphify workspace compatibility repair

## Evidence and scope

image-viewer's Cargo.toml starts with EF BB BF. Graphify 0.9.107 loads Cargo
manifests through tomllib.load on binary files and fails with Invalid statement
at line 1, column 1. Preserve workspace files and add a child-local compatibility
reader for UTF-8 BOM manifests, including workspace members. Real malformed
TOML must still fail rather than silently dropping dependency metadata.

The current host status also reports workspace source-reference rejection for
cloudcode. Reproduce that extraction and inspect the offending references
before changing validation. Keep workspace containment and explicit provenance.

Add regression tests, validate real affected workspaces without inference or
network, and back up/deploy the changed integration scripts. Preserve the
running application and existing worktree changes. Do not commit.

## Repair and validation

The supervised extraction child now runs extract.py in the same pinned uv
environment. It adapts both cargo_introspect and package-manifest ingestion to
an optional UTF-8 BOM, preserving pkg_ as well as crate: nodes. Malformed TOML
and invalid encodings still fail. Child-local PYTHONHASHSEED=0 prevents upstream
re-execution from dropping the compatibility reader. No original manifest,
global Python installation or cached Graphify package was changed.

cloudcode's rejected reference was node:fs, a Node built-in module identifier,
not an absolute/escaping file. Validation now recognizes that narrow namespace,
moves it to explicit external_reference metadata and reports a separate count.
Actual path traversal, absolute paths, unsafe URI schemes and symlink escapes
still fail. The graph detail displays external references without implying a
workspace source file exists.

Validation actually executed:

- 31 Node tests passed, including built-in-module normalization, unsafe scheme
  rejection, stable normalized reads and child-only environment overrides.
- Two Python compatibility tests passed in the cached pinned environment:
  ordinary/BOM/member manifests, package/dependency nodes and invalid input.
- The native RAH managed integration passed in 20.77 s with a BOM root manifest,
  package retention after updates, malformed-TOML rejection/recovery, graph
  render/actions, source reads, workspace switching and disposal (mock model).
- Two normal Rust grader tests passed; three external/live tests remain ignored
  by default. Formatting and diff checks passed; no full workspace suite was run.
- The exact installed launcher rebuilt and queried image-viewer: 873 nodes,
  2271 edges, 26 referenced source files, zero missing files. query_graph for
  scan_folder returned src/app.rs. Both pkg_image_viewer and crate:image-viewer
  are present. Cargo.toml still starts with EF BB BF and has no Git diff.
- The installed launcher also rebuilt and queried cloudcode: 2841 nodes, 8305
  edges, 438 source references and one explicit built-in reference. Querying
  startFromBinary returned scripts/bin-runtime.ts. The existing npm specifier
  @modelcontextprotocol/sdk is reported as one unresolved source reference; it
  does not fail the graph or count as a verified workspace file.

## Deployment and remaining scope

Updated managed-mcp.mjs, artifact.mjs, graph-view.mjs and the new extract.py under
C:\Users\spider.tp\.rah\plugins\graphify; deployed hashes match sources.
Backup: C:\Users\spider.tp\.rah\config-backups\20261113-093327-graphify-workspace-compatibility.
rah.toml and installed binaries were unchanged. No RAH/Desktop process remained
running when deployment was checked; no user process was stopped. One-shot
validation launchers were shut down normally, so their workspace status correctly
reports stopped rather than pretending a watcher is still active. Opening the
workspace in RAH starts the real scoped watcher and uses the repaired scripts.

Changes are integration-local: no kernel, API/protocol/ADR, permissions or crate
dependency changes. Prior uncommitted work was preserved and no commit was made.
The pinned upstream BOM defect is documented in KNOWN_ISSUES.md. Suggested next
check: open image-viewer normally and confirm Graphify reaches Ready and its
relationship view appears.
