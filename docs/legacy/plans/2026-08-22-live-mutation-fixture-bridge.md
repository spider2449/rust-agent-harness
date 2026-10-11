# Live mutation fixture bridge validation

## Scope

Add one opt-in `rah-runtime-codex` example that validates the accepted ADR 0010
deterministic repository-mutation fixture through the generic Codex tool bridge.
It owns a temporary root, advertises only the empty-schema fixture capability,
and performs one successful live turn. No Git, shell, arbitrary filesystem, or
public RAH contract is added.

## Validation

The example checks the RAH event lifecycle, one `{}` tool call, bounded output,
the before/after root state, and cleanup. The normal workspace suite remains
deterministic; the live example is run only when Codex access is explicitly
available.
