# Task 029 plan: profile validation CLI

## Scope

Add one explicit trusted-host command, `rah profile validate <profile-path>`.
It invokes the existing `rah-tools` trusted static profile loader and renders
only that loader's redacted effective inventory. It does not discover,
generate, reload, or modify profiles, and it does not add MCP or Process Plugin
profile composition.

## Design

1. Add a nested `profile validate` clap command that accepts exactly one
   caller-supplied path.
2. Keep profile loading, resource validation, capability construction, and
   all-or-nothing registry ownership in `rah-tools::TrustedStaticProfile`.
3. Render only `EffectiveProfile` fields that are already redacted: profile
   version/ID/source class and each capability's ID, enabled/registered state,
   fixed permission, symbolic resource IDs, and validation state.
4. Propagate the loader's bounded `ProfileError` through the normal CLI error
   path without adding source paths or raw profile values.
5. Add deterministic process-level CLI tests for success, redaction, invalid
   input, and no partial inventory on failure; update README usage.

## Acceptance

- No profile validation logic is duplicated in `rah-cli`.
- The command performs no implicit path discovery or environment-based
  selection.
- Success and failure output contain no resolved host paths, secret values, or
  raw profile content.
- The standard workspace formatting, check, test, Clippy, diff, metadata, and
  representative CLI validation commands pass.
