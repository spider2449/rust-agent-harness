use crate::CodexAdapterError;

const MAX_MODEL_LENGTH: usize = 256;
const MAX_URL_LENGTH: usize = 2_048;
const MAX_ENVIRONMENT_VARIABLE_LENGTH: usize = 128;

/// Immutable, host-owned model selection for a Codex runtime connection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum CodexModelConfig {
    /// Preserve the model and provider selected by the host Codex configuration.
    #[default]
    Inherit,
    /// Use the explicitly selected Codex model provider and model.
    Explicit(CodexModelSelection),
}

/// A validated model and provider pair for one Codex runtime connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexModelSelection {
    model: String,
    provider: CodexModelProvider,
}

impl CodexModelSelection {
    /// Creates an explicit model selection after validating the host-owned model name.
    pub fn new(
        model: impl Into<String>,
        provider: CodexModelProvider,
    ) -> Result<Self, CodexAdapterError> {
        let model = validate_model(model.into())?;
        Ok(Self { model, provider })
    }

    /// Returns the selected model exactly as it will be sent to Codex.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the selected provider.
    pub fn provider(&self) -> &CodexModelProvider {
        &self.provider
    }
}

/// Codex provider selection supported by the restricted RAH adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexModelProvider {
    OpenAi,
    Ollama,
    LmStudio,
    LlamaCpp(CodexLlamaCppProvider),
    Custom(CodexCustomProvider),
}

/// Validated configuration for RAH's Responses-only llama.cpp preset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexLlamaCppProvider {
    base_url: String,
    credential_environment_variable: Option<String>,
}

impl CodexLlamaCppProvider {
    /// The default llama.cpp OpenAI-compatible Responses endpoint.
    pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080/v1";

    /// Creates the default local llama.cpp provider without credentials.
    pub fn default_local() -> Self {
        Self {
            base_url: Self::DEFAULT_BASE_URL.to_owned(),
            credential_environment_variable: None,
        }
    }

    /// Creates a llama.cpp provider with an optional credential environment-variable name.
    pub fn new(
        base_url: impl Into<String>,
        credential_environment_variable: Option<String>,
    ) -> Result<Self, CodexAdapterError> {
        Ok(Self {
            base_url: validate_base_url(base_url.into())?,
            credential_environment_variable: credential_environment_variable
                .map(validate_environment_variable)
                .transpose()?,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    pub fn credential_environment_variable(&self) -> Option<&str> {
        self.credential_environment_variable.as_deref()
    }
}

impl Default for CodexLlamaCppProvider {
    fn default() -> Self {
        Self::default_local()
    }
}

/// Validated configuration for a host-owned Responses-only custom provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexCustomProvider {
    base_url: String,
    credential_environment_variable: Option<String>,
}

impl CodexCustomProvider {
    /// Creates a custom Responses provider with an optional credential environment-variable name.
    pub fn new(
        base_url: impl Into<String>,
        credential_environment_variable: Option<String>,
    ) -> Result<Self, CodexAdapterError> {
        Ok(Self {
            base_url: validate_base_url(base_url.into())?,
            credential_environment_variable: credential_environment_variable
                .map(validate_environment_variable)
                .transpose()?,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    pub fn credential_environment_variable(&self) -> Option<&str> {
        self.credential_environment_variable.as_deref()
    }
}

fn validate_model(value: String) -> Result<String, CodexAdapterError> {
    validate_text("model", value, MAX_MODEL_LENGTH, false)
}

fn validate_base_url(value: String) -> Result<String, CodexAdapterError> {
    let value = validate_text("base URL", value, MAX_URL_LENGTH, false)?;
    if value.starts_with("http://") || value.starts_with("https://") {
        Ok(value)
    } else {
        Err(invalid("base URL must start with http:// or https://"))
    }
}

fn validate_environment_variable(value: String) -> Result<String, CodexAdapterError> {
    let value = validate_text(
        "credential environment-variable name",
        value,
        MAX_ENVIRONMENT_VARIABLE_LENGTH,
        false,
    )?;
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(invalid("credential environment-variable name is empty"));
    };
    if !(first == '_' || first.is_ascii_alphabetic())
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(invalid(
            "credential environment-variable name is not a valid identifier",
        ));
    }
    Ok(value)
}

fn validate_text(
    field: &str,
    value: String,
    maximum_length: usize,
    allow_empty: bool,
) -> Result<String, CodexAdapterError> {
    let trimmed = value.trim();
    if (!allow_empty && trimmed.is_empty())
        || trimmed.len() > maximum_length
        || trimmed.contains('\0')
    {
        return Err(invalid(&format!("invalid {field}")));
    }
    Ok(trimmed.to_owned())
}

fn invalid(message: &str) -> CodexAdapterError {
    CodexAdapterError::InvalidModelProviderConfig {
        message: message.to_owned(),
    }
}
