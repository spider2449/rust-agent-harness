//! Durable, descriptive remembered-workspace candidates.
//!
//! This module deliberately has no repository, Git, provider, runtime, or
//! frontend dependencies. A remembered candidate is only a host-owned catalog
//! record and never an executable repository member.

use rah_protocol::RequestId;
use serde::{
    Serialize, Serializer,
    de::{self, Deserialize, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use std::{
    collections::HashSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf, Prefix},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(test)]
use std::sync::OnceLock;

const FILE_NAME: &str = "remembered-workspace.json";
const COORDINATION_NAME: &str = "remembered-workspace.json.coordination";
const TEMP_PREFIX: &str = "remembered-workspace.json.remembered-workspace-tmp-";
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_MEMBERS: usize = 64;
const MAX_LABEL_BYTES: usize = 256;
const MAX_ID_BYTES: usize = 64;
const MAX_LOCATION_HINT_BYTES: usize = 4096;
const SCHEMA_VERSION: u64 = 1;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
static LOCATION_HINT_PATH_ACCESSES: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestFault {
    Coordination,
    Create,
    Write,
    Sync,
    StagedReparse,
    StagedDifferent,
    Replacement,
    ReparseAncestor,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestOperation {
    Coordination,
    Create,
    Write,
    Sync,
    StagedReparse,
    Replacement,
    FirstMove,
    Cleanup,
}

#[cfg(test)]
#[derive(Default)]
struct TestState {
    fault: Option<(PathBuf, TestFault)>,
    operations: Vec<(PathBuf, TestOperation)>,
}

#[cfg(test)]
static TEST_STATE: OnceLock<Mutex<TestState>> = OnceLock::new();
#[cfg(test)]
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
fn test_state() -> &'static Mutex<TestState> {
    TEST_STATE.get_or_init(|| Mutex::new(TestState::default()))
}

#[cfg(test)]
fn test_lock() -> &'static Mutex<()> {
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn test_fault(path: &Path, fault: TestFault) -> bool {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .fault
        .as_ref()
        .is_some_and(|(candidate, value)| candidate == path && *value == fault)
}

#[cfg(test)]
fn record_operation(path: &Path, operation: TestOperation) {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .operations
        .push((path.to_owned(), operation));
}

#[cfg(test)]
fn set_test_fault(path: PathBuf, fault: TestFault) {
    let mut state = test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.fault = Some((path, fault));
    state.operations.clear();
}

#[cfg(test)]
fn clear_test_state() {
    *test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = TestState::default();
}

#[cfg(test)]
fn test_operations(path: &Path) -> Vec<TestOperation> {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .operations
        .iter()
        .filter_map(|(candidate, operation)| (candidate == path).then_some(*operation))
        .collect()
}

/// A durable opaque identifier for a descriptive remembered candidate.
///
/// This is intentionally a different type from the process-local repository
/// member identifier. It has no repository, identity, or authority meaning.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct RememberedCandidateId(String);

impl RememberedCandidateId {
    pub(crate) fn generate() -> Self {
        Self(RequestId::new().to_string())
    }

    fn parse(value: String) -> Result<Self, ValidationError> {
        if value.is_empty()
            || value.len() > MAX_ID_BYTES
            || !value.is_ascii()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ValidationError::InvalidId);
        }
        Ok(Self(value))
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated native absolute location hint retained only as descriptive
/// user data. Constructing this value does not touch the filesystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RememberedLocationHint(PathBuf);

impl RememberedLocationHint {
    pub(crate) fn parse(path: PathBuf) -> Result<Self, ValidationError> {
        let raw = path.to_str().ok_or(ValidationError::InvalidLocationHint)?;
        if raw.is_empty()
            || raw.len() > MAX_LOCATION_HINT_BYTES
            || !path.is_absolute()
            || raw.chars().any(char::is_control)
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(ValidationError::InvalidLocationHint);
        }

        let mut components = path.components();
        if !matches!(
            components.next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
        ) {
            return Err(ValidationError::InvalidLocationHint);
        }
        if path
            .components()
            .filter_map(|component| match component {
                Component::Normal(part) => Some(part),
                _ => None,
            })
            .any(|part| part.to_str().is_none_or(|value| value.contains(':')))
        {
            return Err(ValidationError::InvalidLocationHint);
        }
        Ok(Self(path))
    }

    #[allow(dead_code)] // Used by a later explicit human-facing presentation action.
    pub(crate) fn path(&self) -> &Path {
        #[cfg(test)]
        LOCATION_HINT_PATH_ACCESSES.fetch_add(1, Ordering::SeqCst);
        &self.0
    }
}

#[cfg(test)]
pub(crate) fn reset_test_location_hint_path_accesses() {
    LOCATION_HINT_PATH_ACCESSES.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn test_location_hint_path_accesses() -> u64 {
    LOCATION_HINT_PATH_ACCESSES.load(Ordering::SeqCst)
}

impl Serialize for RememberedLocationHint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            self.0
                .to_str()
                .ok_or_else(|| serde::ser::Error::custom("location hint is not representable"))?,
        )
    }
}

/// A validated inert remembered workspace member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RememberedWorkspaceMember {
    id: RememberedCandidateId,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location_hint: Option<RememberedLocationHint>,
}

