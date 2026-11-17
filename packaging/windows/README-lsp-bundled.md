# RAH with offline language servers — Windows x64

This package includes rust-analyzer (Rust), TypeScript Language Server and
TypeScript (TS/JS), Pyright (Python), clangd (C/C++), and portable Node.js.
There is no npm, pip, or language-server download during startup.

Keep the entire `lsp-runtime` directory beside `rah.exe` and `rah-desktop.exe`.
The executables discover its current location, including after moving the ZIP.
Enable diagnostics when starting:

```powershell
.\rah.exe web --with lsp
# Or the native desktop:
.\rah-desktop.exe --with lsp
```

For subsequent launches, add `"lsp"` to the existing top-level `features` array
in `rah.toml` (`rah config edit`). Preserve other enabled features. With a fresh
profile you can also use the normal configuration setup with `--with lsp`.
Diagnostics appear after RAH writes a file; this is not editor completion.
Explicit `servers.<name>.command` settings override the bundled defaults;
`defaults = false` disables automatic bundle discovery. Servers still start
through the configured sandbox and have its permissions.

The bundle contains analysis servers, not complete project development environments:

- Rust: install/provision a compatible Rust toolchain, `rust-src`, and project
  crates before going offline. A rustup launcher alone is not a toolchain.
- TS/JS: bundle supplies TypeScript itself; project `node_modules` and type
  declarations must already be available. Automatic type-package acquisition
  is disabled for the bundled default server.
- Python: Pyright runs on bundled Node without Python. Project interpreters,
  imported packages, and their type stubs are still needed for full analysis.
- C/C++: clangd includes its builtin headers. Provide the project's SDK,
  standard-library headers, third-party headers, and `compile_commands.json`
  (or suitable `compile_flags.txt`). No C++ compiler or Windows SDK is bundled.

Packaging does not enforce network isolation. Project build scripts, rustup,
and your model provider can still use the network unless separately restricted.
An offline Rust workspace can set Cargo's `[net] offline = true`; prepare its
toolchain and dependencies first. RAH does not automatically change these settings.
Initial indexing may exceed the default 4-second diagnostic wait; increase
`lsp-diagnostics`'s `wait_ms` for large projects. Missing dependencies are
reported as diagnostics; a timeout is not a successful check.

Versions and SHA-256 file inventory are in `lsp-runtime/runtime.json`.
Upstream licenses remain in the runtime, including `licenses`, package license
files, and clangd's distribution license.
