use std::fmt;

use rah_api::tools::ToolError;
use serde::de::{Deserializer, Error, MapAccess, Visitor};
use serde_json::value::RawValue;

pub(super) fn pointer_tokens(pointer: &str) -> Result<Vec<String>, &'static str> {
    let Some(tail) = pointer.strip_prefix('/') else {
        return Err("`pointer` must be a non-root JSON Pointer beginning with `/`");
    };
    if pointer.len() > 1024 || pointer.bytes().filter(|&byte| byte == b'/').count() > 128 {
        return Err("`pointer` is too long");
    }
    tail.split('/')
        .map(|token| {
            let mut decoded = String::with_capacity(token.len());
            let mut chars = token.chars();
            while let Some(ch) = chars.next() {
                decoded.push(match ch {
                    '~' => match chars.next() {
                        Some('0') => '~',
                        Some('1') => '/',
                        _ => return Err("`pointer` only allows `~0` and `~1` escapes"),
                    },
                    ch => ch,
                });
            }
            Ok(decoded)
        })
        .collect()
}

pub(super) fn replace_integer(
    original: &str,
    tokens: &[String],
    value: u64,
) -> Result<String, ToolError> {
    let json = original.strip_prefix('\u{feff}').unwrap_or(original);
    let mut target: &RawValue = serde_json::from_str(json).map_err(invalid_json)?;
    for token in tokens {
        let child = match target.get().as_bytes().first() {
            Some(b'{') => serde_json::Deserializer::from_str(target.get())
                .deserialize_map(Member(token))
                .map_err(invalid_json)?,
            Some(b'[') => {
                let entries =
                    serde_json::from_str::<Vec<&RawValue>>(target.get()).map_err(invalid_json)?;
                array_index(token).and_then(|index| entries.get(index).copied())
            }
            _ => None,
        };
        target = child.ok_or_else(|| {
            ToolError::InvalidInput("the configured JSON pointer does not exist".into())
        })?;
    }
    let old = serde_json::from_str::<u64>(target.get()).map_err(|_| {
        ToolError::InvalidInput("the configured field is not an unsigned integer".into())
    })?;
    if old == value {
        return Err(ToolError::NotStarted(
            "the field already has that value".into(),
        ));
    }

    // Borrowed RawValue spans are slices of the deserializer's input, which
    // is either the original string or another borrowed span in that string.
    let start = target.get().as_ptr().addr() - original.as_ptr().addr();
    let mut replacement = original.to_owned();
    replacement.replace_range(start..start + target.get().len(), &value.to_string());
    Ok(replacement)
}

fn invalid_json(error: serde_json::Error) -> ToolError {
    ToolError::InvalidInput(error.to_string())
}

fn array_index(token: &str) -> Option<usize> {
    if token.bytes().all(|byte| byte.is_ascii_digit()) && (token == "0" || !token.starts_with('0'))
    {
        token.parse().ok()
    } else {
        None
    }
}

struct Member<'a>(&'a str);

impl<'de> Visitor<'de> for Member<'_> {
    type Value = Option<&'de RawValue>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object containing a unique matching key")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut matching = None;
        while let Some((key, value)) = map.next_entry::<String, &RawValue>()? {
            if key == self.0 && matching.replace(value).is_some() {
                return Err(M::Error::custom(
                    "the configured pointer matches a duplicate object key",
                ));
            }
        }
        Ok(matching)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_every_byte_except_the_selected_integer() {
        let source = concat!(
            "\u{feff}{\r\n  \"a/b~\": [ { \"\\u0078\" : 42, \"text\": \"42\" } ],\r\n",
            "  \"wide\": 18446744073709551616, \"repeat\": 42, \"repeat\": 42\r\n}\r\n "
        );
        let path = pointer_tokens("/a~1b~0/0/x").unwrap();
        assert_eq!(
            replace_integer(source, &path, u64::MAX).unwrap(),
            source.replacen(": 42", ": 18446744073709551615", 1)
        );
        assert!(matches!(
            replace_integer(source, &path, 42),
            Err(ToolError::NotStarted(_))
        ));
    }

    #[test]
    fn rejects_ambiguous_missing_and_non_integer_targets() {
        for (source, pointer) in [
            (r#"{"x":1,"\u0078":2}"#, "/x"),
            (r#"{"x":{"y":1},"x":{"y":2}}"#, "/x/y"),
            (r#"{"x":1.0}"#, "/x"),
            (r#"{"x":-1}"#, "/x"),
            (r#"{"x":18446744073709551616}"#, "/x"),
            (r#"{"x":true}"#, "/x"),
            (r#"{"x":1}"#, "/y"),
            ("[1]", "/00"),
            ("[1]", "/-"),
            ("[1]", "/1"),
            (r#"{"x":1} trailing"#, "/x"),
        ] {
            let path = pointer_tokens(pointer).unwrap();
            assert!(
                matches!(
                    replace_integer(source, &path, 8),
                    Err(ToolError::InvalidInput(_))
                ),
                "{source} {pointer}"
            );
        }
        for pointer in ["", "x", "/x~", "/x~2", "#/x"] {
            assert!(pointer_tokens(pointer).is_err(), "{pointer}");
        }
        assert_eq!(pointer_tokens("/").unwrap(), [""]);
    }
}
