use std::collections::HashMap;

use rah_protocol::PermissionLevel;
use thiserror::Error;

/// Opaque provider-neutral identity used for host permission assignment.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExternalToolIdentity(String);

impl ExternalToolIdentity {
    /// Creates a non-empty external identity without interpreting its contents.
    pub fn new(identity: impl Into<String>) -> Result<Self, ExternalToolPermissionError> {
        let identity = identity.into();
        if identity.is_empty() {
            return Err(ExternalToolPermissionError::EmptyIdentity);
        }
        Ok(Self(identity))
    }

    /// Returns the external identity as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid trusted-host external-tool permission configuration.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExternalToolPermissionError {
    /// An external identity cannot be empty.
    #[error("external tool identity must not be empty")]
    EmptyIdentity,
    /// Each external identity may receive exactly one host assignment.
    #[error("permission for external tool `{identity}` is configured more than once")]
    DuplicateIdentity {
        /// Conflicting external identity.
        identity: String,
    },
}

/// Trusted host assignments for tools discovered from an external provider.
///
/// Absence is intentionally distinct from [`PermissionLevel::None`]: callers
/// must reject or omit an unassigned external tool before registration.
#[derive(Clone, Debug, Default)]
pub struct ExternalToolPermissionPolicy {
    assignments: HashMap<ExternalToolIdentity, PermissionLevel>,
}

impl ExternalToolPermissionPolicy {
    /// Creates an empty, default-deny assignment policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assigns a RAH permission to one opaque external tool identity.
    pub fn assign(
        &mut self,
        identity: ExternalToolIdentity,
        permission: PermissionLevel,
    ) -> Result<(), ExternalToolPermissionError> {
        if self.assignments.contains_key(&identity) {
            return Err(ExternalToolPermissionError::DuplicateIdentity {
                identity: identity.0,
            });
        }
        self.assignments.insert(identity, permission);
        Ok(())
    }

    /// Returns the explicit host assignment, or `None` when access is unconfigured.
    #[must_use]
    pub fn permission_for(&self, identity: &ExternalToolIdentity) -> Option<PermissionLevel> {
        self.assignments.get(identity).copied()
    }
}

#[cfg(test)]
mod tests {
    use rah_protocol::PermissionLevel;

    use super::{ExternalToolIdentity, ExternalToolPermissionError, ExternalToolPermissionPolicy};

    #[test]
    fn unknown_identity_has_no_implicit_permission() {
        let policy = ExternalToolPermissionPolicy::new();
        let identity = ExternalToolIdentity::new("echo").expect("identity should be valid");

        assert_eq!(policy.permission_for(&identity), None);
    }

    #[test]
    fn assignments_are_identity_based_and_can_differ() {
        let mut policy = ExternalToolPermissionPolicy::new();
        let read = ExternalToolIdentity::new("read").expect("identity should be valid");
        let write = ExternalToolIdentity::new("write").expect("identity should be valid");
        policy
            .assign(read.clone(), PermissionLevel::Read)
            .expect("first assignment should succeed");
        policy
            .assign(write.clone(), PermissionLevel::Write)
            .expect("first assignment should succeed");

        assert_eq!(policy.permission_for(&read), Some(PermissionLevel::Read));
        assert_eq!(policy.permission_for(&write), Some(PermissionLevel::Write));
    }

    #[test]
    fn malformed_and_duplicate_assignments_are_rejected() {
        assert_eq!(
            ExternalToolIdentity::new(""),
            Err(ExternalToolPermissionError::EmptyIdentity)
        );

        let mut policy = ExternalToolPermissionPolicy::new();
        let identity = ExternalToolIdentity::new("echo").expect("identity should be valid");
        policy
            .assign(identity.clone(), PermissionLevel::None)
            .expect("first assignment should succeed");
        assert_eq!(
            policy.assign(identity, PermissionLevel::Execute),
            Err(ExternalToolPermissionError::DuplicateIdentity {
                identity: "echo".to_owned()
            })
        );
    }
}
