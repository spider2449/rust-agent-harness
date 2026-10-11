# Task 095 — Trusted Profile and Generic Tool Bridge Integration for repo.edit-files

## Scope

Trusted Profile v1 admits the closed `repo.edit-files` declaration:

```json
{
  "name": "repo.edit-files",
  "enabled": true,
  "permission": "execute",
  "executable": "git",
  "repository": "repo"
}
```

`profile_version` remains `1`. The deferred host-only
`RepositoryMultiFileEditProfile` stores only symbolic executable and repository
IDs. Static load records configured/unregistered redacted inventory and neither
constructs the tool nor observes or mutates Git/worktree state. Effective
composition resolves the symbolic resources, builds the host-owned tool into a
fresh registry, and promotes inventory to validated/registered atomically.

The Generic Tool Bridge receives the tool only through `ToolDefinition`,
`ToolRegistry`, and `Tool::execute`; production bridge code has no
`repo.edit-files` branch. Deterministic bridge coverage verifies private alias
advertisement, Execute denial before entry, two-file ordered success, opaque
ToolOutput translation, and call-identity dedupe with no replay. No profile or
bridge output exposes resolved host paths.

Mixed local/external composition remains atomic and provider lifecycle remains
unchanged. Windows deterministic tests cover static/effective construction and
generic bridge execution. Ubuntu CI remains required for exact-head regression
evidence. Certified Windows Codex live validation is deferred to Task 097;
there is no cross-file transaction, rollback, or replay claim.