impl RememberedWorkspaceMember {
    pub(crate) fn new(
        id: RememberedCandidateId,
        label: String,
        location_hint: Option<RememberedLocationHint>,
    ) -> Result<Self, ValidationError> {
        validate_label(&label)?;
        Ok(Self {
            id,
            label,
            location_hint,
        })
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn id(&self) -> &RememberedCandidateId {
        &self.id
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    #[allow(dead_code)] // Used by later explicit catalog presentation/action routes.
    pub(crate) fn location_hint(&self) -> Option<&RememberedLocationHint> {
        self.location_hint.as_ref()
    }
}

/// The one descriptive workspace record in the v1 catalog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RememberedWorkspace {
    id: RememberedCandidateId,
    label: String,
    members: Vec<RememberedWorkspaceMember>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_active_member_id: Option<RememberedCandidateId>,
}

impl RememberedWorkspace {
    pub(crate) fn new(
        id: RememberedCandidateId,
        label: String,
        members: Vec<RememberedWorkspaceMember>,
        last_active_member_id: Option<RememberedCandidateId>,
    ) -> Result<Self, ValidationError> {
        validate_label(&label)?;
        if members.len() > MAX_MEMBERS {
            return Err(ValidationError::TooManyMembers);
        }
        let mut ids = HashSet::with_capacity(members.len());
        for member in &members {
            if !ids.insert(member.id.clone()) {
                return Err(ValidationError::DuplicateCandidateId);
            }
        }
        if last_active_member_id
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            return Err(ValidationError::InvalidLastActiveMember);
        }
        Ok(Self {
            id,
            label,
            members,
            last_active_member_id,
        })
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn id(&self) -> &RememberedCandidateId {
        &self.id
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn members(&self) -> &[RememberedWorkspaceMember] {
        &self.members
    }

    #[allow(dead_code)] // Used by later closed catalog presentation/action DTOs.
    pub(crate) fn last_active_member_id(&self) -> Option<&RememberedCandidateId> {
        self.last_active_member_id.as_ref()
    }
}

/// The closed v1 durable remembered-workspace catalog.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RememberedWorkspaceCatalog {
    version: u64,
    workspace: RememberedWorkspace,
}

impl RememberedWorkspaceCatalog {
    pub(crate) fn new(workspace: RememberedWorkspace) -> Result<Self, ValidationError> {
        let catalog = Self {
            version: SCHEMA_VERSION,
            workspace,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub(crate) fn empty() -> Self {
        let workspace = RememberedWorkspace::new(
            RememberedCandidateId::generate(),
            "Remembered workspaces".to_owned(),
            Vec::new(),
            None,
        )
        .expect("the fixed empty catalog is valid");
        Self::new(workspace).expect("the fixed empty catalog is valid")
    }

    #[allow(dead_code)] // Used by later presentation and catalog action routes.
    pub(crate) fn workspace(&self) -> &RememberedWorkspace {
        &self.workspace
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.version != SCHEMA_VERSION {
            return Err(ValidationError::UnsupportedVersion);
        }
        RememberedWorkspace::new(
            self.workspace.id.clone(),
            self.workspace.label.clone(),
            self.workspace.members.clone(),
            self.workspace.last_active_member_id.clone(),
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValidationError {
    InvalidId,
    InvalidLabel,
    InvalidLocationHint,
    TooManyMembers,
    DuplicateCandidateId,
    InvalidLastActiveMember,
    UnsupportedVersion,
    SerializedTooLarge,
    InvalidCatalog,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadError {
    CatalogUnavailable,
    UnsupportedVersion,
    StorageFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StoreError {
    InvalidCatalog,
    StorageFailure,
}

/// Immutable descriptive state captured while Desktop starts.
///
/// This state is intentionally separate from process-local repository
/// membership. Loading it never admits, validates, or activates a repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RememberedWorkspaceStartupState {
    Available(RememberedWorkspaceCatalog),
    Unavailable(RememberedWorkspaceLoadStatus),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RememberedWorkspaceLoadStatus {
    CatalogUnavailable,
    UnsupportedVersion,
    StorageFailure,
}

/// Loads only the dedicated remembered-workspace catalog for startup.
///
/// In particular, this function never dereferences a candidate location hint.
pub(crate) fn load_startup_state(directory: &Path) -> RememberedWorkspaceStartupState {
    let store = match RememberedWorkspaceStore::open(directory.to_owned()) {
        Ok(store) => store,
        Err(StoreError::StorageFailure | StoreError::InvalidCatalog) => {
            return RememberedWorkspaceStartupState::Unavailable(
                RememberedWorkspaceLoadStatus::StorageFailure,
            );
        }
    };
    match store.load() {
        Ok(catalog) => RememberedWorkspaceStartupState::Available(catalog),
        Err(LoadError::CatalogUnavailable) => RememberedWorkspaceStartupState::Unavailable(
            RememberedWorkspaceLoadStatus::CatalogUnavailable,
        ),
        Err(LoadError::UnsupportedVersion) => RememberedWorkspaceStartupState::Unavailable(
            RememberedWorkspaceLoadStatus::UnsupportedVersion,
        ),
        Err(LoadError::StorageFailure) => RememberedWorkspaceStartupState::Unavailable(
            RememberedWorkspaceLoadStatus::StorageFailure,
        ),
    }
}

/// Owns only the remembered-workspace catalog in the application data root.
pub(crate) struct RememberedWorkspaceStore {
    directory: PathBuf,
    coordination: Mutex<()>,
}

impl RememberedWorkspaceStore {
    pub(crate) fn open(directory: PathBuf) -> Result<Self, StoreError> {
        validate_storage_directory(&directory).map_err(|_| StoreError::StorageFailure)?;
        Ok(Self {
            directory,
            coordination: Mutex::new(()),
        })
    }

    #[allow(dead_code)] // Used by later explicit catalog mutation actions.
    pub(crate) fn generate_candidate_id(&self) -> RememberedCandidateId {
        RememberedCandidateId::generate()
    }

    pub(crate) fn load(&self) -> Result<RememberedWorkspaceCatalog, LoadError> {
        let _guard = self
            .coordination
            .try_lock()
            .map_err(|_| LoadError::StorageFailure)?;
        validate_storage_directory(&self.directory).map_err(|_| LoadError::StorageFailure)?;
        let Some(bytes) = read_catalog_bytes(&self.directory)? else {
            return Ok(RememberedWorkspaceCatalog::empty());
        };
        match parse_catalog(&bytes) {
            Ok(catalog) => Ok(catalog),
            Err(ParseError::UnsupportedVersion) => Err(LoadError::UnsupportedVersion),
            Err(ParseError::Invalid) => Err(LoadError::CatalogUnavailable),
        }
    }

    #[allow(dead_code)] // Used by later explicit catalog mutation actions.
    pub(crate) fn save(&self, catalog: &RememberedWorkspaceCatalog) -> Result<(), StoreError> {
        catalog.validate().map_err(|_| StoreError::InvalidCatalog)?;
        let bytes = serialize_catalog(catalog).map_err(|_| StoreError::InvalidCatalog)?;
        let _guard = self
            .coordination
            .try_lock()
            .map_err(|_| StoreError::StorageFailure)?;
        validate_storage_directory(&self.directory).map_err(|_| StoreError::StorageFailure)?;
        ensure_storage_directory(&self.directory).map_err(|_| StoreError::StorageFailure)?;
        let coordination =
            CoordinationFile::acquire(&self.directory).map_err(|_| StoreError::StorageFailure)?;
        let result = atomic_replace(&self.directory, catalog, &bytes);
        drop(coordination);
        result.map_err(|_| StoreError::StorageFailure)
    }

    #[allow(dead_code)] // Used by later explicit catalog mutation actions.
    pub(crate) fn delete(&self) -> Result<(), StoreError> {
        let _guard = self
            .coordination
            .try_lock()
            .map_err(|_| StoreError::StorageFailure)?;
        validate_storage_directory(&self.directory).map_err(|_| StoreError::StorageFailure)?;
        if !storage_directory_exists(&self.directory).map_err(|_| StoreError::StorageFailure)? {
            return Ok(());
        }
        let coordination =
            CoordinationFile::acquire(&self.directory).map_err(|_| StoreError::StorageFailure)?;
        let path = self.path();
        let result = match checked_file_state(&path).map_err(|_| StoreError::StorageFailure)? {
            false => Ok(()),
            true => fs::remove_file(path).map_err(|_| StoreError::StorageFailure),
        };
        drop(coordination);
        result
    }

    fn path(&self) -> PathBuf {
        self.directory.join(FILE_NAME)
    }
}

fn validate_label(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() || value.len() > MAX_LABEL_BYTES || value.chars().any(char::is_control) {
        Err(ValidationError::InvalidLabel)
    } else {
        Ok(())
    }
}

fn serialize_catalog(catalog: &RememberedWorkspaceCatalog) -> Result<Vec<u8>, ValidationError> {
    let mut bytes = serde_json::to_vec(catalog).map_err(|_| ValidationError::InvalidCatalog)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_FILE_BYTES {
        return Err(ValidationError::SerializedTooLarge);
    }
    Ok(bytes)
}

#[derive(Default)]
struct RootWire {
    version: Option<u64>,
    workspace: Option<WorkspaceWire>,
}

#[derive(Default)]
struct VersionProbe {
    version: Option<u64>,
}

impl<'de> Deserialize<'de> for VersionProbe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct VersionVisitor;
        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = VersionProbe;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("remembered workspace version probe")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut probe = VersionProbe::default();
                while let Some(key) = map.next_key::<String>()? {
                    if key == "version" {
                        if probe.version.is_some() {
                            return Err(de::Error::duplicate_field("version"));
                        }
                        probe.version = Some(map.next_value()?);
                    } else {
                        map.next_value::<de::IgnoredAny>()?;
                    }
                }
                Ok(probe)
            }
        }
        deserializer.deserialize_map(VersionVisitor)
    }
}

#[derive(Default)]
struct WorkspaceWire {
    id: Option<String>,
    label: Option<String>,
    members: Option<Vec<MemberWire>>,
    last_active_member_id: Option<Option<String>>,
}

#[derive(Default)]
struct MemberWire {
    id: Option<String>,
    label: Option<String>,
    location_hint: Option<Option<String>>,
}

struct BoundedMembers;

impl<'de> DeserializeSeed<'de> for BoundedMembers {
    type Value = Vec<MemberWire>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct MembersVisitor;
        impl<'de> Visitor<'de> for MembersVisitor {
            type Value = Vec<MemberWire>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("at most 64 remembered workspace members")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut members = Vec::new();
                while let Some(member) = sequence.next_element::<MemberWire>()? {
                    if members.len() >= MAX_MEMBERS {
                        return Err(de::Error::custom("member count exceeds bound"));
                    }
                    members.push(member);
                }
                Ok(members)
            }
        }
        deserializer.deserialize_seq(MembersVisitor)
    }
}

impl<'de> Deserialize<'de> for RootWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct RootVisitor;
        impl<'de> Visitor<'de> for RootVisitor {
            type Value = RootWire;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("closed remembered workspace root")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut root = RootWire::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "version" => {
                            if root.version.is_some() {
                                return Err(de::Error::duplicate_field("version"));
                            }
                            root.version = Some(map.next_value()?);
                        }
                        "workspace" => {
                            if root.workspace.is_some() {
                                return Err(de::Error::duplicate_field("workspace"));
                            }
                            root.workspace = Some(map.next_value()?);
                        }
                        _ => return Err(de::Error::unknown_field(&key, &["version", "workspace"])),
                    }
                }
                Ok(root)
            }
        }
        deserializer.deserialize_map(RootVisitor)
    }
}

