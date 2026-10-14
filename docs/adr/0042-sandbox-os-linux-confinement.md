# ADR 0042: `sandbox-os`, OS-Level Confinement on Linux

Status: Accepted; Windows decided in ADR 0043

Date: 2026-10-14

## Context

`sandbox-process` provides the `sandbox` service as process supervision: a
fixed argument vector, a bounded working directory, a cleared environment,
bounded output, a timeout and process-tree termination. It is not an OS
sandbox. A program that `shell.exec`, `tools-repo` or an MCP server starts
can read and write anything the user can and can use the network.

The plugin-gaps plan (G4) asks for a sandbox that provides the same service
with real isolation, can replace `sandbox-process` through `admin.replace`,
refuses to start where its mechanism is unavailable instead of falling back,
and states exactly what it guarantees (AGENTS.md: do not claim isolation
that is not implemented and tested).

Two platform questions were open when G4 was deferred:

- **Windows.** AppContainer needs an ACL entry for the container on the
  working directory, which is a lasting change to host state, and tools
  installed under the user profile (rustup under `~/.cargo`, for example)
  are not readable from the container without further ACL changes. A
  restricted token alone does not confine writes to the user's own files.
- **Linux.** The mechanisms (landlock, seccomp) can only be verified on a
  Linux kernel. WSL 2 (Linux 6.18, landlock ABI 7) is now available locally,
  and CI runs on `ubuntu-latest`.

## Decision

1. **A new catalog plugin, `sandbox-os`, in the `sandbox-process` crate.**
   It provides the unchanged `sandbox` service, so `rah-api` and the plugin
   protocol do not change. It shares the supervision code with
   `sandbox-process` (one crate with two catalog plugins, as
   `model-responses` and `tools-mcp` already do); a separate crate could not
   depend on `sandbox-process` and would duplicate it. Confinement is applied
   in the child between `fork` and `exec`, so it also covers every
   descendant, and the supervision guarantees of `sandbox-process` hold
   unchanged.

2. **Files: landlock, ABI 3 or later (Linux 6.2).** The ruleset handles
   every file-system right the kernel knows (including `TRUNCATE`, the
   reason for the ABI 3 minimum, and `IOCTL_DEV` on ABI 5):
   - `readable` paths (default `/`) may be read, listed and executed;
   - the root, a private temporary directory and `writable` paths get every
     right;
   - `/dev/null`, `/dev/zero` and `/dev/full` stay writable and
     `/dev/random`, `/dev/urandom` readable.

   Relative `readable` and `writable` paths are relative to `root`, and each
   must exist when the plugin starts. The private temporary directory is
   created with mode `0700` under the system temporary directory, passed as
   `TMPDIR`, and removed when the plugin's scope is disposed.

3. **Network: seccomp, unless `network = true`.** The filter returns
   `EACCES` for `socket` with any family other than `AF_UNIX` and `EPERM`
   for `io_uring_setup` (io_uring can create sockets without `socket`), and
   kills a process that makes system calls of another architecture (32-bit
   or x32), which would bypass the check. This blocks TCP, UDP, raw and
   netlink sockets, loopback included. On landlock ABI 6 or later, abstract
   Unix sockets outside the sandbox are also unreachable. The filter exists
   for x86-64 and AArch64; elsewhere `network = false` refuses to start.

   A network namespace was considered and rejected: it needs unprivileged
   user namespaces, which Ubuntu 24.04 (and so the CI runner) restricts
   through AppArmor, so it could neither be relied on nor tested in CI.

4. **Signals.** On landlock ABI 6 or later a confined program cannot signal
   processes outside its sandbox (RAH, the user's other programs).

5. **Fail closed.** Missing landlock, an ABI below 3, missing seccomp
   filters, an unsupported architecture with `network = false`, or a
   `readable`/`writable` path that does not exist fails the plugin. A
   failure to confine one program fails that run with `ExecError::Spawn`;
   nothing runs unconfined.

6. **Other platforms are not supported yet.** On Windows and macOS
   `sandbox-os` fails to start with a message that names `sandbox-process`
   as the unisolated alternative. A Windows implementation needs its own
   decision on the ACL and user-profile questions above. (ADR 0043 made it:
   a Low integrity token, not AppContainer.)

7. **Not in any preset.** Presets keep `sandbox-process`, because
   `sandbox-os` does not run on Windows and its defaults (no network) break
   tools that fetch dependencies. Users opt in by mounting it or replacing
   `sandbox-process` with `admin.replace`. `sandbox-os` is a workspace plugin
   (`rah --root` and `set_workspace` move its root).

## Guarantees and limits

With `sandbox-os` mounted on a supported Linux host, a program started
through the `sandbox` service, and every process it starts:

- cannot create, modify, truncate, rename or delete files outside the root,
  the private temporary directory and `writable`;
- cannot read or execute files outside `readable`, the root, the temporary
  directory and `writable`;
- cannot gain privileges through set-user-ID programs (`no_new_privs`);
- with `network = false`, cannot open IP or other non-Unix sockets.

Landlock also denies `mount`, `pivot_root` and `ptrace` of processes
outside the sandbox, and on ABI 6 the signal and abstract Unix socket
scopes above. These are kernel properties of the ruleset; RAH's tests do
not exercise them.

It does **not**:

- hide anything readable by default: `readable` defaults to `/`, so the
  user's credentials (`~/.ssh`, cloud configuration, browser profiles) are
  readable. Narrow `readable` to protect them; the program's output reaches
  the model, which is untrusted.
- stop connections to Unix-domain socket **files**: landlock does not
  mediate `connect` on a socket path. A program can talk to any socket file
  the user can, such as `ssh-agent` or `/var/run/docker.sock` (which is
  equivalent to root when the user may use Docker).
- restrict the network in any way with `network = true`.
- limit CPU, memory, disk use or process count.
- keep a descendant that leaves the process group (`setsid`) from surviving
  termination, as with `sandbox-process`.
- prevent signals to processes outside the sandbox on ABI 3 to 5.
- provide rollback. Effects of a terminated program remain uncertain.

## Consequences

- The `sandbox` contract, `rah-api` and the plugin protocol are unchanged.
- Tests run the confinement on Linux (CI and WSL): writes outside the root
  are denied (also end to end through `shell.exec`), TCP and UDP are
  blocked, `writable` and `network = true`
  re-enable them, a narrowed `readable` hides files, and timeout,
  cancellation, orphaned descendants and unmount still end the process
  tree. Kernels below ABI 3 skip these tests with a message. On other
  platforms the tests assert that the plugin refuses to start.
- Programs that need the network (package managers fetching dependencies),
  loopback servers, or writes under the home directory (`~/.cargo`
  registries) need `network = true` or `writable` entries.
