//! Optional, process-isolated Codex runtime adapter for RAH.

mod bridge;
mod catalog;
mod connection;
mod errors;
pub mod experimental;
mod model_config;
mod process;
mod protocol;
mod runtime;
mod transport;

#[cfg(test)]
mod bridge_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod test_support;

pub use errors::CodexAdapterError;
pub use model_config::{
    CodexCustomProvider, CodexLlamaCppProvider, CodexModelConfig, CodexModelProvider,
    CodexModelSelection,
};
pub use runtime::CodexRuntime;

/// Exact Codex CLI versions certified for the current adapter source.
pub const CURRENT_CERTIFIED_CODEX_VERSIONS: &[&str] = &["codex-cli 0.157.1"];

/// Deterministic default from the current certified set.
pub const PREFERRED_CURRENT_CODEX_VERSION: &str = "codex-cli 0.157.1";

/// Checks exact current admission; baseline storage and release history grant no admission.
pub fn is_current_certified_codex_version(version: &str) -> bool {
    CURRENT_CERTIFIED_CODEX_VERSIONS.contains(&version)
}
