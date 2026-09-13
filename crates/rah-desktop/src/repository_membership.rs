//! Process-local, inert repository membership state.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_tools::{RepositoryAdmissionIdentity, RepositoryAdmissionRelation};

static NEXT_WORKSPACE_EPOCH: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RepositoryMemberId {
    workspace_epoch: u64,
    ordinal: u64,
}

impl RepositoryMemberId {
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

    #[cfg(test)]
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

    pub(crate) fn remove(&mut self, id: RepositoryMemberId) -> Option<InertRepositoryMember> {
        if self.active_member == Some(id) {
            return None;
        }
        self.membership_generation = self.membership_generation.wrapping_add(1);
        self.members.remove(&id)
    }

    pub(crate) fn publish_active(&mut self, id: RepositoryMemberId) -> bool {
        if self.members.contains_key(&id) {
            self.active_member = Some(id);
            true
        } else {
            false
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
