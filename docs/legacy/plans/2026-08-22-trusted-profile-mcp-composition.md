# Task 035: Trusted-profile MCP composition

## Scope

Compose hardened local stdio MCP providers from explicit trusted static profile
entries.  The profile remains host-only configuration; all discovered tools enter
the ordinary `ToolRegistry` only after exact admission succeeds.

## Design

1. Move the trusted-profile loader into a narrow composition crate above
   `rah-tools` and `rah-tools-mcp`, avoiding their otherwise unavoidable Cargo
   dependency cycle.
2. Retain non-spawning `profile validate` for source/schema/static validation.
   Add `profile validate-effective` for explicit provider launch/discovery.
3. Parse a closed `mcp_providers` section with symbolic executable resources,
   exact tools, schemas, and explicit permissions.  Do not admit argv, cwd,
   environment, or transport-limit controls.
4. Build built-ins and MCP adapters privately into a fresh registry.  Keep live
   adapters owned by the effective profile so their proxy tools cannot outlive
   their connection.
5. Render only redacted provider/tool identities and validation state.

## Validation

Run focused profile/MCP/CLI tests, then workspace fmt/check/test/clippy, diff
checks, Cargo metadata, and representative static/effective CLI cases.

## Non-goals

No Process Plugin composition, generic subprocess schema, HTTP MCP, restart,
hot reload, provider manager, raw argv/cwd/environment profile fields, or
model-facing profile APIs.
