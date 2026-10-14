# ADR 0043: `sandbox-os` on Windows, a Low Integrity Token

Status: Accepted

Date: 2026-10-14

## Context

ADR 0042 added `sandbox-os` for Linux and left Windows for its own
decision: it refused to start there. Two mechanisms were prototyped on
Windows 10 (10.0.19045) before choosing:

- **AppContainer.** `CreateProcess` with only a derived container SID fails
  (`ERROR_FILE_NOT_FOUND`); it needs a registered profile
  (`CreateAppContainerProfile`, which writes to the registry and to
  `%LOCALAPPDATA%\Packages`). A container cannot reach the network, loopback
  included, and reads only what grants `ALL APPLICATION PACKAGES` or the
  container SID, so the root needs an access-control entry for the container.
  But the `NUL` device is denied inside the container: `echo x>nul` fails and
  Git for Windows aborts at start ("could not open '/dev/null'"), so
  `tools-repo` and most build tools cannot run. `cmd.exe` also cannot list
  directories or find batch files, because it touches parents (`C:\`, the
  user profile) that the container cannot read.
- **A Low integrity token.** Windows denies a Low integrity process write
  access to every object whose mandatory label is higher, and an object
  without a label counts as Medium. With the root labeled Low, writes outside
  it failed while reads, `NUL`, Git and `cmd.exe` worked. The network is not
  restricted. No administrator rights are needed, and a label set on a
  directory reaches its existing files (20,000 files took about 1.5 s).

Either way RAH has to call `CreateProcess` itself: std cannot start a
process with another token, and its process-attribute support
(`raw_attribute`) is unstable.

## Decision

1. **`sandbox-os` on Windows runs every program with a Low integrity
   token.** The token is a restricted copy of RAH's own
   (`CreateRestrictedToken` with `DISABLE_MAX_PRIVILEGE`, so no privileges;
   `BUILTIN\Administrators` becomes deny-only when the token has it) with its
   integrity level set to Low. A restricted copy of the caller's token needs
   no privilege to start a process with.

2. **The root, the `writable` paths and a private temporary directory get an
   inheritable Low label** (`S:(ML;OICI;NW;;;LW)`), set with
   `SetNamedSecurityInfoW`, which also relabels what they already contain.
   New files inherit it. The private temporary directory is created under
   the user's temporary directory, passed as `TEMP` and `TMP`, and removed
   when the plugin's scope is disposed.

   **The labels on the root and `writable` stay** after unmount. Removing
   them would rewrite the whole tree on every unmount and again on the next
   mount, and a crash would leave them anyway. A directory that already
   carries the label is not relabeled, so later mounts are fast. `icacls
   "<dir>" /setintegritylevel "(OI)(CI)medium"` restores the default.

   A drive root, the user profile or a directory holding it, and the Windows
   directory or anything under it are refused as root or `writable`: every
   Low integrity process of the user could write there afterwards.

3. **The configuration may not promise what Windows cannot do.** `readable`
   is refused, because reads cannot be narrowed. `network` must be `true`;
   the default (`false`) fails the plugin with a message saying so.

4. **Programs are started by the plugin.** `CreateProcessAsUserW` starts
   the program suspended and without a console window. Only its three
   standard handles are inherited (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`). The
   handles are named pipes (one instance, created first, remote clients
   rejected) served through tokio, and `NUL` as stdin for `run`. The program
   joins the kill-on-close Job Object before it is resumed, so unlike
   `sandbox-process` no descendant can escape before the assignment. The
   exit is observed with a thread-pool wait (`RegisterWaitForSingleObject`).
   Supervision (timeout, output limits, cancellation, unmount, orphaned
   descendants) is the code `sandbox-process` uses, now over a small process
   abstraction that both kinds of child implement.

   The program is found as std finds it: `.exe` is appended when there is no
   extension, a path is relative to the working directory, and a bare name is
   looked up in the program's `PATH`, then the system and Windows
   directories. Batch files are refused: they run through `cmd.exe`, whose
   quoting differs from the C runtime's, and std's escaping for them is not
   reproduced. Running `cmd.exe /c` is explicit.

5. **AppContainer is not used.** A sandbox in which Git cannot start is not
   usable for a coding agent. It stays the candidate for a future
   network-isolated mode if the `NUL` problem is solved.

6. ADR 0042 is unchanged for Linux. `sandbox-os` still refuses to start on
   other platforms, is not in any preset, and is a workspace plugin.

## Guarantees and limits

With `sandbox-os` mounted on Windows, a program started through the
`sandbox` service, and every process it starts:

- cannot create, modify, rename or delete files and directories that are not
  labeled Low: everything outside the root, the private temporary directory
  and `writable`, except locations already labeled Low by Windows or other
  software (`%USERPROFILE%\AppData\LocalLow`);
- cannot write registry keys other than Low ones
  (`HKCU\Software\AppDataLow`);
- cannot open RAH or the user's other Medium processes to write to or
  inject into them, and cannot send input to their windows (UIPI);
- has no privileges, and gains nothing from membership of Administrators;
- belongs to the Job Object from its first instruction.

It does **not**:

- restrict reads: everything the user can read (credentials, browser
  profiles, SSH keys) is readable, and the program's output reaches the
  model, which is untrusted;
- restrict the network in any way. Together with the reads, a program can
  send away whatever the user can read;
- keep other Low integrity programs of the user out of the labeled
  directories;
- make a file moved into the root writable: a move keeps the file's own
  label, so it stays Medium (and unwritable) until labeled again;
- limit CPU, memory, disk use or process count;
- provide rollback. Effects of a terminated program remain uncertain.

## Consequences

- The `sandbox` contract, `rah-api` and the plugin protocol are unchanged.
- The root and `writable` directories of a Windows `sandbox-os` keep a Low
  label: a lasting change to the user's files that the README and
  `docs/SECURITY.md` state.
- Tests run on Windows: writes outside the root are
  denied (also end to end through `shell.exec`), the root, the private
  temporary directory and `writable` can be written, files that existed
  before the mount become writable, the labels remain after unmount,
  timeout, cancellation, orphaned descendants and unmount still end the
  process tree, piped `spawn` works, arguments with spaces, quotes and
  backslashes arrive unchanged, programs are found on `PATH`, and batch
  files and invalid configurations are refused.
- CI runs on Linux only, so the Windows code is checked by the local test
  runs. They ran with an elevated token; a standard user account has not
  been tested.
