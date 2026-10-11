//! Events exchanged between plugins and the host process.

/// Emitted by a frontend to ask the host process to shut down and exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitRequested {
    pub code: i32,
}
