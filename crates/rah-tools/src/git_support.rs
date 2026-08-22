use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
};

use crate::ToolError;

pub(crate) fn git_environment() -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();
    insert_environment(&mut environment, "GIT_CONFIG_NOSYSTEM", "1");
    insert_environment(
        &mut environment,
        "GIT_CONFIG_GLOBAL",
        platform_null_device(),
    );
    insert_environment(&mut environment, "GIT_CONFIG_COUNT", "2");
    insert_environment(&mut environment, "GIT_CONFIG_KEY_0", "core.fsmonitor");
    insert_environment(&mut environment, "GIT_CONFIG_VALUE_0", "false");
    insert_environment(&mut environment, "GIT_CONFIG_KEY_1", "core.untrackedCache");
    insert_environment(&mut environment, "GIT_CONFIG_VALUE_1", "false");
    insert_environment(&mut environment, "GIT_OPTIONAL_LOCKS", "0");
    insert_environment(&mut environment, "GIT_TERMINAL_PROMPT", "0");
    environment
}

pub(crate) fn git_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "Git repository policy rejected capability: {}",
            message.into()
        ),
    }
}

fn insert_environment(
    environment: &mut BTreeMap<OsString, OsString>,
    name: impl AsRef<OsStr>,
    value: impl AsRef<OsStr>,
) {
    environment.insert(name.as_ref().to_owned(), value.as_ref().to_owned());
}

#[cfg(windows)]
fn platform_null_device() -> &'static str {
    "NUL"
}
#[cfg(not(windows))]
fn platform_null_device() -> &'static str {
    "/dev/null"
}
