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

/// Offered by the host on an interrupt from the user (Ctrl-C), before it
/// exits. A frontend that cancels running work in response returns
/// `Some(())`, and the host keeps running; without such an answer it exits.
pub struct Interrupt;

impl rah_kernel::BailHook for Interrupt {
    const NAME: &'static str = "host.interrupt";
    type Input = ();
    type Output = ();
}
