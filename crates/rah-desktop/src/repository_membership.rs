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
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::Ordering,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct LinkedFixture {
        base: PathBuf,
        git: PathBuf,
        main: PathBuf,
        linked_a: PathBuf,
        linked_b: PathBuf,
    }

    impl LinkedFixture {
        fn new() -> Self {
            let sequence = NEXT_WORKSPACE_EPOCH.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir().join(format!(
                "RAH_V030_PRIVATE_COMMON_SENTINEL-{}-{sequence}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time should follow Unix epoch")
                    .as_nanos()
            ));
            let main = base.join("main");
            let linked_a = base.join("linked-a");
            let linked_b = base.join("linked-b");
            fs::create_dir_all(&main).expect("main worktree should be created");
            #[cfg(windows)]
            let output = Command::new("where.exe")
                .arg("git.exe")
                .output()
                .expect("Git should be discoverable");
            #[cfg(not(windows))]
            let output = Command::new("which")
                .arg("git")
                .output()
                .expect("Git should be discoverable");
            assert!(output.status.success(), "Git should be available");
            let git = fs::canonicalize(
                String::from_utf8(output.stdout)
                    .expect("Git path should be UTF-8")
                    .lines()
                    .next()
                    .expect("Git path should be present"),
            )
            .expect("Git executable should be canonical");
            run(&git, &main, &["init", "--quiet", "--initial-branch=main"]);
            run(&git, &main, &["config", "user.name", "RAH Membership Test"]);
            run(
                &git,
                &main,
                &["config", "user.email", "rah@example.invalid"],
            );
            run(&git, &main, &["config", "core.autocrlf", "false"]);
            fs::write(main.join("base.txt"), b"base\n").expect("tracked file should be written");
            run(&git, &main, &["add", "--", "base.txt"]);
            run(&git, &main, &["commit", "--quiet", "-m", "base"]);
            run(
                &git,
                &main,
                &[
                    "worktree",
                    "add",
                    "--quiet",
                    "-b",
                    "linked-a",
                    linked_a.to_str().expect("fixture path should be UTF-8"),
                ],
            );
            run(
                &git,
                &main,
                &[
                    "worktree",
                    "add",
                    "--quiet",
                    "-b",
                    "linked-b",
                    linked_b.to_str().expect("fixture path should be UTF-8"),
                ],
            );
            Self {
                base,
                git,
                main,
                linked_a,
                linked_b,
            }
        }
    }

    impl Drop for LinkedFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn run(git: &Path, root: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .expect("Git fixture command should start");
        assert!(
            output.status.success(),
            "Git fixture command failed {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

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

    #[tokio::test]
    async fn sibling_worktrees_are_distinct_members_and_removal_keeps_git_registration() {
        use super::WorkspaceMembershipRemoval;

        let fixture = LinkedFixture::new();
        let roots = [&fixture.main, &fixture.linked_a, &fixture.linked_b];
        let identities =
            roots.map(|root| RepositoryAdmissionIdentity::capture(&fixture.git, root).unwrap());
        for identity in &identities {
            identity.validate_git().await.unwrap();
        }
        let common_dirs = roots.map(|root| {
            let output = run(
                &fixture.git,
                root,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            );
            fs::canonicalize(String::from_utf8(output).unwrap().trim()).unwrap()
        });
        assert_eq!(common_dirs[0], common_dirs[1]);
        assert_eq!(common_dirs[1], common_dirs[2]);

        let mut membership = WorkspaceMembershipState::new();
        let mut members = Vec::new();
        for (name, root, identity) in [
            ("main", &fixture.main, &identities[0]),
            ("A", &fixture.linked_a, &identities[1]),
            ("B", &fixture.linked_b, &identities[2]),
        ] {
            assert_eq!(membership.relation_to_existing(identity), None);
            members.push(membership.admit(name.to_owned(), root.clone(), identity.clone()));
        }
        assert_ne!(members[0].id, members[1].id);
        assert_ne!(members[1].id, members[2].id);
        assert_ne!(members[0].id, members[2].id);
        assert_eq!(membership.active_member(), None);
        let membership_json =
            serde_json::to_string(&crate::repository_membership_presentation_from(&membership))
                .unwrap();
        assert!(!membership_json.contains("RAH_V030_PRIVATE_COMMON_SENTINEL"));
        assert!(!membership_json.contains(".git"));
        assert_eq!(
            membership.relation_to_existing(&identities[1]),
            Some(rah_tools::RepositoryAdmissionRelation::Same)
        );

        let alias = RepositoryAdmissionIdentity::capture(
            &fixture.git,
            fixture.linked_a.join("..").join("linked-a"),
        )
        .unwrap();
        assert_eq!(
            membership.relation_to_existing(&alias),
            Some(rah_tools::RepositoryAdmissionRelation::Same)
        );
        let copied = fixture.base.join("copied");
        fs::create_dir(&copied).expect("copied candidate root should be created");
        fs::copy(fixture.linked_a.join(".git"), copied.join(".git"))
            .expect("candidate gitfile should be copied");
        assert!(RepositoryAdmissionIdentity::capture(&fixture.git, &copied).is_err());

        assert!(membership.publish_active(members[0].id));
        assert!(membership.publish_active(members[1].id));
        assert!(membership.publish_active(members[2].id));
        assert_eq!(membership.active_member(), Some(members[2].id));
        let worktrees_before_close = run(
            &fixture.git,
            &fixture.main,
            &["worktree", "list", "--porcelain", "-z"],
        );
        let listed = String::from_utf8_lossy(&worktrees_before_close).replace('\\', "/");
        assert_eq!(listed.matches("worktree ").count(), 3);
        let refs_before_close = run(&fixture.git, &fixture.main, &["show-ref"]);
        assert!(matches!(
            membership.deactivate_expected(members[2].id),
            WorkspaceMembershipDeactivation::Deactivated
        ));
        assert!(matches!(
            membership.remove(members[2].id),
            WorkspaceMembershipRemoval::Removed
        ));
        assert!(fixture.linked_b.is_dir());
        assert_eq!(
            run(
                &fixture.git,
                &fixture.main,
                &["worktree", "list", "--porcelain", "-z"],
            ),
            worktrees_before_close
        );
        assert_eq!(
            run(&fixture.git, &fixture.main, &["show-ref"]),
            refs_before_close
        );
        assert!(membership.publish_active(members[1].id));
        assert_eq!(membership.active_member(), Some(members[1].id));

        let before = worktrees_before_close;
        assert!(matches!(
            membership.remove(members[1].id),
            WorkspaceMembershipRemoval::Active
        ));
        assert!(matches!(
            membership.deactivate_expected(members[1].id),
            WorkspaceMembershipDeactivation::Deactivated
        ));
        assert!(membership.publish_active(members[0].id));
        assert!(matches!(
            membership.remove(members[1].id),
            WorkspaceMembershipRemoval::Removed
        ));
        assert!(fixture.linked_a.is_dir());
        assert_eq!(
            run(
                &fixture.git,
                &fixture.main,
                &["worktree", "list", "--porcelain", "-z"],
            ),
            before
        );
        assert_eq!(
            run(&fixture.git, &fixture.main, &["show-ref"]),
            refs_before_close
        );
        let fresh_a =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        fresh_a.validate_git().await.unwrap();
        assert_eq!(membership.relation_to_existing(&fresh_a), None);
        let fresh_member =
            membership.admit("A again".to_owned(), fixture.linked_a.clone(), fresh_a);
        assert_ne!(fresh_member.id, members[1].id);
        assert_eq!(membership.active_member(), Some(members[0].id));
    }
}
