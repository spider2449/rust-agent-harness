# Offline LSP distribution (Windows x64)

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
  after actual file writes. Rust through RAH has an unresolved initial-result
  issue described in [known issues](KNOWN_ISSUES.md); direct protocol success
  must not be presented as full Rust integration verification.
- No physical network disconnection or OS network-denial test was performed.
  Preparation supports verified cache-only operation; packaging itself is not
  network isolation. No kernel, service contract, protocol, or ADR changed.

Next task: resolve Rust initial diagnostics before relying on this feature as
a Rust verification gate.

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
