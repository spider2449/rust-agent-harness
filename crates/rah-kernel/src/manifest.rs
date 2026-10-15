use std::{collections::BTreeMap, fmt};

use crate::KernelError;

/// Kernel API version. A plugin manifest must target exactly this version.
pub const KERNEL_API: u32 = 1;

macro_rules! name_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            fn is_valid(&self) -> bool {
                !self.0.is_empty()
                    && self.0.len() <= 128
                    && self
                        .0
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

name_type!(
    /// Unique identity of a mounted plugin, such as `model-llamacpp`.
    PluginId
);
name_type!(
    /// Name of a service contract, such as `model` or `tools`.
    ServiceName
);

/// Static description of a plugin: identity and service dependencies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: PluginId,
    pub version: String,
    pub kernel_api: u32,
    /// Services this plugin may register.
    pub provides: Vec<ServiceName>,
    /// Services that must resolve before the plugin starts.
    pub requires: Vec<ServiceName>,
    /// Services the plugin may inject if present.
    pub optional: Vec<ServiceName>,
}

impl PluginManifest {
    pub fn new(id: impl Into<PluginId>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            kernel_api: KERNEL_API,
            provides: Vec::new(),
            requires: Vec::new(),
            optional: Vec::new(),
        }
    }

    pub fn provides<I: IntoIterator<Item = S>, S: Into<ServiceName>>(mut self, names: I) -> Self {
        self.provides.extend(names.into_iter().map(Into::into));
        self
    }

    pub fn requires<I: IntoIterator<Item = S>, S: Into<ServiceName>>(mut self, names: I) -> Self {
        self.requires.extend(names.into_iter().map(Into::into));
        self
    }

    pub fn optional<I: IntoIterator<Item = S>, S: Into<ServiceName>>(mut self, names: I) -> Self {
        self.optional.extend(names.into_iter().map(Into::into));
        self
    }

    pub(crate) fn may_inject(&self, service: &ServiceName) -> bool {
        self.requires.contains(service) || self.optional.contains(service)
    }

    pub(crate) fn validate(&self, options: &MountOptions) -> Result<(), KernelError> {
        let invalid = |reason: String| KernelError::InvalidManifest {
            id: self.id.clone(),
            reason,
        };
        if !self.id.is_valid() {
            return Err(invalid("plugin id must be 1-128 of [A-Za-z0-9._:-]".into()));
        }
        if self.kernel_api != KERNEL_API {
            return Err(KernelError::IncompatibleApi {
                id: self.id.clone(),
                found: self.kernel_api,
                expected: KERNEL_API,
            });
        }
        for (list, names) in [
            ("provides", &self.provides),
            ("requires", &self.requires),
            ("optional", &self.optional),
        ] {
            for (index, name) in names.iter().enumerate() {
                if !name.is_valid() {
                    return Err(invalid(format!("invalid service name `{name}` in {list}")));
                }
                if names[..index].contains(name) {
                    return Err(invalid(format!("duplicate service `{name}` in {list}")));
                }
            }
        }
        if let Some(name) = self.requires.iter().find(|n| self.optional.contains(n)) {
            return Err(invalid(format!("`{name}` is both required and optional")));
        }
        if let Some(name) = options.bindings.keys().find(|n| !self.may_inject(n)) {
            return Err(invalid(format!(
                "binding for undeclared dependency `{name}`"
            )));
        }
        Ok(())
    }
}

/// Per-mount options: configuration and explicit provider selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MountOptions {
    /// Plugin configuration, available as [`crate::Scope::config`].
    pub config: serde_json::Value,
    /// Selects a specific provider instance for a dependency. Without a
    /// binding, the earliest-registered instance of the service is used.
    pub bindings: BTreeMap<ServiceName, String>,
}

impl MountOptions {
    pub fn with_config(config: serde_json::Value) -> Self {
        Self {
            config,
            bindings: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, service: impl Into<ServiceName>, instance: impl Into<String>) -> Self {
        self.bindings.insert(service.into(), instance.into());
        self
    }
}

/// Lifecycle state of a mounted plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginStatus {
    /// Waiting for the listed required services.
    Pending {
        missing: Vec<ServiceName>,
    },
    /// Waiting on services that only plugins in this cycle could provide.
    DependencyCycle {
        cycle: Vec<PluginId>,
    },
    Starting,
    Active,
    Stopping,
    /// `apply` failed or panicked. Use `restart` or `update_config` to retry.
    Failed {
        message: String,
    },
}
