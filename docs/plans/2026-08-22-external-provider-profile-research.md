# Task 031 plan: external provider profile integration research

## Scope

Perform a documentation-only audit of the current `rah-tools-mcp` and
`rah-tools-plugin` constructors, lifecycle handling, permission boundary, and
their fit with Task 028-030's trusted static profile loader. Do not modify Rust
code, manifests, public APIs, or dependencies.

## Method

1. Inspect the current adapter source, tests, trusted-profile loader/source
   validator, execute policy, accepted ADRs, and security documentation.
2. Separate implemented facts from future design recommendations.
3. Evaluate each provider against a host-controlled configuration path,
   including executable identity, environment, cwd, limits, discovery, exact
   permission assignment, lifecycle, inventory, and validation side effects.
4. Compare a provider-specific schema with generic alternatives, recommend
   atomic construction, and define deterministic and opt-in local validation.
5. End with one readiness recommendation and a narrowly scoped proposed Task
   032, then stop at the requested checkpoint.

## Acceptance

- `docs/RAH_V0.4_EXTERNAL_PROVIDER_PROFILE_RESEARCH.md` records current code as
  the source of truth and clearly labels VERIFIED, IMPLEMENTED, PLANNED, and
  DEFERRED material.
- It evaluates MCP and Process Plugin independently and does not imply they
  have equivalent security properties.
- It makes exactly one final A/B/C/D recommendation.
- No production source, Cargo manifest, dependency, protocol/API, or provider
  implementation changes.

## Validation

Run only documentation checks:

```powershell
git diff --check
git diff --no-index --check -- NUL docs/RAH_V0.4_EXTERNAL_PROVIDER_PROFILE_RESEARCH.md
git diff --no-index --check -- NUL docs/plans/2026-08-22-external-provider-profile-research.md
git status --short
git diff
```
