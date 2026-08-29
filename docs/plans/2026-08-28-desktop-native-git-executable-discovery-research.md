# Task 124: Desktop Native Git Executable Discovery Research

## Status and scope

Research complete.  This document authorizes no production implementation.
It evaluates a Desktop-private Windows resolver only; it does not create a
generic executable-discovery facility, alter repository authority, invoke Git,
or change Task 123's `safe.directory` environment policy.

Starting revision: `e4716add4a99dfd0fcf1c4883a6dd89712bccea7` (master).

## Current behavior

`rah-desktop`'s Windows-only `selected_git_executable()` reads
`RAH_GIT_EXECUTABLE`, rejects a relative value, and otherwise returns the
sanitized `FrontendError::GitUnavailable`. It is reached from `choose_repository`
after the user has chosen a directory. The resulting path is passed to the
fixed repository tools.

`rah_tools::HostExecutionPolicy` is the authoritative executable validator: it
requires an absolute path, rejects symlinks/reparse points, canonicalizes the
path, requires a regular file and (on Windows) a `.exe`, captures identity, and
revalidates canonical path and identity before every invocation. The Task 123
repository observer continues to own its fixed environment, including exactly
the host-selected repository's `safe.directory` value.

## Upstream discovery evidence

Git for Windows' current installer source explicitly writes `CurrentVersion`,
`InstallPath`, and `LibexecPath` at `Software\\GitForWindows`. It writes the
values to HKLM for an administrator installation and HKCU for a non-admin
installation. The installer comments say these values aid third-party helpers.
`InstallPath` is written as the installation root.

Primary sources:

