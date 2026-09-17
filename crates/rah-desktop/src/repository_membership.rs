//! Process-local, inert repository membership state.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_tools::{RepositoryAdmissionIdentity, RepositoryAdmissionRelation};

static NEXT_WORKSPACE_EPOCH: AtomicU64 = AtomicU64::new(1);

const MEMBER_SELECTOR_PREFIX: &str = "m";
const MEMBER_SELECTOR_MAX_BYTES: usize = 42;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RepositoryMemberId {
    workspace_epoch: u64,
    ordinal: u64,
}

impl RepositoryMemberId {
    pub(crate) fn selector(self) -> String {
        format!(
            "{MEMBER_SELECTOR_PREFIX}{}-{}",
            self.workspace_epoch, self.ordinal
        )
    }

    pub(crate) fn parse_selector(selector: &str) -> Option<Self> {
        if selector.len() > MEMBER_SELECTOR_MAX_BYTES
            || !selector.starts_with(MEMBER_SELECTOR_PREFIX)
        {
            return None;
        }
        let body = selector.strip_prefix(MEMBER_SELECTOR_PREFIX)?;
        let (workspace_epoch, ordinal) = body.split_once('-')?;
        if workspace_epoch.is_empty()
            || ordinal.is_empty()
            || (workspace_epoch.len() > 1 && workspace_epoch.starts_with('0'))
            || (ordinal.len() > 1 && ordinal.starts_with('0'))
            || !workspace_epoch.bytes().all(|byte| byte.is_ascii_digit())
            || !ordinal.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let workspace_epoch = workspace_epoch.parse().ok()?;
        let ordinal = ordinal.parse().ok()?;
        if workspace_epoch == 0 || ordinal == 0 {
            return None;
        }
        Some(Self {
            workspace_epoch,
            ordinal,
        })
    }

    #[cfg(test)]
    pub(crate) fn as_debug_tuple(self) -> (u64, u64) {
        (self.workspace_epoch, self.ordinal)
    }
}

#[derive(Clone)]
pub(crate) struct InertRepositoryMember {
    pub(crate) id: RepositoryMemberId,
    #[allow(dead_code)] // Retained as private fresh-member currentness evidence.
    pub(crate) admission_generation: u64,
    #[allow(dead_code)] // Presentation remains deferred; the path stays host-private.
    pub(crate) display_path: String,
    pub(crate) root: PathBuf,
    pub(crate) identity: RepositoryAdmissionIdentity,
}

pub(crate) struct WorkspaceMembershipState {
    workspace_epoch: u64,
    membership_generation: u64,
    next_ordinal: u64,
    members: BTreeMap<RepositoryMemberId, InertRepositoryMember>,
    active_member: Option<RepositoryMemberId>,
}

pub(crate) enum WorkspaceMembershipRemoval {
    Removed,
    Active,
    NotFound,
}

pub(crate) enum WorkspaceMembershipDeactivation {
    Deactivated,
    NoActive,
    ActiveChanged,
    NotFound,
}

impl WorkspaceMembershipState {
    pub(crate) fn new() -> Self {
        Self {
            workspace_epoch: NEXT_WORKSPACE_EPOCH.fetch_add(1, Ordering::Relaxed),
            membership_generation: 0,
            next_ordinal: 0,
            members: BTreeMap::new(),
            active_member: None,
        }
    }

    pub(crate) fn membership_generation(&self) -> u64 {
        self.membership_generation
    }

    pub(crate) fn active_member(&self) -> Option<RepositoryMemberId> {
        self.active_member
    }

    #[cfg(test)]
    pub(crate) fn member_count(&self) -> usize {
        self.members.len()
    }

    pub(crate) fn member(&self, id: RepositoryMemberId) -> Option<&InertRepositoryMember> {
        self.members.get(&id)
    }

    pub(crate) fn members(&self) -> impl Iterator<Item = &InertRepositoryMember> {
        self.members.values()
    }

    pub(crate) fn admit(
        &mut self,
        display_path: String,
        root: PathBuf,
        identity: RepositoryAdmissionIdentity,
    ) -> InertRepositoryMember {
        self.next_ordinal = self.next_ordinal.wrapping_add(1);
        let id = RepositoryMemberId {
            workspace_epoch: self.workspace_epoch,
            ordinal: self.next_ordinal,
        };
        self.membership_generation = self.membership_generation.wrapping_add(1);
        let member = InertRepositoryMember {
            id,
            admission_generation: self.membership_generation,
            display_path,
            root,
            identity,
        };
        let previous = self.members.insert(id, member.clone());
        debug_assert!(previous.is_none());
        member
    }

    pub(crate) fn remove(&mut self, id: RepositoryMemberId) -> WorkspaceMembershipRemoval {
        if self.active_member == Some(id) {
            return WorkspaceMembershipRemoval::Active;
        }
        if self.members.remove(&id).is_none() {
            return WorkspaceMembershipRemoval::NotFound;
        }
        self.membership_generation = self.membership_generation.wrapping_add(1);
        WorkspaceMembershipRemoval::Removed
    }

    pub(crate) fn publish_active(&mut self, id: RepositoryMemberId) -> bool {
        if self.members.contains_key(&id) {
            self.active_member = Some(id);
            true
        } else {
            false
        }
    }

    pub(crate) fn deactivate_expected(
        &mut self,
        expected_member: RepositoryMemberId,
    ) -> WorkspaceMembershipDeactivation {
        if !self.members.contains_key(&expected_member) {
            return WorkspaceMembershipDeactivation::NotFound;
        }
        match self.active_member {
            None => WorkspaceMembershipDeactivation::NoActive,
            Some(active_member) if active_member != expected_member => {
                WorkspaceMembershipDeactivation::ActiveChanged
            }
            Some(_) => {
                self.active_member = None;
                WorkspaceMembershipDeactivation::Deactivated
            }
        }
    }

