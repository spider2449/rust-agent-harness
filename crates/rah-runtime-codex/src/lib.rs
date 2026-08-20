//! Optional, process-isolated Codex runtime adapter for RAH.

mod connection;
mod errors;
mod process;
mod protocol;
mod runtime;
mod transport;

#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod test_support;

pub use errors::CodexAdapterError;
pub use runtime::CodexRuntime;

/// Exact Codex CLI version supported by this adapter release.
pub const SUPPORTED_CODEX_VERSION: &str = "codex-cli 0.148.0";