- Git for Windows installer [`install.iss`](https://github.com/git-for-windows/build-extra/blob/main/installer/install.iss#L2453-L2465).
- Git for Windows [release notes](https://github.com/git-for-windows/build-extra/blob/master/ReleaseNotes.md), which describe these values as helping third-party add-ons locate Git.
- Git for Windows [Git wrapper documentation](https://gitforwindows.org/git-wrapper.html), which identifies `<Git>\\cmd\\git.exe` as a main entry point and explains why the wrapper establishes Git's runtime environment.

## Recommended closed resolution contract

Resolution is lazy and runs only as part of `choose_repository`, before any
repository tool is constructed:

```text
RAH_GIT_EXECUTABLE
  -> HKCU\\Software\\GitForWindows:InstallPath + \\cmd\\git.exe
  -> HKLM\\Software\\GitForWindows:InstallPath + \\cmd\\git.exe
  -> GitUnavailable
```

HKCU deliberately wins over HKLM. It is the installer-recorded location for
the current Windows user; giving it priority makes a per-user installation
usable without changing machine state, while preserving an explicit override
as the highest host-controlled choice. This is a closed list of named sources,
not a general registry or process lookup policy.

The only accepted registry keys and value are:

```text
HKEY_CURRENT_USER\\Software\\GitForWindows    REG_SZ InstallPath
HKEY_LOCAL_MACHINE\\Software\\GitForWindows  REG_SZ InstallPath
```

Ignore `CurrentVersion` and `LibexecPath`: neither is needed to form the
specific supported entry point. Accept `REG_SZ` only, not `REG_EXPAND_SZ`,
`REG_MULTI_SZ`, binary data, or an implicit environment expansion.

The exact suffix is `cmd\\git.exe`. It is sufficient: it is Git for Windows'
documented wrapper entry point and provides the Git-specific environment. Do
not add `mingw64\\bin\\git.exe`, `usr\\bin`, guessed architecture directories,
or recursive executable discovery. The direct mingw executable is a documented
advanced wrapper-bypass use case, not evidence to broaden automatic discovery.

## Invalid-present semantics

Every source has one of three resolver outcomes: `Absent`, `Selected`, or
`Invalid`. Resolution continues only from `Absent`; `Invalid` returns
`GitUnavailable` immediately and never falls through. This avoids silently
replacing a deliberate higher-precedence host selection with another executable.

| Source condition | Outcome |
| --- | --- |
| Override unset | continue to HKCU |
| Override present but relative, missing, non-file, link/reparse, or not `.exe` | unavailable; do not inspect registry |
| HKCU key/value absent | continue to HKLM |
| HKCU `InstallPath` wrong type, malformed UTF-16, empty, relative, or whose `cmd\\git.exe` is invalid | unavailable; do not use HKLM |
| HKLM key/value absent | unavailable |
| HKLM present but invalid | unavailable |
| Registry open/query error other than absence | unavailable; do not fall through |

For a present registry value, reject embedded NULs, missing terminator,
odd-length UTF-16 bytes, malformed UTF-16, empty values, and paths that do not
parse as absolute Windows paths. A bound is required before allocation; use a
small fixed contract limit (for example, at most 32 KiB of registry bytes,
including the required terminator). Values beyond that limit are `Invalid`.

## Validation ownership

The resolver is a source selector, not a second executable-authority validator.
It may do only the minimum structural checks needed to safely form a candidate:
absolute override/install root and the exact `cmd\\git.exe` append. It returns
the candidate plus a private source enum. The existing construction path through
`HostExecutionPolicy` remains the single authoritative validation path for
canonicalization, regular-file and native-extension validation, link/reparse
rejection, identity capture, and later revalidation.

Thus a candidate which is syntactically formed but nonexistent or unsafe is
reported as `Invalid` when the trusted policy constructor rejects it. The
implementation must map that private error to `GitUnavailable` and must not try
the next source. This preserves both fail-closed precedence and one authoritative
validator.

## Registry API and dependency design

Use `windows-sys` 0.61, already a Windows-only `rah-desktop` dependency, with
the `Win32_System_Registry` feature added alongside
`Win32_Storage_FileSystem`. No new crate or RAH dependency edge is needed.

Use `RegOpenKeyExW` with read-only access and `KEY_WOW64_64KEY` for the HKLM
key, then query `InstallPath` with `RegGetValueW` (or a bounded two-call
`RegQueryValueExW` wrapper). `RegGetValueW` is preferred because its flags can
require `RRF_RT_REG_SZ`; request the 64-bit subkey view with
`RRF_SUBKEY_WOW6464KEY` if opening through that API instead. The Desktop target
is verified x86_64 Windows, but explicitly selecting the 64-bit HKLM view
avoids registry redirection ambiguity. Close handles through a small RAII
wrapper if using `RegOpenKeyExW`.

The Windows API is Unicode-first: construct static key/value names as
NUL-terminated UTF-16, first obtain the byte size, enforce the bound and even
byte length, then read into a `u16` buffer. Require exactly one trailing NUL and
decode the content with `String::from_utf16`; do not use lossy conversion or
environment-variable expansion. Registry reads are metadata reads only and
must not start a process.

Microsoft documents `RegGetValueW` type restriction flags and 64-bit-view
selection in [RegGetValueW](https://learn.microsoft.com/windows/win32/api/winreg/nf-winreg-reggetvaluew).

## Portable/custom Git, PATH, and App Paths

Portable Git and custom layouts are supported only by an explicit absolute
`RAH_GIT_EXECUTABLE`; they are not automatically discovered. This keeps the
resolver bounded and avoids filesystem scanning.

Do not add PATH lookup. It would turn ambient process state into a generic
executable-authority shortcut and is not necessary for the upstream installer
contract. Do not add App Paths. Microsoft documents App Paths as a general
ShellExecute-oriented application registration mechanism, but the Git for
Windows installer evidence above registers `Software\\GitForWindows`, not an
`App Paths\\git.exe` key. It would also introduce a distinct, unproven source.

## Startup, diagnostics, and authority

Do not read the registry at application startup. Lazy resolution during
`choose_repository` is sufficient and preserves the present lifecycle:

```text
Desktop startup -> no Git resolution, execution, repository selection, or tools
Choose Repository -> lazy named-source selection -> HostExecutionPolicy validation
                 -> only then DesktopRepository/tool construction
```

No subprocess (including `git --version`, `where.exe`, PowerShell, cmd, or
shell execution) is permitted during resolution. The model never controls a
source, registry key, executable path, repository path, or argv.

Keep source provenance private by default. The frontend already receives the
sanitized `GitUnavailable`; it does not need `override`, `git_for_windows_user`,
or `git_for_windows_machine` to complete the workflow. If later support work
needs it, permit only that closed enum in local structured tracing and never a
path, registry value, user-profile location, or repository path. Do not alter
the IPC error shape in this task.

This adds no repository capability itself. Selection only supplies a candidate
to the already-existing host-owned fixed Git tool composition. Task 123's
isolated system/global config and exact `safe.directory` remain unmodified.

## Deterministic implementation test matrix

Make resolution unit-testable through a private trait or injected reader that
returns source states; production wiring is the only Windows Registry caller.
Tests use a temporary native fixture and fake registry responses, never the
developer's actual registry, PATH, or a subprocess.

1. Valid absolute override is selected and no registry source is read.
2. Invalid override is terminal and does not use HKCU/HKLM.
3. Absent override plus valid HKCU selects HKCU.
4. Absent HKCU plus valid HKLM selects HKLM.
5. Present-invalid HKCU does not use a valid HKLM.
6. Present-invalid HKLM is terminal; both absent yield `GitUnavailable`.
7. Relative override and relative `InstallPath` candidates are rejected.
8. Missing `cmd\\git.exe`, directories, wrong extensions, and `.cmd`, `.bat`,
   and `.ps1` candidates are rejected by the existing authoritative policy.
9. Symlink/reparse candidate rejection follows the existing policy, including
   a construction-time rejection and a later identity/reparse replacement
   revalidation rejection.
10. Wrong registry type, malformed/unterminated UTF-16, embedded NUL, empty,
    oversized value, missing value, and access/query failure have the stated
    absent-or-terminal behavior.
11. Production resolver tests prove no PATH query/command lookup and no
    subprocess execution; resolver interfaces expose no executable name/argv
    parameter.
12. Startup construction tests prove neither registry resolution nor
    repository/tool authority activation occurs before `choose_repository`.
13. Composition tests prove the selected candidate passes through
    `HostExecutionPolicy`, whose canonical identity is captured and revalidated.
14. Existing Task 123 tests continue to prove the exact three-entry Git config
    environment with only the canonical selected repository as `safe.directory`.

## ADR impact and implementation recommendation

No ADR is required. The resolver is a Desktop-private source-selection detail;
it changes no stable RAH boundary, dependency direction, permission model, or
plugin/runtime architecture.

Implement one private `GitExecutableResolver` in `rah-desktop` on Windows:
represent source order and `Absent`/`Invalid`/candidate outcomes explicitly,
read only `InstallPath` as bounded `REG_SZ` from the two named Git for Windows
keys, append only `cmd\\git.exe`, and pass the selected path once into the
existing `HostExecutionPolicy`-backed repository composition. Map every resolver
or policy-validation failure to the existing sanitized `GitUnavailable` and do
not add PATH/App Paths or fallback after an invalid present source.
