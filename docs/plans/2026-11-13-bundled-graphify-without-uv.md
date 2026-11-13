# Bundle Graphify without uv

## Requirements

Add a distribution mode that includes a relocatable Python interpreter, pinned
Graphify dependencies and Node. The target computer needs neither uv nor a
separate Python/Node installation, and installation/querying must work offline.
Keep the existing lightweight uv package supported. Do not change user profiles,
commit, publish or install system-wide dependencies.

## Design

The managed launcher accepts a direct Python backend in addition to uvx. RAH's
Graphify installer discovers graphify-runtime beside the executable (or accepts
an explicit runtime directory/Python), validates the pinned environment and
uses its bundled Node/Python without searching for uvx. Python runs in isolated
mode. Runtime files are data supplied by an explicit user installation, never
selected by model arguments.

A standard-library Python preparation script copies a supplied portable Python
distribution and Node, installs dependencies with pip or copies a prepared
site-packages directory, preserves license files, writes a runtime manifest and
validates relocation. It refuses venvs and unsafe output/source overlap.
package.ps1 and package.sh accept a prepared runtime and emit separate bundled
artifacts. Windows installer and Linux binary installation preserve the runtime
beside the executables. Existing lightweight artifacts must not be deleted.

## Validation

Test backend arguments, bundle discovery, runtime confinement and configuration
selection. Build a real Windows runtime from locally prepared components without
invoking uv, relocate it, hide uv/Python/Node from PATH and run RAH install/query
through the packaged executable offline. Verify repeated installation and
preserved settings. Run normal Graphify/CLI tests, targeted Clippy, formatting,
workspace check and packaging syntax checks. Record platform limits explicitly.

References: Python's Windows embedding documentation and pip's --target install
documentation explain application-managed runtimes/dependencies. A normal venv
must not be copied as a portable interpreter.

## Result and validation

Implemented direct isolated Python execution in managed-mcp.mjs and bundle
selection in the CLI installer. A valid graphify-runtime beside RAH supplies
both programs without PATH discovery or uv. --runtime-dir/--python/--node allow
explicit selection. Bundle manifest version/platform/path containment and the
actual interpreter/package are checked before writing config. Existing uv mode
remains available when there is no bundle or explicit Python selection.

prepare-graphify-runtime.py uses only the Python standard library plus pip when
dependencies need preparing. The real Windows build used an existing portable
CPython 3.14.0 root, Node v24.13.0 and prepared Graphify 0.9.107 site-packages;
no uv command was invoked. The Node license was obtained from its matching
official v24.13.0 release LICENSE, and Python/package licenses were copied.
The bundle contains no uv.exe or uvx.exe. Its expanded runtime is about 320 MiB.

Validation actually executed:

- CLI library/binary tests: 64 and 20 passed. Includes bundle selection without
  PATH, unsupported manifests/escaping paths and preserved/repeated config.
- 32 Graphify Node tests passed, including direct backend arguments and existing
  lifecycle/indexing/status behavior. Three preparation boundary tests passed.
- Real relocated-runtime RAH integration passed (8.84 s): PATH hidden, invalid
  PYTHONHOME/PYTHONPATH and uv cache, BOM Cargo fixture, direct graph build/query
  and Ready status. Model was mock; no inference API or downloads.
- Workspace check, targeted all-target Clippy (-D warnings), formatting and diff
  checks passed. PowerShell parsing and bash -n packaging/install checks passed.
- CLI/Desktop release build succeeded. The actual package.ps1 -SkipBuild
  -GraphifyRuntime invocation produced the ZIP and Inno setup successfully.
- Extracted the actual ZIP into C:\Temp\RAH Graphify no uv package 20261113-110141.
  Its manifest, Python and Node entries exist (7303 archive entries). With PATH
  empty and invalid system Python/uv variables, the release executable installed
  twice without duplicate/config changes, then completed one graph query with
  zero errors. Its summary and generated config prove the Python backend.

The portable ZIP is dist/rah-0.45.38-graphify-bundled-portable.zip (123999571 bytes),
SHA256 A4E4E88919502B78036C5B968D5D670EAE71BB9EB8EEB7B5F4C0CE6F6BB6B58E.
The installer is dist/rah-0.45.38-graphify-bundled-setup.exe (81030028 bytes),
SHA256 AD8B4115E0EFD112262EBC4AE3240A471B55C49886A0E8911F922848B66CEDF4.
The installer was compiled, not run against the user's installation.

Windows payload/runtime behavior is verified. Linux packaging/install syntax is
checked and runtime copying is implemented, but no native Linux interpreter,
binary or end-to-end package was tested on this Windows host. Full workspace
tests were not repeated. The existing lightweight package mode is retained;
the script now replaces only its own staging variant rather than deleting dist.

No active user configuration, installed binaries, kernel, API/protocol, ADR or
crate dependency edge changed. Existing portability work is preserved. These
are local builds, not published releases; no commit/push was created. Use the
complete bundled package, run rah graphify install once, and reopen its Desktop.
Build/use commands are documented in docs/GRAPHIFY_PACKAGING.md.
