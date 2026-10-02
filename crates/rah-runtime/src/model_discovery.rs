/// Dynamic advertisement only; neither entitlement nor inference readiness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCatalog {
    pub models: Vec<String>,
}

/// Conservative observation for the immutable host selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelPreflight {
    /// Inherit or a provider not covered by the runtime's default catalog.
    NotChecked,
    Advertised(ModelCatalog),
    NotAdvertised(ModelCatalog),
}
