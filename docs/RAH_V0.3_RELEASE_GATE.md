# RAH v0.3 release-gate audit

Status: **READY FOR RELEASE COMMIT**

Date: 2026-08-22

This report records local release preparation only. It does not create a
release commit, tag, push, or claim CI validation for a release commit that
does not yet exist.

## 1. v0.3 scope

v0.3 stops at verified, index-only repository mutation. It adds a hardened
host-owned Execute foundation and bounded Git index capabilities without
authorizing worktree-byte replacement, history/ref mutation, generic command
execution, or network Git.

## 2. Capability classification

### Public / host capabilities

These are the public host-owned Execute capabilities in v0.3:

- `host.cargo.version`
- `host.git.status`
- `host.git.stage`
- `host.git.unstage`

`host.cargo.version` and `host.git.status` are real host-constructed,
capability-specific tools. `host.git.stage` and `host.git.unstage` are real
host-constructed, index-only mutation tools governed by the private
`RepositoryMutationPolicy`.

### Validation fixtures

These are test/live-validation infrastructure, not production/public host
capabilities:

- hardened Execute deterministic/live fixture: `process.test.echo` (with its
  repository-owned fixture executable);
- deterministic/live repository-mutation fixture used to validate
  `RepositoryMutationPolicy`.

The prior `host.fixture.echo` blocker is resolved by this classification and by
renaming the opt-in Execute example's advertised fixture to `process.test.echo`.
No `host.fixture.echo` capability exists or is added by v0.3.

## 3. Verified v0.3 claims

The following are verified by committed implementation plus deterministic and,
where stated, opt-in live-validation evidence:

- Generic Tool Bridge.
- `fs.read`.
- MCP adapter.
- Process Plugin adapter.
- Hardened `HostExecutionPolicy`, through deterministic and live fixture
  validation.
- Real `host.cargo.version`.
- Real `host.git.status`.
- `RepositoryMutationPolicy`.
- Deterministic and live repository-mutation fixture.
- `host.git.stage` deterministic and live validation.
- `host.git.unstage` deterministic and live validation.

The optional Codex adapter baseline is exactly `codex-cli 0.148.0`. Its
dynamic-tool protocol remains experimental and version-pinned.

## 4. Explicitly deferred

- arbitrary `shell.exec`;
- arbitrary `process.exec`;
- model-selected executable, argv, cwd, or environment;
- worktree restore;
- arbitrary file mutation;
- Git commit;
- refs/history mutation;
- reset, clean, checkout, switch, or stash;
- merge or rebase;
- push, pull, or fetch;
- network Git authority;
- credential-bearing Git execution.

Worktree-destructive authority is deferred beyond v0.3 and requires ADR 0011.

## 5. Security boundary and non-guarantees

Model output remains an untrusted request. It reaches tools only through the
RAH `ToolRegistry`, host permission decision, and capability-specific policy.
`PermissionLevel::Execute` is necessary but insufficient: the trusted host
selects executable, repository, symbolic target, argv, cwd, environment, and
timeout.

RAH does not claim that process supervision is OS sandboxing, that it provides
network isolation, or that it can roll back mutations. Timeout or cancellation
may leave uncertain mutation effects, and uncertain mutations are never
automatically replayed. Windows Job Object assignment remains post-spawn.
External OS processes can race repository mutation. Git configuration may
influence Git semantics. These limitations remain documented in
`docs/SECURITY.md` and ADRs 0009 and 0010.

## 6. Version and lockfile verification

The workspace package version is `0.3.0`. Cargo refreshed every workspace
package entry in `Cargo.lock` from `0.2.0` to `0.3.0`. `cargo metadata
--no-deps --format-version 1` is the final check that every workspace package
using `version.workspace = true` resolves to `0.3.0`.

## 7. Deterministic release gate

These commands passed locally against the final uncommitted release patch on
2026-08-22:

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
git status --short
git diff --stat
git diff
```

`cargo metadata` resolved all 11 workspace packages (`rah-cli`, `rah-core`,
`rah-model`, `rah-protocol`, `rah-runtime`, `rah-runtime-codex`, `rah-sandbox`,
`rah-session`, `rah-tools`, `rah-tools-mcp`, and `rah-tools-plugin`) to
`0.3.0`.

The normal suite is offline/deterministic and does not require a model,
credentials, network, paid API, or GPU. Opt-in live Codex examples remain
outside that suite and require the pinned Codex CLI and explicit trusted local
configuration.

## 8. Recommended release commit sequence

1. Review the final release patch and deterministic-gate evidence.
2. Create one coherent release-preparation commit containing the version bump,
   lockfile refresh, fixture-name correction, release notes, and documentation.
3. Re-run the recorded validation commands against that commit in the intended
   release environment.
4. Create an annotated `v0.3.0` tag only after the release commit is clean and
   validation evidence is retained.
5. Push only with separate authorization.
