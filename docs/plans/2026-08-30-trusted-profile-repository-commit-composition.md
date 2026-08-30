# Task 137: trusted-profile repository commit composition

Baseline: Task 136 commit `243abd5d7a6f5d3a504e956d8c365919609cd430`.

Trusted Profile v1 adds only the closed `repo.commit` declaration: symbolic
repository and native executable resources plus explicit trusted host identity
name/email. Static validation remains structural and never creates hooks,
captures a snapshot, or mutates Git.

Effective composition resolves those resources, constructs the ADR 0016 policy,
and registers a fresh message-only `repo.commit` tool. It also retains a
non-serializable host-only `RepositoryCommitControl`; composition never arms it.
The control explicitly captures one reviewed staged snapshot. A later explicit
arm replaces an unused authorization; authorizations are never queued or
persisted.

Invalid messages do not consume an armed authorization. A valid call consumes
the authorization before policy evaluation, so precondition failure,
known-no-effect, uncertain outcome, and verified commit all require new host
review. Effective inventory is capability-only and redacts identity and live
authorization state. Staged construction is atomic: later provider failure
publishes no registry/control.

Task 138 (Generic Tool Bridge verification) remains explicitly deferred.
