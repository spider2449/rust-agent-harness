# Task 038 — Trusted-Profile Process Plugin Composition

## Scope

Extend the trusted static profile with a closed `process_plugins` section. Each
entry selects a symbolic executable, configured plugin ID, and exact remote
tool schemas with explicit RAH permissions. It intentionally provides no argv,
cwd, environment, inherited-environment, or lifecycle-limit controls.

## Design

Static loading validates only source, strict JSON, IDs, symbolic references,
permissions, and object schemas. Effective validation remains explicit and
constructs MCP and Process Plugin adapters through their hardened constructors.
The CLI host owns a private aggregate containing the fresh registry and all
adapters. A failure shuts down every already staged provider and returns no
registry or inventory.

The Process Plugin prototype uses its fixed `0.1.0` fixture/provider version
and no profile argv. The configured profile ID is passed directly to
`PluginConfig`; child handshake identity must match it. Tool permissions flow
only through `with_expected_tool` and the adapter's external permission policy.

## Acceptance

- static validation never spawns a provider;
- effective validation admits all declared MCP and Process Plugin providers or
  publishes nothing;
- provider tools retain adapter ownership for the aggregate lifetime;
- CLI inventory contains only provider/tool identity, status, and permissions.
