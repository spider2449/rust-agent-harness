//! One bounded recovery for a confirmed, side-effect-free exact edit rejection.

use std::collections::HashMap;

use rah_api::hooks::ToolCompletion;
use rah_protocol::{ContextItem, ModelRequest, ToolContent, ToolEffect, ToolOutput};
use serde_json::json;

use super::{
    CallOutcome, TaskProbe, frozen::Mode, live_settings, old_was_not_found, sample, tasks,
    workflow_paths,
};

const VERIFIED: &str = "rah_fixture_edit_rejected_without_effect";

pub(super) fn mark_rejection(mut completion: ToolCompletion) -> ToolCompletion {
    if completion.request.call.name.as_str() == "repo.edit-files"
        && completion.effect == ToolEffect::None
        && old_was_not_found(&completion.output)
    {
        completion
            .output
            .content
            .push(ToolContent::Json(json!({VERIFIED:true})));
    }
    completion
}

pub(super) fn is_verified_rejection(output: &ToolOutput) -> bool {
    old_was_not_found(output) && output.content.iter().any(|content| {
        matches!(content, ToolContent::Json(value) if value.get(VERIFIED) == Some(&serde_json::Value::Bool(true)))
    })
}

pub(super) fn continue_after_rejection(request: &mut ModelRequest, restrict_old: bool) -> bool {
    let calls: HashMap<_, _> = request
        .items
        .iter()
        .filter_map(|item| match item {
            ContextItem::ToolCall {
                call_id,
                name,
                arguments,
            } => Some((
                call_id.as_str(),
                (name.as_str(), workflow_paths(name.as_str(), arguments)?),
            )),
            _ => None,
        })
        .collect();
    let results: Vec<_> = request
        .items
        .iter()
        .filter_map(|item| match item {
            ContextItem::ToolResult { call_id, output } => Some((call_id.as_str(), output)),
            _ => None,
        })
        .collect();
    let [
        (_, listing),
        (read_id, initial),
        (edit_id, rejection),
        later @ ..,
    ] = results.as_slice()
    else {
        return false;
    };
    if listing.is_error || initial.is_error || !is_verified_rejection(rejection) {
        return false;
    }
    let (Some(("fs.read", paths)), Some(("repo.edit-files", edited))) =
        (calls.get(read_id), calls.get(edit_id))
    else {
        return false;
    };
    let [path] = paths.as_slice() else {
        return false;
    };
    if paths != edited {
        return false;
    }
    let latest_steps = ["fs.read", "repo.edit-files", "fs.read"];
    let valid = later.len() <= latest_steps.len()
        && later.iter().zip(latest_steps).all(|((id, output), tool)| {
            !output.is_error
                && calls
                    .get(id)
                    .is_some_and(|(name, paths)| *name == tool && paths == &[*path])
        });
    let goal = request
        .items
        .iter()
        .find_map(|item| match item {
            ContextItem::User { text } => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or_default();
    let exact_lines = if restrict_old && valid && later.len() == 1 {
        Some(exact_old_lines(later[0].1))
    } else {
        None
    };
    let (tool, prompt) = match (valid, later.len()) {
        (false, _) => (
            None,
            "Report which step failed. Do not try the edit again.".to_owned(),
        ),
        (true, 0 | 2) => (Some("fs.read"), format!("Call fs_read to read {path} now.")),
        (true, 1) if exact_lines.as_ref().is_some_and(Vec::is_empty) => (
            None,
            "No unique source lines are available for a precise retry. Report the unfinished edit."
                .into(),
        ),
        (true, 1) if exact_lines.is_some() => (
            Some("repo.edit-files"),
            format!(
                "{goal}\nChoose the exact old line from the allowed values. Replace it with the edited line and preserve all other fields. Submit one edit. Call repo_edit-files now."
            ),
        ),
        (true, 1) => (
            Some("repo.edit-files"),
            format!(
                "{goal}\nThe first edit was rejected before writing. Use the latest file read. Copy the exact old source substring including quotes and spaces. Submit one new replacement. Call repo_edit-files now."
            ),
        ),
        (true, _) => (
            None,
            "Summarize only the change confirmed by the last file read.".to_owned(),
        ),
    };
    let read_id = read_id.to_string();
    let old_edit_id = edit_id.to_string();
    let has_refresh = valid && !later.is_empty();
    if has_refresh {
        // Keep the original tool exchange in the session, but avoid asking the
        // model to copy either the superseded source or the rejected arguments.
        for item in &mut request.items {
            match item {
                ContextItem::ToolResult { call_id, output } if *call_id == read_id => {
                    output.content =
                        vec![ToolContent::Text("Use the newer read of this file.".into())];
                }
                ContextItem::ToolCall {
                    call_id, arguments, ..
                } if *call_id == old_edit_id => {
                    *arguments = json!({"previous_attempt":"rejected without writing"});
                }
                _ => {}
            }
        }
    }
    request
        .tools
        .retain(|definition| Some(definition.name.as_str()) == tool);
    if let Some(lines) = exact_lines {
        for definition in &mut request.tools {
            definition.input_schema["properties"]["edits"]["items"]["properties"]["old"] =
                json!({"type":"string","enum":lines});
        }
    }
    request.items.push(ContextItem::User { text: prompt });
    true
}

fn exact_old_lines(output: &ToolOutput) -> Vec<String> {
    let [ToolContent::Text(source)] = output.content.as_slice() else {
        return Vec::new();
    };
    source
        .lines()
        .filter(|line| {
            line.len() <= 512
                && line.trim().chars().count() >= 3
                && source.matches(line).count() == 1
        })
        .take(32)
        .map(str::to_owned)
        .collect()
}

pub(super) fn valid_retry_trace(trace: &[(&str, Vec<&str>, CallOutcome)]) -> bool {
    use CallOutcome::{RetryableOldMissing, Success};
    let [first, initial, rejected, refreshed, edited, verified] = trace else {
        return false;
    };
    first.0 == "fs.list"
        && first.1 == ["."]
        && first.2 == Success
        && initial.0 == "fs.read"
        && initial.1.len() == 1
        && initial.2 == Success
        && [rejected, refreshed, edited, verified]
            .into_iter()
            .zip([
                ("repo.edit-files", RetryableOldMissing),
                ("fs.read", Success),
                ("repo.edit-files", Success),
                ("fs.read", Success),
            ])
            .all(|((tool, paths, outcome), (expected, status))| {
                *tool == expected && *paths == initial.1 && *outcome == status
            })
}

#[test]
fn only_a_no_effect_exact_edit_rejection_can_enable_one_retry() {
    use rah_api::hooks::ToolRequestContext;
    use rah_protocol::{
        PermissionLevel, SessionId, ToolCall, ToolCallId, ToolDefinition, ToolInput, ToolName,
    };

    let tool = |name| ToolDefinition {
        name: ToolName::new(name),
        description: String::new(),
        input_schema: json!({}),
        permission: PermissionLevel::Write,
    };
    let error = ToolOutput {
        is_error: true,
        content: vec![ToolContent::Text("`old` was not found".into())],
    };
    let completion = |name, effect| ToolCompletion {
        effect,
        output: error.clone(),
        request: ToolRequestContext {
            session: SessionId::new(),
            definition: tool(name),
            call: ToolCall {
                id: ToolCallId::new(),
                name: ToolName::new(name),
                input: ToolInput(json!({})),
            },
        },
    };
    let eligible = mark_rejection(completion("repo.edit-files", ToolEffect::None)).output;
    assert!(is_verified_rejection(&eligible));
    for (name, effect) in [
        ("repo.edit-files", ToolEffect::Uncertain),
        ("fs.read", ToolEffect::None),
    ] {
        assert!(!is_verified_rejection(
            &mark_rejection(completion(name, effect)).output
        ));
    }
    let mut request = ModelRequest {
        items: vec![ContextItem::User {
            text: "Change the fixture limit.".into(),
        }],
        tools: vec![tool("fs.read"), tool("repo.edit-files")],
        ..ModelRequest::default()
    };
    let step = |request: &mut ModelRequest, id: &str, name, args, output| {
        request.items.extend([
            ContextItem::ToolCall {
                call_id: id.into(),
                name: ToolName::new(name),
                arguments: args,
            },
            ContextItem::ToolResult {
                call_id: id.into(),
                output,
            },
        ]);
    };
    let ok = ToolOutput {
        is_error: false,
        content: vec![],
    };
    step(&mut request, "ls", "fs.list", json!({}), ok.clone());
    step(
        &mut request,
        "read",
        "fs.read",
        json!({"path":"fixture.json"}),
        ok.clone(),
    );
    let args = json!({"edits":[{"path":"fixture.json","old":"wrong","new":"requested"}]});
    let mut uncertain = request.clone();
    step(
        &mut uncertain,
        "edit",
        "repo.edit-files",
        args.clone(),
        error,
    );
    assert!(!continue_after_rejection(&mut uncertain, false));
    step(
        &mut request,
        "edit",
        "repo.edit-files",
        args.clone(),
        eligible.clone(),
    );
    let mut reread = request.clone();
    assert!(continue_after_rejection(&mut reread, false));
    assert_eq!(reread.tools[0].name.as_str(), "fs.read");
    step(
        &mut request,
        "refresh",
        "fs.read",
        json!({"path":"fixture.json"}),
        ok,
    );
    let mut retry = request.clone();
    assert!(continue_after_rejection(&mut retry, false));
    assert_eq!(retry.tools[0].name.as_str(), "repo.edit-files");
    step(&mut request, "retry", "repo.edit-files", args, eligible);
    assert!(continue_after_rejection(&mut request, false));
    assert!(
        request.tools.is_empty(),
        "a second rejection ends the turn without more tools"
    );
}

#[test]
fn candidate_lines_are_verbatim_unique_substrings_of_the_latest_read() {
    let output = ToolOutput {
        is_error: false,
        content: vec![ToolContent::Text(
            "{\r\n  \"unique\": 8,\r\n  \"duplicate\": 1,\r\n  \"duplicate\": 1,\r\n}\r\n".into(),
        )],
    };
    assert_eq!(exact_old_lines(&output), ["  \"unique\": 8,"]);
}

#[tokio::test]
#[ignore = "requires an isolated llama.cpp with Qwen3-Coder template and the native tool choice proxy"]
async fn live_small_model_native_retry() {
    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model retry configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":"development"})
    );
    for task in tasks()
        .into_iter()
        .filter(|task| matches!(task.id.as_str(), "batch-limit" | "stale-edit"))
    {
        for (index, retry) in [None, Some(false), Some(true), Some(true), Some(false), None]
            .into_iter()
            .enumerate()
        {
            let probe = match retry {
                Some(restrict_old) => TaskProbe::NativeRetry { restrict_old },
                None => TaskProbe::NativeGrammar(Mode::EditOne),
            };
            let row = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
            eprintln!("small-model retry sample: {row}");
        }
    }
}