impl<'de> Deserialize<'de> for WorkspaceWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct WorkspaceVisitor;
        impl<'de> Visitor<'de> for WorkspaceVisitor {
            type Value = WorkspaceWire;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("closed remembered workspace object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut workspace = WorkspaceWire::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "id" => {
                            if workspace.id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            workspace.id = Some(map.next_value()?);
                        }
                        "label" => {
                            if workspace.label.is_some() {
                                return Err(de::Error::duplicate_field("label"));
                            }
                            workspace.label = Some(map.next_value()?);
                        }
                        "members" => {
                            if workspace.members.is_some() {
                                return Err(de::Error::duplicate_field("members"));
                            }
                            workspace.members = Some(map.next_value_seed(BoundedMembers)?);
                        }
                        "last_active_member_id" => {
                            if workspace.last_active_member_id.is_some() {
                                return Err(de::Error::duplicate_field("last_active_member_id"));
                            }
                            workspace.last_active_member_id = Some(map.next_value()?);
                        }
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["id", "label", "members", "last_active_member_id"],
                            ));
                        }
                    }
                }
                Ok(workspace)
            }
        }
        deserializer.deserialize_map(WorkspaceVisitor)
    }
}

impl<'de> Deserialize<'de> for MemberWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct MemberVisitor;
        impl<'de> Visitor<'de> for MemberVisitor {
            type Value = MemberWire;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("closed remembered workspace member")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut member = MemberWire::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "id" => {
                            if member.id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            member.id = Some(map.next_value()?);
                        }
                        "label" => {
                            if member.label.is_some() {
                                return Err(de::Error::duplicate_field("label"));
                            }
                            member.label = Some(map.next_value()?);
                        }
                        "location_hint" => {
                            if member.location_hint.is_some() {
                                return Err(de::Error::duplicate_field("location_hint"));
                            }
                            member.location_hint = Some(map.next_value()?);
                        }
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["id", "label", "location_hint"],
                            ));
                        }
                    }
                }
                Ok(member)
            }
        }
        deserializer.deserialize_map(MemberVisitor)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParseError {
    Invalid,
    UnsupportedVersion,
}

