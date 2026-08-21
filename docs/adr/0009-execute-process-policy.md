# ADR 0009 — Host-owned process execution policy

Status: Accepted

## Context

`PermissionLevel::Execute` identifies that a tool requires subprocess authority,
but it does not constrain the executable, arguments, working directory,
environment, resource use, or process lifetime. Model and tool input is
untrusted data and cannot grant those authorities. The existing generic
`ShellExecTool` accepts model-selected process details and therefore remains
unsuitable for live model exposure.

Recommendation B in `docs/RAH_V0.3_EXECUTE_SECURITY_RESEARCH.md` defines the
minimum deterministic hardened boundary for initial v0.3 Execute support.

## Decision

RAH accepts Recommendation B with these rules:

1. `PermissionLevel::Execute` is necessary but not sufficient authority.
2. Every executable capability must additionally pass a host-owned
   `HostExecutionPolicy`.
3. Model and tool input is a request, never execution authorization.
4. A model cannot select arbitrary executables.
5. Initial v0.3 execution uses capability-specific tools backed by canonical
   executables selected by the host.
6. Executables are resolved or configured by the trusted host.
7. Arbitrary `PATH` lookup is disabled by default.
8. Shell interpretation is not part of v0.3. RAH does not invoke `cmd.exe /c`,
   `powershell -Command`, `sh -c`, or `bash -c`.
9. Model-supplied shell strings are not accepted. Shell metacharacters remain
   literal capability data.
10. Arguments are produced by capability-specific exact or typed policy.
11. The working directory is host-fixed or resolved beneath an explicitly
    configured canonical root.
12. The child environment is cleared by default and populated only from
    explicit trusted host values or an allowlist.
13. Standard input is null or closed for the initial prototype.
14. Standard output and standard error are drained concurrently and bounded.
    Initial maxima are 256 KiB for each stream, 512 KiB combined, and 768 KiB
    for serialized `ToolOutput`.
15. Output overflow is terminal and deterministic.
16. Timeout and cancellation each mean a termination attempt, not rollback.
17. Uncertain execution is never automatically replayed.
18. Execute authority may indirectly exceed Read plus Write authority because
    an allowed process retains the ambient authority of the RAH host process.
19. Process execution is not claimed to be filesystem-isolated or
    network-isolated, and process supervision is not OS sandboxing.
20. Windows process-tree ownership uses Job Objects where implemented. Unix
    process-tree ownership uses process groups where implemented. Descendant
    containment remains best effort and must not be overstated.
21. No architecture-defining RAH public contract changes are authorized for the
    initial prototype.
22. Interactive per-call approval is deferred. Initial v0.3 Execute support is
    host-preauthorized only.

The deterministic prototype uses a repository-owned native fixture executable.
The model-visible input contains only capability-specific data; it never
contains a program, raw argv, cwd, environment, timeout, or shell command.

The existing generic `ShellExecTool` is not deleted or promoted. It remains
unsuitable for live model exposure unless a separately authorized design routes
it through an equivalent host policy. The Codex bridge and its exact-once call
handling remain unchanged, and the new Execute capability is not registered
with a live model in this task.

## Consequences

- Hosts must construct and register each executable capability explicitly and
  must separately enable `PermissionLevel::Execute`.
- The host policy owns executable identity, argv production, cwd, environment,
  output limits, timeout, and supervisor choice.
- Direct process spawning never implies authorization of a shell, script
  association, arbitrary process, filesystem boundary, or network boundary.
- Timeout, cancellation, and overflow may occur after side effects. RAH attempts
  termination but does not claim rollback.
- Current `ToolContext` cannot carry cooperative cancellation. The prototype
  therefore relies on abort-safe execution ownership and termination on future
  drop without claiming confirmed AgentRuntime-to-process cancellation.
- Windows Job Objects and Unix process groups improve process-tree ownership but
  do not contain adversarial descendants or replace an OS sandbox.
- Strong OS isolation, interactive approval, arbitrary command execution, and
  live Codex validation require separate authorization and design work.
