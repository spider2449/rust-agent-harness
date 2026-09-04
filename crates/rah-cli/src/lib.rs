//! Reusable host-side orchestration seams for the RAH command-line host.

/// Backward-compatible access to shared trusted-profile effective composition.
pub mod profile_composition {
    pub use rah_profile_composition::*;
}
