# Offline LSP distribution (Windows x64)

Combined Windows packages and source updates deduplicate identical Node binaries
by SHA-256 into a distribution-owned directory, independent of any plugin:

```text
rah.exe
runtime/bin/node.exe
lsp-runtime/runtime.json       # node = ../runtime/bin/node.exe
graphify-runtime/runtime.json  # node = ../runtime/bin/node.exe
```

Both manifests explicitly select this path and record its checksum; neither
consumer searches the other plugin's directory or PATH. Keep all three runtime
directories together when relocating the package. Different Node binaries remain
separate until prepared with the same version. Standalone inputs and older
manifests still use their own `node.exe`. Preparation inputs are not modified.
Node licenses remain in the runtime bundles. The updater stages and validates
copies before installation; older releases retain their private Node layout so
older executables continue to work. Shared-layout releases retain sharing.

Local shared-runtime validation (2026-11-17): the prepared Graphify runtime was
aligned to LSP's Node v26.13.1, preserving the old Node, license and manifest in
`.rah/runtime-backups/node-unification-20261117-160131`. The staged distribution
contains exactly one `node.exe` (100,089,160 bytes), saving that amount of
uncompressed duplication. Graphify's 36 Node tests pass; an isolated offline
Graphify installation with an empty PATH selects the shared executable. Direct
LSP smoke checks report intentional diagnostics for TS, JS, Python, C++, C and
Rust from the shared distribution. The Rust-through-RAH limitation below remains.
Workspace validation passed: `cargo fmt --check`, `cargo check --workspace`,
`cargo test --workspace --no-fail-fast` (794 passed, 18 ignored),
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
`git diff --check`. The rebuilt portable ZIP passes CRC verification and contains
only `runtime/bin/node.exe`; both manifests record its matching SHA-256.

The optional distribution contains Rust, TypeScript/JavaScript, Python, and
C/C++ language servers plus Node.js. `lsp-diagnostics` selects the fixed bundle
layout beside the running executable when its default servers are enabled.
Explicit commands and disabled servers retain precedence. No kernel, API,
protocol, dependency edge, or tool permission changes are required.

## Prepare and package

Use Python 3.11+ on the build computer. Preparation downloads pinned upstream
archives from `packaging/windows/lsp-runtime.lock.json` and verifies their hashes.
It does not run npm lifecycle scripts or install global programs. TypeScript
5.9.3 is deliberately pinned with its JS `tsserver`; do not substitute a major
version without checking Language Server compatibility. Pyright's optional
macOS-only fsevents dependency is not part of the Windows distribution.

```powershell
python scripts/prepare-lsp-runtime.py --out .rah/lsp-runtime
.\scripts\package.ps1
```

Preparation refuses an existing output. Rebuild to a new directory using the
archive cache without downloads:

```powershell
python scripts/prepare-lsp-runtime.py --offline --out .rah/lsp-runtime-rebuilt
python scripts/prepare-lsp-runtime.py --check .rah/lsp-runtime-rebuilt
```

The default command includes both runtimes: prepare [Graphify](GRAPHIFY_PACKAGING.md)
as well. It discovers `.rah/graphify-no-uv-runtime` (then `.rah/graphify-runtime`)
and `.rah/lsp-runtime`. `-GraphifyRuntime` and `-LspRuntime` override these
locations; `-Lightweight` explicitly omits both and cannot be combined with
runtime paths. A missing runtime fails before build or staging. A build needs Cargo and the
existing desktop build prerequisites; an installer additionally needs Inno
Setup. `-SkipBuild` is only appropriate after building these source changes.
The outputs are `dist/rah-<version>-graphify-bundled-lsp-bundled-portable.zip` and
`dist/rah-<version>-graphify-bundled-lsp-bundled-setup.exe`. Runtime preparation is independent
of RAH compilation, so offline rebuilding also needs the usual Rust build cache.

## Validation and limits