fn parse_catalog(bytes: &[u8]) -> Result<RememberedWorkspaceCatalog, ParseError> {
    if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES || bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(ParseError::Invalid);
    }
    let probe: VersionProbe = serde_json::from_slice(bytes).map_err(|_| ParseError::Invalid)?;
    if probe.version.ok_or(ParseError::Invalid)? > SCHEMA_VERSION {
        return Err(ParseError::UnsupportedVersion);
    }
    let root: RootWire = serde_json::from_slice(bytes).map_err(|_| ParseError::Invalid)?;
    if root.version != Some(SCHEMA_VERSION) {
        return Err(ParseError::Invalid);
    }
    let workspace = root.workspace.ok_or(ParseError::Invalid)?;
    let id = RememberedCandidateId::parse(workspace.id.ok_or(ParseError::Invalid)?)
        .map_err(|_| ParseError::Invalid)?;
    let label = workspace.label.ok_or(ParseError::Invalid)?;
    let member_wires = workspace.members.ok_or(ParseError::Invalid)?;
    let mut members = Vec::with_capacity(member_wires.len());
    for member in member_wires {
        let id = RememberedCandidateId::parse(member.id.ok_or(ParseError::Invalid)?)
            .map_err(|_| ParseError::Invalid)?;
        let label = member.label.ok_or(ParseError::Invalid)?;
        let location_hint = match member.location_hint {
            None => None,
            Some(Some(value)) => Some(
                RememberedLocationHint::parse(PathBuf::from(value))
                    .map_err(|_| ParseError::Invalid)?,
            ),
            Some(None) => return Err(ParseError::Invalid),
        };
        members.push(
            RememberedWorkspaceMember::new(id, label, location_hint)
                .map_err(|_| ParseError::Invalid)?,
        );
    }
    let last_active_member_id = match workspace.last_active_member_id {
        None => None,
        Some(Some(value)) => {
            Some(RememberedCandidateId::parse(value).map_err(|_| ParseError::Invalid)?)
        }
        Some(None) => return Err(ParseError::Invalid),
    };
    let workspace = RememberedWorkspace::new(id, label, members, last_active_member_id)
        .map_err(|_| ParseError::Invalid)?;
    RememberedWorkspaceCatalog::new(workspace).map_err(|_| ParseError::Invalid)
}

