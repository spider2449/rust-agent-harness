//! Events exchanged between plugins and the host process.

/// Emitted by a frontend to ask the host process to shut down and exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitRequested {
    pub code: i32,
}

/// Emitted by a frontend that listens for connections, so the host can show
/// where. `url` may carry an access secret; show it only to the local user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendListening {
    pub plugin: rah_kernel::PluginId,
    pub url: String,
}
