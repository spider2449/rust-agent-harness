# Optional read-only CBM for RAH

This example uses Node.js and the Windows x64 Codebase Memory MCP v0.12.1
binary. Use the executable whose SHA-256 is
`39f6a789cd858d7428561ac5afaefb9a47b018dcba0ad6fa41ae71a45e0ed656`.
The launcher checks the hash before preparing an index or accepting calls.
See the [pinned CBM release](https://github.com/DeusData/codebase-memory-mcp/releases/tag/v0.12.1).

In PowerShell, set the absolute executable, private cache base, and workspace
paths for your installation. Run the prepare command again to refresh the
index after source changes.

```powershell
$env:RAH_CBM_BINARY = 'C:\rah\cbm\codebase-memory-mcp.exe'
$env:RAH_CBM_HOME = Join-Path $env:USERPROFILE 'rah-cbm'
$rahCbmWorkspace = 'C:\projects\my-project'
node examples/mcp/cbm-readonly.mjs --prepare $rahCbmWorkspace
```

Set the same executable and cache base in [cbm.toml](cbm.toml), along with the
absolute launcher path. Start RAH with `rah --config examples/mcp/cbm.toml
--root C:\projects\my-project`. The example uses the mock model. In the
terminal, try `/call mcp.cbm.search_graph {"query":"MyClass"}`.

The launcher derives an index directory and a separate daemon rendezvous
directory from the canonical workspace path. Preparation explicitly builds
the index in fast mode and disables the CBM UI, auto-indexing, and watching
in that workspace's cache. Prepare while RAH is stopped. RAH starts the
query adapter through `sandbox-process`. The Windows Job Object owns the
adapter and any CBM descendants it starts. A workspace switch restarts the
adapter under the new sandbox root. Calls check the index receipt and CBM's
reported project root. A workspace without a prepared index returns an
explicit error. CBM requires the cache and rendezvous paths to be in a
private directory it accepts. [CBM v0.12.1 configuration](https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/v0.12.1/docs/CONFIGURATION.md)

The configured tools are `search_graph`, `trace_path`, `get_code_snippet`,
and `query_graph`, each assigned `read` by `policy-authority`. The adapter
supplies the workspace's project ID and restricts trace requests to call paths.
CBM's pinned Cypher implementation accepts a read-only subset. Each call
has a bounded reply. Results carry the preparation timestamp and preserve
any resolver strategy, confidence, and source lines CBM returns. Read the
original source to verify candidates such as `unique_name`. Use Graphify
or source inspection when an exact signal-connection location is needed.
[CBM v0.12.1 query syntax](https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/v0.12.1/README.md#supported-cypher-opencypher-read-subset)

The adapter backend is validated on Windows x64. Run the offline adapter
tests with `node --test examples/mcp/cbm-readonly.test.mjs` and the mock-model
workspace-switch test with `cargo test -p rah-cli --test cbm_mcp -- --ignored`.