fn read_catalog_bytes(directory: &Path) -> Result<Option<Vec<u8>>, LoadError> {
    if !storage_directory_exists(directory).map_err(|_| LoadError::StorageFailure)? {
        return Ok(None);
    }
    let path = directory.join(FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(LoadError::StorageFailure),
    };
    if !is_regular_non_reparse_file(&metadata) {
        return Err(LoadError::StorageFailure);
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(LoadError::CatalogUnavailable);
    }
    let file = File::open(path).map_err(|_| LoadError::StorageFailure)?;
    let mut bytes = Vec::with_capacity((metadata.len() as usize).min(MAX_FILE_BYTES));
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| LoadError::StorageFailure)?;
    if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
        return Err(LoadError::CatalogUnavailable);
    }
    Ok(Some(bytes))
}

fn validate_storage_directory(directory: &Path) -> io::Result<()> {
    if !directory.is_absolute()
        || directory
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported storage root",
        ));
    }
    #[cfg(windows)]
    {
        let mut components = directory.components();
        if !matches!(
            components.next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported storage root",
            ));
        }
    }
    let mut current = directory.to_owned();
    let mut found_existing = false;
    loop {
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                found_existing = true;
                #[cfg(test)]
                if test_fault(directory, TestFault::ReparseAncestor)
                    && current == directory.parent().unwrap_or(directory)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "unsafe storage root",
                    ));
                }
                if !is_regular_non_reparse_directory(&metadata) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "unsafe storage root",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent.to_owned();
    }
    if found_existing {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "storage root unavailable",
        ))
    }
}

fn ensure_storage_directory(directory: &Path) -> io::Result<()> {
    validate_storage_directory(directory)?;
    fs::create_dir_all(directory)?;
    validate_storage_directory(directory)
}

fn storage_directory_exists(directory: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if !is_regular_non_reparse_directory(&metadata) {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "unsafe storage root",
                ))
            } else {
                Ok(true)
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn is_regular_non_reparse_directory(metadata: &fs::Metadata) -> bool {
    metadata.is_dir() && !is_reparse(metadata)
}

fn is_regular_non_reparse_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !is_reparse(metadata)
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn read_staged_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !is_regular_non_reparse_file(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe staged catalog",
        ));
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "staged catalog is oversized",
        ));
    }
    let mut bytes = Vec::with_capacity((metadata.len() as usize).min(MAX_FILE_BYTES));
    File::open(path)?
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "staged catalog is empty or oversized",
        ));
    }
    Ok(bytes)
}

fn checked_file_state(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !is_regular_non_reparse_file(&metadata) {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "unsafe catalog file",
                ))
            } else {
                Ok(true)
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

struct CoordinationFile {
    file: Option<File>,
    path: PathBuf,
}

impl CoordinationFile {
    fn acquire(directory: &Path) -> io::Result<Self> {
        let path = directory.join(COORDINATION_NAME);
        #[cfg(test)]
        record_operation(&path, TestOperation::Coordination);
        #[cfg(test)]
        if test_fault(&directory.join(FILE_NAME), TestFault::Coordination) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "test coordination failure",
            ));
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self {
            file: Some(file),
            path,
        })
    }
}