`--check` verifies the file inventory and every checksum, not executable trust
or an OS sandbox. Runtime startup checks the manifest platform, required entry
points and containment. It does not rehash hundreds of files on each launch.
Treat the installed directory as trusted code, like RAH itself.

Run deterministic tests with `python -m unittest discover -s scripts -p
test_prepare_lsp_runtime.py` and `cargo test -p rah-plugin-lsp-diagnostics`.
The explicit live smoke script is separate from normal workspace tests:

```powershell
python scripts/smoke-lsp-runtime.py .rah/lsp-runtime
```

See the packaged [user guide](../packaging/windows/README-lsp-bundled.md) for
activation and project dependency requirements. Official upstream references:
[rust-analyzer installation](https://rust-analyzer.github.io/book/installation.html),
[TypeScript Language Server](https://github.com/typescript-language-server/typescript-language-server),
[Pyright](https://github.com/microsoft/pyright), and
[clangd system headers](https://clangd.llvm.org/guides/system-headers).

## Validation of the 2026-11-17 bundle

- `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace
  --no-fail-fast`, and `cargo clippy --workspace --all-targets --all-features --
  -D warnings` passed after fixing the Rust default-server name mapping.
- Three deterministic Python packaging tests and 20 LSP plugin tests passed.
- Offline cache preparation to a second directory containing spaces passed.
  All six languages (Rust, TS, JS, Python, C++, C) returned deliberate errors
  in direct protocol smoke checks using bundled programs and a PATH without
  host development tools. Rust used a detached syntax-error fixture.
- The rebuilt RAH binary with a mock model appended TS, Python and C++ errors
  after actual file writes. Rust's initial-result issue at that time is
  addressed by the 2026-11-18 follow-up below; direct protocol success alone
  must not be presented as full Rust integration verification.
- No physical network disconnection or OS network-denial test was performed.
  Preparation supports verified cache-only operation; packaging itself is not
  network isolation. No kernel, service contract, protocol, or ADR changed.

## Rust first-result follow-up (2026-11-18)

The client now tracks LSP work-progress tokens and waits for all active work
to finish before settling a publication. Indexing/checking can initially publish
an empty list; 250 ms of silence while work is active is not completion.
The total publication wait, including settling, is bounded by `wait_ms`.
Incomplete work or a disconnected server returns an unknown result.

The opt-in integration test uses the real sandbox-process and LSP plugins,
a temporary dependency-free Cargo project and the prepared rust-analyzer.
It verified E0308 on the first write and no errors after correction:

```powershell
cargo test --offline -p rah-cli --test lsp_runtime -- --ignored --nocapture
```

This needs a local toolchain and uses a 30-second wait. It does not establish
arbitrary-project completeness or detached-file support. Use `cargo check`
when a project compilation result is required. No API, kernel, protocol,
ADR or crate dependency changed.

## Local Codex Rust LSP queries

`scripts/lsp_mcp.py` exposes `rust_lsp` over MCP stdio with diagnostics, hover,
definition, references and symbols operations. It uses Python's standard
library and the prepared rust-analyzer. No additional package installation
is needed. Paths must resolve to Rust files within the configured workspace;
positions are zero-based UTF-16 offsets. It does not edit source files.
Cargo runs offline with build scripts and proc macros disabled; language
server indexing/cache writes are possible. This is not OS isolation.

The checkout's `.codex/config.toml` registers `rah_rust_lsp` using absolute
local paths. Adjust them when moving the checkout or using another machine.
`codex mcp get rah_rust_lsp` verifies that Codex reads the entry. Restart the
MCP connection in the desktop client's MCP settings, then inspect `/mcp`.
An existing conversation's tool catalog may still require a new conversation;
configuration acceptance alone does not prove exposure in that conversation.
See the [official MCP setup guide](https://learn.chatgpt.com/docs/extend/mcp).

```powershell
python -B -m unittest discover -s scripts -p test_lsp_mcp.py -v
$env:RAH_TEST_LSP = (Resolve-Path '.rah/lsp-runtime/rust-analyzer.exe').Path
python -B -m unittest discover -s scripts -p test_lsp_mcp.py -v
```

The opt-in test verifies real diagnostics, correction, navigation queries and
normal shutdown. Requests are serialized and bounded; cancellation or client
disconnect interrupts server waits and closes the process. Do not treat an
empty publication as proof of compilation.

Navigation follow-up (2026-11-18): navigation no longer waits for diagnostic
publications or sends save notifications. Unchanged files keep their current
document version; changed files are synchronized before the request. Initial
semantic queries wait for rust-analyzer's
[server readiness notification](https://rust-analyzer.github.io/book/contributing/lsp-extensions.html#server-status),
avoiding empty hover results while project metadata is loading. Requests and
event draining retain their deadlines even under continuous notifications.
Diagnostics retain their separate publication/progress wait.

All nine bridge tests passed with `RAH_TEST_LSP` set, including a fresh MCP
stdio connection to this repository: eight consecutive symbols/hover/definition/
references queries passed, with about 7.2 seconds for initial readiness and
0–16 ms for subsequent calls in this run. The tests also cover no diagnostic
publications during navigation, document changes, cancellation and deadlines.
This closes the local MCP navigation-timeout entry in `KNOWN_ISSUES.md`.
Restart the existing `rah_rust_lsp` MCP connection to load the changed script;
testing a fresh subprocess does not update a running desktop connection.

## Update an installed copy

`scripts/update.ps1` now requires and installs both LSP and Graphify runtimes.
After preparing LSP as above and [Graphify](GRAPHIFY_PACKAGING.md), run:

```powershell
.\scripts\update.ps1 -Source build
```

Default build inputs are `.rah/lsp-runtime` and `.rah/graphify-no-uv-runtime`
(falling back to `.rah/graphify-runtime`). Other prepared locations can be used:

```powershell
.\scripts\update.ps1 -Source build `
  -LspRuntime 'D:\offline\lsp-runtime' `
  -GraphifyRuntime 'D:\offline\graphify-runtime'
```

The updater validates both inputs before closing RAH, replaces runtime
directories instead of merging stale files, enables an LSP mount if absent,
then runs the installed `rah graphify install --offline`. Existing LSP
commands/settings, model, permissions and comments are retained. Configuration
changes have backups under the selected data directory's `config-backups`.
Sessions are untouched. A new profile leaves model selection to Desktop setup.
The Graphify bundle needs Python 3.11+ for updater TOML handling.

Use `-DataDir DIR` and optionally `-Config FILE` for another profile. The default
respects `RAH_HOME`, otherwise `%USERPROFILE%/.rah`. `-InstallDir DIR
-NoShortcuts` supports an isolated install without changing Start Menu or PATH.
The update does not claim whole-install rollback: errors after copying identify
which activation failed, and must be resolved before rerunning the command.

Release mode selects only `rah-VERSION-graphify-bundled-lsp-bundled-portable.zip`
and validates it using the release's `SHA256SUMS.txt`. It refuses lightweight or
single-runtime releases. Auto mode falls back to a checkout build when that
combined asset is absent. This change does not publish a combined release;
the existing release workflow still needs a separate packaging task to prepare
and publish both runtimes. Even a same-version update installs/repairs both
runtimes instead of exiting early.

Updater validation: six deterministic tests in `scripts/test_update.py` pass;
an isolated build install with fresh profile successfully installed both
runtimes and activated Graphify offline without modifying the host profile.
A same-version reinstall preserved the mock model, read-only permission,
comments and a data sentinel; it removed stale files from both runtimes and
left exactly one mount for each integration. A transient Windows directory
rename failure during the first reinstall restored the old runtime; bounded
rename retries were added and the subsequent reinstall passed.
