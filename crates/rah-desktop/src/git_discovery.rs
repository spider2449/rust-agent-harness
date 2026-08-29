//! Closed, lazy discovery of the Git for Windows wrapper executable.
//!
//! This module selects only a host-owned candidate. `HostExecutionPolicy`
//! remains the sole authority for native executable validation and identity.

use std::{ffi::OsString, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GitExecutableSource {
    Override,
    GitForWindowsUser,
    GitForWindowsMachine,
}

#[derive(Debug)]
pub(super) struct GitExecutableSelection {
    candidate: PathBuf,
    #[allow(dead_code)]
    source: GitExecutableSource,
}

impl GitExecutableSelection {
    pub(super) fn into_candidate(self) -> PathBuf {
        self.candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GitDiscoveryError {
    Unavailable,
}

const REG_SZ: u32 = 1;
const MAX_REGISTRY_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug)]
struct RegistryValue {
    value_type: u32,
    bytes: Vec<u8>,
}

enum RegistryLookup {
    Absent,
    Present(RegistryValue),
    Invalid,
}

/// Resolves only while choosing a repository; startup never calls this.
pub(super) fn resolve() -> Result<GitExecutableSelection, GitDiscoveryError> {
    #[cfg(test)]
    if crate::STARTUP_COUNTER_TRACKING.load(std::sync::atomic::Ordering::SeqCst) {
        crate::startup_activation_counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .git_resolver += 1;
    }
    resolve_with(std::env::var_os("RAH_GIT_EXECUTABLE"), registry_lookup)
}

fn resolve_with<F>(
    override_value: Option<OsString>,
    mut registry: F,
) -> Result<GitExecutableSelection, GitDiscoveryError>
where
    F: FnMut(GitExecutableSource) -> RegistryLookup,
{
    if let Some(value) = override_value {
        return select_absolute(PathBuf::from(value), GitExecutableSource::Override);
    }
    for source in [
        GitExecutableSource::GitForWindowsUser,
        GitExecutableSource::GitForWindowsMachine,
    ] {
        match registry(source) {
            RegistryLookup::Absent => continue,
            RegistryLookup::Invalid => return Err(GitDiscoveryError::Unavailable),
            RegistryLookup::Present(value) => {
                let install_path =
                    parse_install_path(&value).ok_or(GitDiscoveryError::Unavailable)?;
                return select_absolute(
                    PathBuf::from(install_path).join("cmd").join("git.exe"),
                    source,
                );
            }
        }
    }
    Err(GitDiscoveryError::Unavailable)
}

fn select_absolute(
    candidate: PathBuf,
    source: GitExecutableSource,
) -> Result<GitExecutableSelection, GitDiscoveryError> {
    if !candidate.is_absolute() {
        return Err(GitDiscoveryError::Unavailable);
    }
    Ok(GitExecutableSelection { candidate, source })
}

fn parse_install_path(value: &RegistryValue) -> Option<String> {
    if value.value_type != REG_SZ
        || value.bytes.len() < 2
        || value.bytes.len() > MAX_REGISTRY_BYTES
        || !value.bytes.len().is_multiple_of(2)
    {
        return None;
    }
    let units: Vec<u16> = value
        .bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    if units.last() != Some(&0) || units[..units.len() - 1].contains(&0) {
        return None;
    }
    let path = String::from_utf16(&units[..units.len() - 1]).ok()?;
    (!path.is_empty()).then_some(path)
}

fn registry_lookup(source: GitExecutableSource) -> RegistryLookup {
    registry::read_install_path(source)
}

mod registry {
    use super::{GitExecutableSource, MAX_REGISTRY_BYTES, REG_SZ, RegistryLookup, RegistryValue};
    use std::{ffi::c_void, ptr};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
    };

    const ERROR_FILE_NOT_FOUND: u32 = 2;
    const ERROR_PATH_NOT_FOUND: u32 = 3;
    const GIT_FOR_WINDOWS_KEY: &[u16] = &[
        83, 111, 102, 116, 119, 97, 114, 101, 92, 71, 105, 116, 70, 111, 114, 87, 105, 110, 100,
        111, 119, 115, 0,
    ];
    const INSTALL_PATH: &[u16] = &[73, 110, 115, 116, 97, 108, 108, 80, 97, 116, 104, 0];

    pub(super) fn read_install_path(source: GitExecutableSource) -> RegistryLookup {
        let (root, flags) = match source {
            GitExecutableSource::GitForWindowsUser => (HKEY_CURRENT_USER, RRF_RT_REG_SZ),
            GitExecutableSource::GitForWindowsMachine => {
                (HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY)
            }
            GitExecutableSource::Override => return RegistryLookup::Invalid,
        };
        let mut value_type = 0;
        let mut byte_len = 0u32;
        let status = unsafe {
            RegGetValueW(
                root,
                GIT_FOR_WINDOWS_KEY.as_ptr(),
                INSTALL_PATH.as_ptr(),
                flags,
                &mut value_type,
                ptr::null_mut(),
                &mut byte_len,
            )
        };
        if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
            return RegistryLookup::Absent;
        }
        if status != 0 || !sizing_query_is_valid(byte_len) {
            return RegistryLookup::Invalid;
        }
        let mut bytes = vec![0u8; byte_len as usize];
        let status = unsafe {
            RegGetValueW(
                root,
                GIT_FOR_WINDOWS_KEY.as_ptr(),
                INSTALL_PATH.as_ptr(),
                flags,
                &mut value_type,
                bytes.as_mut_ptr().cast::<c_void>(),
                &mut byte_len,
            )
        };
        if status != 0 || value_type != REG_SZ || !returned_size_is_valid(bytes.len(), byte_len) {
            return RegistryLookup::Invalid;
        }
        let Some(bytes) = copied_bytes(bytes, byte_len) else {
            return RegistryLookup::Invalid;
        };
        RegistryLookup::Present(RegistryValue { value_type, bytes })
    }

    fn sizing_query_is_valid(byte_len: u32) -> bool {
        byte_len > 0 && (byte_len as usize) <= MAX_REGISTRY_BYTES && byte_len.is_multiple_of(2)
    }

    fn copied_bytes(mut allocated: Vec<u8>, returned_len: u32) -> Option<Vec<u8>> {
        returned_size_is_valid(allocated.len(), returned_len).then(|| {
            allocated.truncate(returned_len as usize);
            allocated
        })
    }

    /// The sizing query is allocation capacity only. On a successful data read,
    /// `pcbData` is the authoritative number of bytes copied, including the
    /// terminating NUL for a `REG_SZ` value.
    fn returned_size_is_valid(allocated_len: usize, returned_len: u32) -> bool {
        let returned_len = returned_len as usize;
        returned_len > 0 && returned_len <= allocated_len && returned_len.is_multiple_of(2)
    }

    #[cfg(test)]
    mod tests {
        use super::{copied_bytes, returned_size_is_valid, sizing_query_is_valid};

        #[test]
        fn accepts_any_nonzero_even_copied_size_within_capacity() {
            assert!(returned_size_is_valid(44, 42));
            assert!(returned_size_is_valid(44, 44));
            assert!(returned_size_is_valid(44, 40));
            assert!(!returned_size_is_valid(44, 0));
            assert!(!returned_size_is_valid(44, 43));
            assert!(!returned_size_is_valid(44, 46));
        }

        #[test]
        fn copied_length_not_query_capacity_is_passed_to_the_parser() {
            let mut capacity = r"C:\Program Files\Git"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            capacity.extend_from_slice(&0u16.to_le_bytes());
            assert_eq!(capacity.len(), 42);
            capacity.extend_from_slice(&[0xff, 0xff]);

            let copied = copied_bytes(capacity, 42).expect("42 bytes fit in 44-byte capacity");
            assert_eq!(copied.len(), 42);
            assert_eq!(
                super::super::parse_install_path(&super::super::RegistryValue {
                    value_type: super::super::REG_SZ,
                    bytes: copied,
                }),
                Some(r"C:\Program Files\Git".to_owned())
            );
        }

        #[test]
        fn sizing_query_rejects_oversized_or_invalid_capacity() {
            assert!(sizing_query_is_valid(44));
            assert!(!sizing_query_is_valid(0));
            assert!(!sizing_query_is_valid(3));
            assert!(!sizing_query_is_valid(
                (super::MAX_REGISTRY_BYTES + 2) as u32
            ));
        }

        #[test]
        fn copied_length_rejects_zero_odd_or_larger_than_capacity() {
            let capacity = vec![0; 44];
            assert!(copied_bytes(capacity.clone(), 44).is_some());
            assert!(copied_bytes(capacity.clone(), 42).is_some());
            assert!(copied_bytes(capacity.clone(), 0).is_none());
            assert!(copied_bytes(capacity.clone(), 43).is_none());
            assert!(copied_bytes(capacity, 46).is_none());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, path::PathBuf};

    fn reg_sz(path: &str) -> RegistryLookup {
        let mut bytes = path
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        bytes.extend_from_slice(&0u16.to_le_bytes());
        RegistryLookup::Present(RegistryValue {
            value_type: REG_SZ,
            bytes,
        })
    }
    fn resolve_fake(
        override_value: Option<&str>,
        values: Vec<RegistryLookup>,
    ) -> Result<GitExecutableSelection, GitDiscoveryError> {
        let values = RefCell::new(VecDeque::from(values));
        resolve_with(override_value.map(OsString::from), |_| {
            values
                .borrow_mut()
                .pop_front()
                .unwrap_or(RegistryLookup::Absent)
        })
    }

    #[test]
    fn precedence_is_override_then_user_then_machine() {
        let registry_reads = RefCell::new(0);
        let selected = resolve_with(Some(OsString::from(r"C:\override\git.exe")), |_| {
            *registry_reads.borrow_mut() += 1;
            reg_sz(r"C:\user")
        })
        .unwrap();
        assert_eq!(selected.source, GitExecutableSource::Override);
        assert_eq!(
            *registry_reads.borrow(),
            0,
            "override must not inspect registry"
        );
        let selected = resolve_fake(None, vec![reg_sz(r"C:\user"), reg_sz(r"C:\machine")]).unwrap();
        assert_eq!(selected.source, GitExecutableSource::GitForWindowsUser);
        let selected =
            resolve_fake(None, vec![RegistryLookup::Absent, reg_sz(r"C:\machine")]).unwrap();
        assert_eq!(selected.source, GitExecutableSource::GitForWindowsMachine);
        assert_eq!(
            selected.candidate,
            PathBuf::from(r"C:\machine").join("cmd").join("git.exe")
        );
    }

    #[test]
    fn absent_sources_are_unavailable_and_invalid_sources_do_not_fall_through() {
        assert_eq!(
            resolve_fake(None, vec![RegistryLookup::Absent, RegistryLookup::Absent]).unwrap_err(),
            GitDiscoveryError::Unavailable
        );
        assert_eq!(
            resolve_fake(Some("relative\\git.exe"), vec![reg_sz(r"C:\user")]).unwrap_err(),
            GitDiscoveryError::Unavailable
        );
        assert_eq!(
            resolve_fake(None, vec![RegistryLookup::Invalid, reg_sz(r"C:\machine")]).unwrap_err(),
            GitDiscoveryError::Unavailable
        );
        assert_eq!(
            resolve_fake(None, vec![RegistryLookup::Absent, RegistryLookup::Invalid]).unwrap_err(),
            GitDiscoveryError::Unavailable
        );
    }

    #[test]
    fn registry_value_contract_rejects_invalid_encodings_and_paths() {
        let invalid = |value_type, bytes| RegistryValue { value_type, bytes };
        assert_eq!(parse_install_path(&invalid(2, vec![0, 0])), None);
        assert_eq!(parse_install_path(&invalid(REG_SZ, vec![0, 0])), None);
        assert_eq!(
            parse_install_path(&invalid(REG_SZ, vec![b'a', 0, 0, 0, 0, 0])),
            None
        );
        assert_eq!(
            parse_install_path(&invalid(REG_SZ, vec![0xff, 0xdf, 0, 0])),
            None
        );
        assert_eq!(
            parse_install_path(&invalid(REG_SZ, vec![0; MAX_REGISTRY_BYTES + 2])),
            None
        );
        assert_eq!(parse_install_path(&invalid(REG_SZ, vec![1, 2, 3])), None);
        assert_eq!(parse_install_path(&invalid(REG_SZ, vec![b'a', 0])), None);
        assert_eq!(
            resolve_fake(None, vec![reg_sz("relative"), reg_sz(r"C:\machine")]).unwrap_err(),
            GitDiscoveryError::Unavailable
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "host-only Task 125 production resolver probe"]
    fn live_machine_installation_resolves_and_constructs_desktop_repository() {
        // This is deliberately opt-in: it uses the host registry and confirms
        // the production resolver plus the same repository composition seam as
        // Choose Repository. It never changes normal frontend diagnostics.
        let previous_override = std::env::var_os("RAH_GIT_EXECUTABLE");
        unsafe { std::env::remove_var("RAH_GIT_EXECUTABLE") };

        let result: Result<(), GitDiscoveryError> = (|| {
            assert!(
                matches!(
                    registry_lookup(GitExecutableSource::GitForWindowsUser),
                    RegistryLookup::Absent
                ),
                "live HKCU lookup must be absent before HKLM selection"
            );
            let lookup = registry_lookup(GitExecutableSource::GitForWindowsMachine);
            let RegistryLookup::Present(value) = lookup else {
                panic!("live HKLM registry reader did not return a REG_SZ value");
            };
            assert_eq!(
                value.value_type, REG_SZ,
                "live HKLM value type must be REG_SZ"
            );
            assert_eq!(
                parse_install_path(&value),
                Some(r"C:\Program Files\Git".to_owned()),
                "live HKLM payload must parse using its returned copied byte length"
            );
            let selection = resolve()?;
            assert_eq!(selection.source, GitExecutableSource::GitForWindowsMachine);
            assert_eq!(
                selection.candidate,
                PathBuf::from(r"C:\Program Files\Git\cmd\git.exe")
            );
            let current = std::env::current_dir().map_err(|_| GitDiscoveryError::Unavailable)?;
            let root = current
                .ancestors()
                .find(|path| path.join(".git").exists())
                .ok_or(GitDiscoveryError::Unavailable)?;
            crate::DesktopRepository::new(&selection.candidate, root).unwrap_or_else(|error| {
                panic!("DesktopRepository::new rejected selected Git: {error}")
            });
            Ok(())
        })();

        unsafe {
            match previous_override {
                Some(value) => std::env::set_var("RAH_GIT_EXECUTABLE", value),
                None => std::env::remove_var("RAH_GIT_EXECUTABLE"),
            }
        }
        result.expect("host Git for Windows resolver and DesktopRepository probe must pass");
    }
}
