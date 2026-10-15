//! Plugin configuration helpers.
//!
//! A plugin parses its configuration in [`Plugin::check_config`](crate::Plugin)
//! and again in `apply`, with the same function, so the kernel can refuse a
//! bad configuration before it stops a running instance (ADR 0044).

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Deserializes a plugin configuration. `null`, a plugin mounted without a
/// configuration, gives the default.
pub fn parse<T: DeserializeOwned + Default>(config: &Value) -> Result<T, serde_json::Error> {
    if config.is_null() {
        Ok(T::default())
    } else {
        T::deserialize(config)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Default, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Config {
        size: Option<u32>,
    }

    #[test]
    fn null_is_the_default_and_other_values_are_deserialized() {
        assert_eq!(
            super::parse::<Config>(&json!(null)).unwrap(),
            Config::default()
        );
        assert_eq!(
            super::parse::<Config>(&json!({"size": 3})).unwrap(),
            Config { size: Some(3) }
        );
        assert!(super::parse::<Config>(&json!({"sise": 3})).is_err());
    }
}
