# Portable Graphify installation

## Problem and requirements

The distributed RAH binaries include the graph renderer but Graphify itself
was mounted only through one user's absolute paths and local scripts/cache.
A different computer therefore has no Graphify tools, page or footer entry.

Ship the managed integration scripts inside the RAH binary and provide
`rah graphify install` to discover local Node/uvx, prepare the pinned Python
environment, install versioned scripts under the selected data home, and add
the MCP/status mounts plus explicit read policy to that computer's config.
Respect --data-dir/--config, preserve unrelated settings and explicit policy,
back up config, reject conflicting plugin IDs and support repeat installation.
Missing prerequisites and an empty uv cache must have actionable errors. No
dependency download occurs just by starting RAH; install is an explicit action.

## Validation

Use deterministic config/asset tests for fresh profiles, repeat installs,
existing mounts, explicit policies, custom data/config paths and conflicts.
Exercise the CLI with an isolated profile and the cached package offline, then
boot real RAH and verify graph tools/page discovery without developer paths.
Keep normal tests offline. Run relevant Rust/Node tests, formatting, Clippy and
diff checks. Document the command in shipped help/configuration/README; embedded
assets must work from an installed executable with no repository present.
Do not commit or change the user's active configuration in this task.

## Implementation and evidence

Added `rah graphify install [--offline]`. The binary embeds all six managed
modules and installs a version/content-keyed copy under the selected data home.
Node and uvx are discovered locally. The explicit installer uses the existing
supervised ProcessEnv with fixed arguments, a timeout and bounded output to
check Node and prepare the pinned Python environment. Normal startup continues
to use Graphify offline and never implicitly downloads packages.

Configuration edits preserve comments, unrelated mounts, model/workspace/mode
settings and exact/prefix policy assignments. Conflicting custom Graphify IDs
are refused; copied managed mounts are relocated and stale machine-specific uv
and PYTHONHOME settings removed. A config backup is written before atomic
replacement, concurrent changes are refused, and repeating installation does
not add mounts or backups when settings already match. Modified managed assets
are refused rather than overwritten. Runtime lifecycle/tool authorization still
belong to the existing sandbox, MCP and process-view plugins.

Validation executed:

- CLI library: 63 tests passed, including four portable installer tests.
- Binary suite: 20 tests passed, including missing-runtime guidance with the
  original configuration unchanged.
- Ignored cached Graphify portable integration passed (4.94 s): copied the
  executable outside the checkout, used separate data/config paths with spaces,
  rejected an empty offline cache without changing settings, installed embedded
  assets, booted RAH, queried the workspace and rendered the graph/status view.
- Targeted all-target Clippy with -D warnings, workspace check, formatting and
  diff checks passed. No full workspace suite or live inference was run.
- CLI/Desktop release builds succeeded. The packaged release executable was
  independently installed into an isolated temporary profile and completed a
  mock-model graph query: one call, zero errors, two graph nodes, lib.rs source.
  The failed initial manual query was Windows PowerShell argument quoting; the
  fixed-argument-vector verification succeeded and its summary was checked.

## Transferable builds

Generated a local preview pair without overwriting the existing standard
0.45.38 artifacts:

- dist/rah-0.45.38-graphify-preview-setup.exe
- dist/rah-0.45.38-graphify-preview-portable.zip

The portable archive includes both binaries, the Web launcher/icon, current
README and GRAPHIFY-SETUP.txt. The installer was compiled with the existing Inno
Setup tool. It was built, not executed on the user's machine. Node.js and uv
remain prerequisites on each target machine. After updating RAH, run
`rah graphify install`, then reopen the matching Desktop build to see Graphify.
An old release from GitHub does not gain this command from a local commit.

The Windows setup SHA256 is
FFA8ECF48DB058BC4620164F2EAF1BE8A7F707F133B6D07B433EA8D825F2E8B1;
the portable zip SHA256 is
EE2C3DE34069257A11A51A5A5F4B79B63B8560F0283CF895E98CD256CD4C5B85.
These are local preview builds, not published/tagged releases. Existing user
configuration and installed binaries were left unchanged. No kernel, API,
protocol, ADR or dependency edge changed. No commit or push was created.