impl Drop for CoordinationFile {
    fn drop(&mut self) {
        let _ = self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

fn atomic_replace(
    directory: &Path,
    catalog: &RememberedWorkspaceCatalog,
    bytes: &[u8],
) -> io::Result<()> {
    let target = directory.join(FILE_NAME);
    let temporary = directory.join(temp_name(TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        #[cfg(test)]
        if test_fault(&target, TestFault::Create) {
            return Err(io::Error::other("test create failure"));
        }
        #[cfg(test)]
        record_operation(&target, TestOperation::Create);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        #[cfg(test)]
        if test_fault(&target, TestFault::Write) {
            return Err(io::Error::other("test write failure"));
        }
        #[cfg(test)]
        record_operation(&target, TestOperation::Write);
        file.write_all(bytes)?;
        file.flush()?;
        #[cfg(test)]
        if test_fault(&target, TestFault::Sync) {
            return Err(io::Error::other("test sync failure"));
        }
        #[cfg(test)]
        record_operation(&target, TestOperation::Sync);
        file.sync_all()?;
        drop(file);
        #[cfg(test)]
        tamper_staged_file(&target, &temporary)?;
        #[cfg(test)]
        record_operation(&target, TestOperation::StagedReparse);
        let staged_bytes = read_staged_bytes(&temporary)?;
        let staged_catalog = parse_catalog(&staged_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "staged catalog invalid"))?;
        if staged_bytes != bytes || &staged_catalog != catalog {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staged catalog differs from requested catalog",
            ));
        }
        validate_storage_directory(directory)?;
        checked_file_state(&temporary)?;
        let exists = checked_file_state(&target)?;
        if exists {
            replace_existing(&target, &temporary)
        } else {
            move_new(&target, &temporary)
        }
    })();
    if result.is_err() {
        #[cfg(test)]
        record_operation(&target, TestOperation::Cleanup);
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
fn tamper_staged_file(target: &Path, temporary: &Path) -> io::Result<()> {
    if test_fault(target, TestFault::StagedReparse) {
        fs::write(temporary, b"{")?;
    } else if test_fault(target, TestFault::StagedDifferent) {
        fs::write(
            temporary,
            br#"{"version":1,"workspace":{"id":"tampered","label":"Tampered","members":[]}}
"#,
        )?;
    }
    Ok(())
}

fn temp_name(sequence: u64) -> String {
    format!("{TEMP_PREFIX}{}-{sequence}", std::process::id())
}

#[cfg(windows)]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn replace_existing(destination: &Path, temporary: &Path) -> io::Result<()> {
    #[cfg(test)]
    {
        record_operation(destination, TestOperation::Replacement);
        if test_fault(destination, TestFault::Replacement) {
            return Err(io::Error::other("test replacement failure"));
        }
    }
    let destination = wide(destination);
    let temporary = wide(temporary);
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            destination.as_ptr(),
            temporary.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } != 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn move_new(destination: &Path, temporary: &Path) -> io::Result<()> {
    #[cfg(test)]
    {
        record_operation(destination, TestOperation::FirstMove);
        if test_fault(destination, TestFault::Replacement) {
            return Err(io::Error::other("test first replacement failure"));
        }
    }
    let destination = wide(destination);
    let temporary = wide(temporary);
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            temporary.as_ptr(),
            destination.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    } != 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::AtomicU64,
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time follows Unix epoch")
                .as_nanos();
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("rah-remembered-workspace-{timestamp}-{sequence}"));
            fs::create_dir(&path).expect("test directory is created");
            Self(path)
        }

        fn file(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            clear_test_state();
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn id(value: &str) -> RememberedCandidateId {
        RememberedCandidateId::parse(value.to_owned()).expect("test ID is valid")
    }

    fn member(value: &str, label: &str) -> RememberedWorkspaceMember {
        RememberedWorkspaceMember::new(id(value), label.to_owned(), None)
            .expect("test member is valid")
    }

    fn catalog(members: Vec<RememberedWorkspaceMember>) -> RememberedWorkspaceCatalog {
        RememberedWorkspaceCatalog::new(
            RememberedWorkspace::new(id("workspace"), "Workspace".to_owned(), members, None)
                .expect("test workspace is valid"),
        )
        .expect("test catalog is valid")
    }

    fn store(directory: &TestDirectory) -> RememberedWorkspaceStore {
        RememberedWorkspaceStore::open(directory.0.clone()).expect("store opens")
    }

    #[test]
    fn minimal_round_trip_preserves_order_and_optional_values() {
        let location = RememberedLocationHint::parse(PathBuf::from(r"C:\work\harness"))
            .expect("location hint is lexical only");
        let first = RememberedWorkspaceMember::new(id("first"), "First".to_owned(), Some(location))
            .expect("first member is valid");
        let second = member("second", "Second");
        let workspace = RememberedWorkspace::new(
            id("workspace"),
            "Workspace".to_owned(),
            vec![first.clone(), second.clone()],
            Some(id("second")),
        )
        .expect("workspace is valid");
        let expected = RememberedWorkspaceCatalog::new(workspace).unwrap();
        let bytes = serialize_catalog(&expected).unwrap();
        assert_eq!(parse_catalog(&bytes).unwrap(), expected);
        assert!(
            bytes
                .windows(b"first".len())
                .any(|window| window == b"first")
        );
        assert_eq!(expected.workspace().members(), &[first, second]);
    }

    #[test]
    fn closed_parser_rejects_structural_errors() {
        let valid = br#"{"version":1,"workspace":{"id":"workspace","label":"Workspace","members":[{"id":"member","label":"Member"}]}}"#;
        for invalid in [
            br#"{"version":1,"workspace":{"id":"workspace","label":"Workspace","members":[],"unknown":true}}"#.as_slice(),
            br#"{"version":1,"version":1,"workspace":{"id":"workspace","label":"Workspace","members":[]}}"#.as_slice(),
            br#"{"version":1,"workspace":{"id":"workspace","members":[]}}"#.as_slice(),
            br#"{"version":1,"workspace":{"id":"workspace","label":null,"members":[]}}"#.as_slice(),
            br#"{"version":"1","workspace":{"id":"workspace","label":"Workspace","members":[]}}"#.as_slice(),
            br#"{"version":1,"workspace":{"id":"workspace","label":"Workspace","members":[]}} trailing"#.as_slice(),
            br#"{"version":1,"workspace":{"id":"workspace","label":"Workspace","members":null}}"#.as_slice(),
            br#"{"version":1,"workspace":{"id":"workspace","label":"Workspace","members":[{"id":"member","label":"Member","location_hint":null}]}}"#.as_slice(),
            br#"{"version":1,"workspace":{"id":"workspace","label":"Workspace","members":[{"id":"member","label":"Member","unknown":true}]}}"#.as_slice(),
        ] {
            assert_eq!(parse_catalog(invalid), Err(ParseError::Invalid), "{invalid:?}");
        }
        assert!(parse_catalog(valid).is_ok());
        assert_eq!(parse_catalog(&[0xff]), Err(ParseError::Invalid));
        assert_eq!(
            parse_catalog(br#"{"version":2,"future":true}"#),
            Err(ParseError::UnsupportedVersion)
        );
    }

    #[test]
    fn bounds_and_invalid_references_fail_closed() {
        assert!(RememberedCandidateId::parse("i".repeat(MAX_ID_BYTES)).is_ok());
        assert!(RememberedCandidateId::parse("i".repeat(MAX_ID_BYTES + 1)).is_err());
        assert!(
            RememberedWorkspaceMember::new(id("id"), "l".repeat(MAX_LABEL_BYTES), None).is_ok()
        );
        assert!(
            RememberedWorkspaceMember::new(id("id"), "l".repeat(MAX_LABEL_BYTES + 1), None)
                .is_err()
        );
        assert!(
            RememberedWorkspaceMember::new(id("unicode"), "界".repeat(MAX_LABEL_BYTES / 3), None,)
                .is_ok()
        );
        assert!(
            RememberedWorkspaceMember::new(
                id("unicode"),
                "界".repeat((MAX_LABEL_BYTES / 3) + 1),
                None,
            )
            .is_err()
        );
        assert!(
            RememberedLocationHint::parse(PathBuf::from(format!(
                r"C:\{}",
                "x".repeat(MAX_LOCATION_HINT_BYTES - 3)
            )))
            .is_ok()
        );
        assert!(
            RememberedLocationHint::parse(PathBuf::from(format!(
                r"C:\{}",
                "x".repeat(MAX_LOCATION_HINT_BYTES)
            )))
            .is_err()
        );
        assert!(
            RememberedWorkspace::new(
                id("workspace"),
                "Workspace".to_owned(),
                vec![member("id", "Member"); MAX_MEMBERS + 1],
                None
            )
            .is_err()
        );
        assert!(
            RememberedWorkspace::new(
                id("workspace"),
                "Workspace".to_owned(),
                vec![member("id", "Member"), member("id", "Other")],
                None
            )
            .is_err()
        );
        assert!(
            RememberedWorkspace::new(
                id("workspace"),
                "Workspace".to_owned(),
                vec![member("id", "Member")],
                Some(id("missing"))
            )
            .is_err()
        );
        assert!(RememberedCandidateId::parse("bad/id".to_owned()).is_err());
        assert!(RememberedCandidateId::parse("bad id".to_owned()).is_err());
    }

    #[test]
    fn sixty_four_members_are_accepted_and_sixty_five_are_rejected() {
        let members = (0..MAX_MEMBERS)
            .map(|index| member(&format!("member-{index}"), "Member"))
            .collect();
        let bytes = serialize_catalog(&catalog(members)).unwrap();
        assert!(parse_catalog(&bytes).is_ok());
        let members: Vec<RememberedWorkspaceMember> = (0..=MAX_MEMBERS)
            .map(|index| member(&format!("member-{index}"), "Member"))
            .collect();
        let value = serde_json::json!({"version":1,"workspace":{"id":"workspace","label":"Workspace","members":members.iter().map(|member| serde_json::json!({"id":member.id.as_str(),"label":member.label})).collect::<Vec<_>>()}});
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(parse_catalog(&bytes), Err(ParseError::Invalid));
    }

    #[test]
    fn generated_ids_are_opaque_distinct_and_path_independent() {
        let first = RememberedCandidateId::generate();
        let second = RememberedCandidateId::generate();
        assert_ne!(first, second);
        assert!(first.as_str().is_ascii());
        assert!(first.as_str().len() <= MAX_ID_BYTES);
        assert!(!first.as_str().contains("C:"));
    }

    #[test]
    fn load_missing_is_empty_without_creating_file() {
        let directory = TestDirectory::new();
        let store = store(&directory);
        assert_eq!(store.load().unwrap().workspace().members(), &[]);
        assert!(!directory.file().exists());
    }

    #[test]
    fn load_valid_preserves_bytes_and_corruption_has_no_partial_result() {
        let directory = TestDirectory::new();
        let store = store(&directory);
        let expected = catalog(vec![member("member", "Member")]);
        store.save(&expected).unwrap();
        let durable = fs::read(directory.file()).unwrap();
        assert_eq!(store.load().unwrap(), expected);
        assert_eq!(fs::read(directory.file()).unwrap(), durable);
        for bytes in [b"{".to_vec(), vec![0xff], vec![b'x'; MAX_FILE_BYTES + 1]] {
            fs::write(directory.file(), &bytes).unwrap();
            assert!(matches!(store.load(), Err(LoadError::CatalogUnavailable)));
        }
        let future = br#"{"version":2,"workspace":{"future":true}}"#;
        fs::write(directory.file(), future).unwrap();
        assert_eq!(store.load(), Err(LoadError::UnsupportedVersion));
        assert_eq!(fs::read(directory.file()).unwrap(), future);
    }

    #[test]
    fn save_reparses_staged_bytes_and_preserves_previous_file_on_failures() {
        let _lock = test_lock().lock().unwrap();
        let directory = TestDirectory::new();
        let store = store(&directory);
        let prior = catalog(vec![member("prior", "Prior")]);
        let next = catalog(vec![member("next", "Next")]);
        let unrelated = directory.0.join("unrelated.txt");
        fs::write(&unrelated, b"keep").unwrap();
        store.save(&prior).unwrap();
        let durable = fs::read(directory.file()).unwrap();
        for fault in [
            TestFault::Create,
            TestFault::Write,
            TestFault::Sync,
            TestFault::StagedReparse,
            TestFault::StagedDifferent,
            TestFault::Replacement,
        ] {
            set_test_fault(directory.file(), fault);
            assert_eq!(
                store.save(&next),
                Err(StoreError::StorageFailure),
                "{fault:?}"
            );
            assert_eq!(fs::read(directory.file()).unwrap(), durable, "{fault:?}");
            assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
            assert!(
                !fs::read_dir(&directory.0)
                    .unwrap()
                    .flatten()
                    .any(|entry| entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX))
            );
            clear_test_state();
        }
        store.save(&next).unwrap();
        assert_eq!(store.load().unwrap(), next);
        assert!(test_operations(&directory.file()).contains(&TestOperation::StagedReparse));
    }

    #[test]
    fn storage_root_accepts_local_ancestors_and_rejects_unsupported_prefixes() {
        let directory = TestDirectory::new();
        let nested = directory.0.join("ordinary-parent").join("storage");
        assert!(validate_storage_directory(&nested).is_ok());
        assert!(RememberedWorkspaceStore::open(nested).is_ok());

        for path in [
            PathBuf::from(r"\\?\C:\rah-remembered-workspace"),
            PathBuf::from(r"\\.\C:\rah-remembered-workspace"),
            PathBuf::from(r"\\server\share\rah-remembered-workspace"),
        ] {
            assert!(matches!(
                RememberedWorkspaceStore::open(path),
                Err(StoreError::StorageFailure)
            ));
        }
    }

    #[test]
    fn reparse_ancestor_rejection_has_no_catalog_or_coordination_mutation() {
        let directory = TestDirectory::new();
        let storage = directory.0.join("ordinary-ancestor").join("storage");
        fs::create_dir_all(&storage).unwrap();
        assert!(fs::symlink_metadata(&storage).unwrap().is_dir());
        let store = RememberedWorkspaceStore::open(storage.clone()).unwrap();
        set_test_fault(storage.clone(), TestFault::ReparseAncestor);
        let expected = catalog(vec![member("next", "Next")]);

        assert_eq!(store.load(), Err(LoadError::StorageFailure));
        assert_eq!(store.save(&expected), Err(StoreError::StorageFailure));
        assert_eq!(store.delete(), Err(StoreError::StorageFailure));
        assert_eq!(fs::read_dir(&storage).unwrap().count(), 0);
        assert!(!storage.join(COORDINATION_NAME).exists());
        assert!(
            !fs::read_dir(&storage)
                .unwrap()
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX))
        );
        clear_test_state();
    }

    #[test]
    fn serialized_file_bound_is_rejected_before_publication() {
        let directory = TestDirectory::new();
        let store = store(&directory);
        let hint = RememberedLocationHint::parse(PathBuf::from(format!(
            r"C:\{}",
            "x".repeat(MAX_LOCATION_HINT_BYTES - 3)
        )))
        .unwrap();
        let members = (0..MAX_MEMBERS)
            .map(|index| {
                RememberedWorkspaceMember::new(
                    id(&format!("member-{index}")),
                    "Member".to_owned(),
                    Some(hint.clone()),
                )
                .unwrap()
            })
            .collect();
        let oversized = catalog(members);
        assert_eq!(
            serialize_catalog(&oversized),
            Err(ValidationError::SerializedTooLarge)
        );
        assert_eq!(store.save(&oversized), Err(StoreError::InvalidCatalog));
        assert!(!directory.file().exists());
    }

    #[test]
    fn coordination_failure_and_delete_are_bounded_and_isolated() {
        let _lock = test_lock().lock().unwrap();
        let directory = TestDirectory::new();
        let store = store(&directory);
        let unrelated = directory.0.join("unrelated.txt");
        fs::write(&unrelated, b"keep").unwrap();
        let catalog = catalog(vec![member("member", "Member")]);
        set_test_fault(directory.file(), TestFault::Coordination);
        assert_eq!(store.save(&catalog), Err(StoreError::StorageFailure));
        assert!(!directory.file().exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
        clear_test_state();
        store.save(&catalog).unwrap();
        assert_eq!(store.delete(), Ok(()));
        assert!(!directory.file().exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
    }

    #[test]
    fn location_validation_is_lexical_and_does_not_probe_candidate_path() {
        let location = PathBuf::from(r"C:\does-not-exist\sentinel");
        let hint = RememberedLocationHint::parse(location.clone()).unwrap();
        assert_eq!(hint.path(), location);
        for invalid in [
            PathBuf::from("relative"),
            PathBuf::from(r"C:\with\..\parent"),
            PathBuf::from(r"\\server\share\path"),
            PathBuf::from(r"\\?\C:\verbatim"),
            PathBuf::from("C:\\control\npath"),
        ] {
            assert!(RememberedLocationHint::parse(invalid).is_err());
        }
    }
}
