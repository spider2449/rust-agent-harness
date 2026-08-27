# Task 117 — Desktop Certified Codex Baseline Discovery and Selection

## Completed scope

Desktop resolves Codex on every explicit Connect operation with this fixed
precedence:

```text
RAH_CODEX_EXECUTABLE (non-empty explicit host override)
    -> freshly verified %LOCALAPPDATA%\codex-baselines\0.149.0 baseline
    -> codex PATH/npm compatibility fallback
```

`CODEX_BASELINE_HOME` remains the host-side test/store override already used by
the baseline tooling. Discovery considers only the exact adapter-supported
version, never sibling versions.

## Verification boundary

The Desktop-private resolver validates the closed manifest, Windows x64 target,
regular non-reparse-point files, `MZ` prefix, actual SHA-256 (using Windows CNG),
and exact native `--version` result before passing the absolute path to the
existing adapter. A present but invalid exact-version directory fails closed;
only an absent directory permits PATH fallback.

No baseline files are created, repaired, downloaded, installed, promoted, or
deleted. Baseline acquisition remains the external host tooling concern.

## Presentation and authority

The frontend receives only closed source values (`override`,
`certified_baseline`, or `path`) and the supported version. It receives no
executable path, manifest path, hash, package source, or environment data.
Selection runs only during explicit Connect, does not start a model by itself,
and does not add model, tool, process, or persistence authority.

## Dependency and ADR impact

No Cargo files or dependencies changed. SHA-256 uses the Windows CNG API through
a narrow private binding because the existing workspace SHA-256 dependency is
not a Desktop dependency. No ADR was added or changed.