    pub(crate) fn relation_to_existing(
        &self,
        identity: &RepositoryAdmissionIdentity,
    ) -> Option<RepositoryAdmissionRelation> {
        self.members
            .values()
            .map(|member| member.identity.relation(identity))
            .find(|relation| *relation != RepositoryAdmissionRelation::Distinct)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MEMBER_SELECTOR_MAX_BYTES, NEXT_WORKSPACE_EPOCH, RepositoryMemberId,
        WorkspaceMembershipDeactivation, WorkspaceMembershipState,
    };
    use rah_tools::RepositoryAdmissionIdentity;
    use std::sync::atomic::Ordering;

    fn admitted_membership() -> (
        WorkspaceMembershipState,
        RepositoryMemberId,
        RepositoryMemberId,
    ) {
        let sequence = NEXT_WORKSPACE_EPOCH.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "rah-membership-primitive-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join(".git")).expect("test repository should be created");
        let executable = std::env::current_exe().expect("test executable should exist");
        let identity = RepositoryAdmissionIdentity::capture(executable, &root)
            .expect("test repository identity should capture");
        let mut membership = WorkspaceMembershipState::new();
        let a = membership.admit("A".to_owned(), root.clone(), identity.clone());
        let b = membership.admit("B".to_owned(), root.clone(), identity);
        std::fs::remove_dir_all(&root).expect("test repository should be removable");
        (membership, a.id, b.id)
    }

    #[test]
    fn selectors_are_bounded_opaque_and_strictly_parseable() {
        let member = RepositoryMemberId {
            workspace_epoch: 7,
            ordinal: 11,
        };
        let selector = member.selector();
        assert_eq!(selector, "m7-11");
        assert!(selector.len() <= MEMBER_SELECTOR_MAX_BYTES);
        assert_eq!(RepositoryMemberId::parse_selector(&selector), Some(member));

        for malformed in [
            "",
            "m",
            "m0-1",
            "m1-0",
            "m01-1",
            "m1-01",
            "m1",
            "m1-1-extra",
            "x1-1",
            "m1-1/path",
            "m18446744073709551616-1",
        ] {
            assert_eq!(
                RepositoryMemberId::parse_selector(malformed),
                None,
                "{malformed}"
            );
        }
        assert_eq!(
            RepositoryMemberId::parse_selector(&"m1-1".repeat(MEMBER_SELECTOR_MAX_BYTES)),
            None
        );
    }

    #[test]
    fn deactivate_retains_member_identity_and_membership_generation() {
        let (mut membership, member_a, _) = admitted_membership();
        let before_generation = membership.membership_generation();
        assert!(membership.publish_active(member_a));

        assert!(matches!(
            membership.deactivate_expected(member_a),
            WorkspaceMembershipDeactivation::Deactivated
        ));
        assert_eq!(membership.active_member(), None);
        assert_eq!(membership.member_count(), 2);
        assert_eq!(
            membership.member(member_a).map(|member| member.id),
            Some(member_a)
        );
        assert_eq!(membership.membership_generation(), before_generation);
    }

    #[test]
    fn deactivate_retains_all_members_and_order() {
        let (mut membership, member_a, member_b) = admitted_membership();
        let before: Vec<_> = membership.members().map(|member| member.id).collect();
        let before_generation = membership.membership_generation();
        assert!(membership.publish_active(member_a));

        assert!(matches!(
            membership.deactivate_expected(member_a),
            WorkspaceMembershipDeactivation::Deactivated
        ));
        assert_eq!(
            membership
                .members()
                .map(|member| member.id)
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(membership.member_count(), 2);
        assert!(membership.member(member_a).is_some());
        assert!(membership.member(member_b).is_some());
        assert_eq!(membership.active_member(), None);
        assert_eq!(membership.membership_generation(), before_generation);
    }

    #[test]
    fn deactivate_rejects_no_active_changed_and_unknown_without_mutation() {
        let (mut membership, member_a, member_b) = admitted_membership();
        let before_generation = membership.membership_generation();
        let before_members: Vec<_> = membership.members().map(|member| member.id).collect();
        let unknown = RepositoryMemberId {
            workspace_epoch: u64::MAX,
            ordinal: u64::MAX,
        };

        assert!(matches!(
            membership.deactivate_expected(member_a),
            WorkspaceMembershipDeactivation::NoActive
        ));
        assert!(membership.publish_active(member_b));
        assert!(matches!(
            membership.deactivate_expected(member_a),
            WorkspaceMembershipDeactivation::ActiveChanged
        ));
        assert!(matches!(
            membership.deactivate_expected(unknown),
            WorkspaceMembershipDeactivation::NotFound
        ));
        assert_eq!(membership.active_member(), Some(member_b));
        assert_eq!(membership.membership_generation(), before_generation);
        assert_eq!(
            membership
                .members()
                .map(|member| member.id)
                .collect::<Vec<_>>(),
            before_members
        );
    }

    #[test]
    fn retained_member_can_be_explicitly_reactivated() {
        let (mut membership, member_a, _) = admitted_membership();
        let before_generation = membership.membership_generation();
        assert!(membership.publish_active(member_a));
        assert!(matches!(
            membership.deactivate_expected(member_a),
            WorkspaceMembershipDeactivation::Deactivated
        ));

        assert!(membership.publish_active(member_a));
        assert_eq!(membership.active_member(), Some(member_a));
        assert_eq!(membership.membership_generation(), before_generation);
    }
}
