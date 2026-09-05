//! Shared structural assertions for opt-in live-gate examples.
//!
//! These helpers deliberately accept no model text. Live-gate success is
//! determined by host-observed lifecycle and postcondition evidence.

#![allow(dead_code)]

use serde_json::Value;
use std::{fs, path::PathBuf};

pub const CERTIFICATION_TOKEN_PREFIX: &str = "RAH_LIVE_TOOL_TOKEN_";
pub const CERTIFICATION_TOKEN_MAX_LENGTH: usize = 128;

#[allow(dead_code)]
pub struct CopiedFixture {
    directory: PathBuf,
    executable: PathBuf,
    lifecycle: PathBuf,
}

#[allow(dead_code)]
impl CopiedFixture {
    pub fn create(source: &std::path::Path, label: &str) -> Result<Self, String> {
        let directory = std::env::temp_dir().join(format!(
            "rah-live-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&directory).map_err(|error| error.to_string())?;
        let executable = directory.join(format!("fixture{}", std::env::consts::EXE_SUFFIX));
        if let Err(error) = fs::copy(source, &executable) {
            let _ = fs::remove_dir_all(&directory);
            return Err(format!("failed to copy live fixture: {error}"));
        }
        let request = executable.with_extension("lifecycle-request");
        fs::write(&request, b"observe").map_err(|error| error.to_string())?;
        Ok(Self {
            lifecycle: request.with_extension("lifecycle"),
            directory,
            executable,
        })
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    /// Checks provider lifecycle evidence and proves Windows child handles are released.
    pub fn finish(mut self) -> Result<u64, String> {
        let lifecycle = fs::read_to_string(&self.lifecycle).unwrap_or_default();
        let events = lifecycle.lines().collect::<Vec<_>>();
        let call_count = events.iter().filter(|event| **event == "call").count() as u64;
        if !events.contains(&"spawn") || !events.contains(&"shutdown") || !events.contains(&"exit")
        {
            return Err(format!(
                "provider lifecycle audit was incomplete: {events:?}"
            ));
        }
        let released = self
            .executable
            .with_file_name(format!("fixture-released{}", std::env::consts::EXE_SUFFIX));
        fs::rename(&self.executable, &released)
            .map_err(|error| format!("provider executable remained locked: {error}"))?;
        fs::remove_file(&released).map_err(|error| error.to_string())?;
        fs::remove_dir_all(&self.directory).map_err(|error| error.to_string())?;
        self.directory = PathBuf::new();
        Ok(call_count)
    }
}

impl Drop for CopiedFixture {
    fn drop(&mut self) {
        if !self.directory.as_os_str().is_empty() {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum ProofEvent {
    ToolRequested { name: String, arguments: Value },
    ToolStarted,
    ToolFinished { is_error: bool, output_text: String },
    ModelDelta,
    Completed,
    Failed,
}

/// Creates the nonce used by one live certification run.
#[allow(dead_code)]
pub fn certification_token() -> String {
    format!(
        "{CERTIFICATION_TOKEN_PREFIX}{}",
        uuid::Uuid::new_v4().simple()
    )
}

/// Validates the bounded token contract shared by the live harness and fixtures.
pub fn validate_certification_token(token: &str) -> Result<(), String> {
    let Some(suffix) = token.strip_prefix(CERTIFICATION_TOKEN_PREFIX) else {
        return Err("certification token has an invalid prefix".to_owned());
    };
    if token.is_empty() || token.len() > CERTIFICATION_TOKEN_MAX_LENGTH || suffix.len() != 32 {
        return Err("certification token is empty, oversized, or the wrong length".to_owned());
    }
    if !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("certification token contains a non-hex nonce".to_owned());
    }
    Ok(())
}

/// Verifies the complete host-observed proof for one external provider.
pub fn verify_tool_proof(
    subject: &str,
    expected_tool: &str,
    expected_arguments: &Value,
    expected_token: &str,
    events: &[ProofEvent],
    provider_execution_count: u64,
) -> Result<(), String> {
    validate_certification_token(expected_token)?;
    if provider_execution_count != 1 {
        return Err(format!(
            "{subject} provider execution count was {provider_execution_count}; expected exactly one"
        ));
    }

    let mut requested = 0_usize;
    let mut started = 0_usize;
    let mut finished = 0_usize;
    let mut finish_index = None;
    let mut continuation_index = None;

    for (index, event) in events.iter().enumerate() {
        match event {
            ProofEvent::ToolRequested { name, arguments } => {
                requested += 1;
                if name != expected_tool {
                    return Err(format!(
                        "{subject} exposed `{name}` as public certification identity; expected `{expected_tool}`"
                    ));
                }
                if arguments != expected_arguments {
                    return Err(format!(
                        "{subject} Tool arguments were {arguments}; expected {expected_arguments}"
                    ));
                }
            }
            ProofEvent::ToolStarted => started += 1,
            ProofEvent::ToolFinished {
                is_error,
                output_text,
            } => {
                finished += 1;
                finish_index = Some(index);
                if *is_error {
                    return Err(format!("{subject} ToolFinished reported an error"));
                }
                if !output_text.contains(expected_token) {
                    return Err(format!(
                        "{subject} ToolFinished output did not contain the hidden certification token"
                    ));
                }
            }
            ProofEvent::ModelDelta if finish_index.is_some() => {
                continuation_index.get_or_insert(index);
            }
            ProofEvent::Failed => return Err(format!("{subject} turn failed")),
            ProofEvent::ModelDelta | ProofEvent::Completed => {}
        }
    }

    require_exactly_once(subject, requested, started, finished)?;
    let finish_index = finish_index.ok_or_else(|| format!("{subject} is missing ToolFinished"))?;
    let continuation_index = continuation_index
        .ok_or_else(|| format!("{subject} has no continuation after ToolFinished"))?;
    if continuation_index <= finish_index {
        return Err(format!(
            "{subject} continuation did not follow ToolFinished"
        ));
    }
    if events.last() != Some(&ProofEvent::Completed) {
        return Err(format!("{subject} did not terminate with Completed"));
    }
    if events
        .iter()
        .position(|event| matches!(event, ProofEvent::Completed))
        .is_some_and(|index| index <= continuation_index)
    {
        return Err(format!("{subject} Completed preceded its continuation"));
    }
    Ok(())
}

/// Ensures a per-run token is absent from every model-visible definition.
pub fn require_token_hidden(
    token: &str,
    prompt: &str,
    description: &str,
    schema: &Value,
    metadata: &Value,
) -> Result<(), String> {
    let schema = serde_json::to_string(schema).map_err(|error| error.to_string())?;
    let metadata = serde_json::to_string(metadata).map_err(|error| error.to_string())?;
    for (label, value) in [
        ("prompt", prompt.to_owned()),
        ("description", description.to_owned()),
        ("schema", schema),
        ("provider metadata", metadata),
    ] {
        if value.contains(token) {
            return Err(format!("certification token entered model-visible {label}"));
        }
    }
    Ok(())
}

/// Requires one requested, started, and finished lifecycle event.
pub fn require_exactly_once(
    subject: &str,
    requested: usize,
    started: usize,
    finished: usize,
) -> Result<(), String> {
    if requested == 1 && started == 1 && finished == 1 {
        Ok(())
    } else {
        Err(format!(
            "{subject} lifecycle was requested={requested}, started={started}, finished={finished}; expected exactly one of each"
        ))
    }
}

/// Requires the RAH event stream to have reached a terminal completion.
pub fn require_completed<T: AsRef<str>>(sequence: &[T]) -> Result<(), String> {
    if sequence
        .last()
        .is_some_and(|event| event.as_ref() == "Completed")
    {
        Ok(())
    } else {
        Err(format!(
            "turn did not terminate with Completed: {}",
            sequence
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(" -> ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn token() -> String {
        "RAH_LIVE_TOOL_TOKEN_0123456789abcdef0123456789abcdef".to_owned()
    }

    fn success_events() -> Vec<ProofEvent> {
        vec![
            ProofEvent::ToolRequested {
                name: "mcp.test.certify".to_owned(),
                arguments: json!({"request": "certification-token"}),
            },
            ProofEvent::ToolStarted,
            ProofEvent::ToolFinished {
                is_error: false,
                output_text: format!("provider result {}", token()),
            },
            ProofEvent::ModelDelta,
            ProofEvent::Completed,
        ]
    }

    #[test]
    fn token_is_bounded_and_malformed_or_oversized_tokens_fail_closed() {
        validate_certification_token(&token()).expect("valid token");
        assert!(validate_certification_token("").is_err());
        assert!(validate_certification_token("known-marker").is_err());
        assert!(
            validate_certification_token(&format!(
                "{CERTIFICATION_TOKEN_PREFIX}{}",
                "a".repeat(33)
            ))
            .is_err()
        );
        assert!(
            validate_certification_token(&format!(
                "{CERTIFICATION_TOKEN_PREFIX}{}",
                "z".repeat(32)
            ))
            .is_err()
        );
        assert!(
            validate_certification_token(&format!(
                "{CERTIFICATION_TOKEN_PREFIX}{}",
                "a".repeat(CERTIFICATION_TOKEN_MAX_LENGTH)
            ))
            .is_err()
        );
    }

    #[test]
    fn token_does_not_enter_prompt_description_schema_or_metadata() {
        let token = token();
        require_token_hidden(
            &token,
            "Request the certification result.",
            "Returns the per-run certification result.",
            &json!({"type":"object","properties":{"request":{"enum":["certification-token"]}}}),
            &json!({"fixture":true}),
        )
        .expect("token must remain hidden before execution");
        assert!(
            require_token_hidden(
                &token,
                &format!("leak {token}"),
                "description",
                &json!({}),
                &json!({}),
            )
            .is_err()
        );
    }

    #[test]
    fn complete_proof_requires_provider_execution_and_model_continuation() {
        verify_tool_proof(
            "MCP",
            "mcp.test.certify",
            &json!({"request":"certification-token"}),
            &token(),
            &success_events(),
            1,
        )
        .expect("complete proof");
    }

    #[test]
    fn final_marker_without_requested_tool_fails() {
        let events = vec![ProofEvent::Completed];
        assert!(
            verify_tool_proof("MCP", "mcp.test.certify", &json!({}), &token(), &events, 0).is_err()
        );
    }

    #[test]
    fn wrong_identity_and_arguments_fail() {
        let mut events = success_events();
        events[0] = ProofEvent::ToolRequested {
            name: "rah_tool_0".to_owned(),
            arguments: json!({"request":"certification-token"}),
        };
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &events,
                1
            )
            .is_err()
        );

        let mut events = success_events();
        events[0] = ProofEvent::ToolRequested {
            name: "mcp.test.certify".to_owned(),
            arguments: json!({"request":"wrong"}),
        };
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &events,
                1
            )
            .is_err()
        );
    }

    #[test]
    fn every_required_lifecycle_failure_is_rejected() {
        let missing_started = success_events()
            .into_iter()
            .filter(|event| !matches!(event, ProofEvent::ToolStarted))
            .collect::<Vec<_>>();
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &missing_started,
                1
            )
            .is_err()
        );
        let missing_finished = success_events()
            .into_iter()
            .filter(|event| !matches!(event, ProofEvent::ToolFinished { .. }))
            .collect::<Vec<_>>();
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &missing_finished,
                1
            )
            .is_err()
        );
        let mut error = success_events();
        error[2] = ProofEvent::ToolFinished {
            is_error: true,
            output_text: token(),
        };
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &error,
                1
            )
            .is_err()
        );
    }

    #[test]
    fn duplicate_calls_wrong_provider_count_missing_continuation_and_nonterminal_completion_fail() {
        let mut duplicate = success_events();
        duplicate.insert(1, duplicate[0].clone());
        assert!(
            verify_tool_proof(
                "Plugin",
                "plugin.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &duplicate,
                1
            )
            .is_err()
        );

        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &success_events(),
                2
            )
            .is_err()
        );

        let mut no_continuation = success_events();
        no_continuation.retain(|event| !matches!(event, ProofEvent::ModelDelta));
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &no_continuation,
                1
            )
            .is_err()
        );

        let mut nonterminal = success_events();
        nonterminal.push(ProofEvent::ModelDelta);
        assert!(
            verify_tool_proof(
                "MCP",
                "mcp.test.certify",
                &json!({"request":"certification-token"}),
                &token(),
                &nonterminal,
                1
            )
            .is_err()
        );
    }
}
