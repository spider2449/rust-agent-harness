//! The ignored runner uses RAH_SMALL_MODEL (served ID), RAH_SMALL_MODEL_RUN
//! (backend, quantization, context and thinking settings), and optionally
//! RAH_SMALL_MODEL_PORT (an explicit test port) and RAH_SMALL_MODEL_SET
//! (development or holdout).
//! Each group reserves its third fixture for the holdout run.

#[path = "frozen.rs"]
mod frozen;
#[path = "json_preflight.rs"]
mod json_preflight;
#[path = "json_value.rs"]
mod json_value;
#[path = "json_value_generalization.rs"]
mod json_value_generalization;
#[path = "native_retry.rs"]
mod native_retry;
#[path = "write_guard.rs"]
mod write_guard;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures::StreamExt;
use rah_api::{
    CancellationToken, MountOptions, Plugin, PluginError, PluginManifest, Scope,
    agent_loop::{LOOP, TurnOrigin, TurnRequest},
    async_trait,
    hooks::{ModelBeforeRequest, ToolAfterInvoke},
};
use rah_protocol::{AgentEvent, ContextItem, ModelRequest, ModelUsage, ToolContent, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    LOOPS,
    common::{TempDir, headless, kernel, new_session},
};

const GROUPS: [&str; 4] = ["read", "single", "cross", "recovery"];
const READ_TOOLS: [&str; 4] = ["fs.read", "fs.list", "fs.glob", "fs.grep"];
const WAIT: Duration = Duration::from_secs(120);
const RUST_PROBE_PATH: &str = "limits.rs";

#[derive(Deserialize)]
struct Task {
    id: String,
    group: String,
    prompt: String,
    files: BTreeMap<String, String>,
    changes: BTreeMap<String, String>,
    answer: Option<Value>,
    #[serde(default)]
    evidence: Vec<String>,
}

fn tasks() -> Vec<Task> {
    serde_json::from_str(include_str!("../fixtures/small_model_tasks.json")).unwrap()
}

fn seed(root: &Path, files: &BTreeMap<String, String>) {
    for (path, content) in files {
        fs::write(root.join(path), content).unwrap();
    }
}

fn matches_change(path: &str, actual: &[u8], expected: &str) -> bool {
    if path.ends_with(".json") {
        return serde_json::from_slice::<Value>(actual)
            .is_ok_and(|actual| actual == serde_json::from_str::<Value>(expected).unwrap());
    }
    // The Rust fixture requests a literal-only fix. Its contents are never
    // compiled or executed when grading a model-produced candidate.
    String::from_utf8_lossy(actual)
        .replace("\r\n", "\n")
        .trim_end()
        == expected.trim_end()
}

fn file_failures(task: &Task, root: &Path) -> (Vec<String>, Vec<String>) {
    let mut unexpected: BTreeSet<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|path| !task.files.contains_key(path))
        .collect();
    let mut incorrect = Vec::new();
    for (path, original) in &task.files {
        let target = root.join(path);
        let actual = target
            .symlink_metadata()
            .ok()
            .filter(|metadata| metadata.is_file())
            .and_then(|_| fs::read(&target).ok());
        if let Some(expected) = task.changes.get(path) {
            if !actual.is_some_and(|actual| matches_change(path, &actual, expected)) {
                incorrect.push(path.clone());
            }
        } else if actual.as_deref() != Some(original.as_bytes()) {
            unexpected.insert(path.clone());
        }
    }
    (incorrect, unexpected.into_iter().collect())
}

fn answer_matches(task: &Task, answer: &str) -> bool {
    task.answer.as_ref().is_none_or(|expected| {
        serde_json::from_str::<Value>(answer.trim()).is_ok_and(|actual| &actual == expected)
    })
}

fn allowed(task: &Task, tool: &str, edit_tool: &str) -> bool {
    READ_TOOLS.contains(&tool) || (!task.changes.is_empty() && tool == edit_tool)
}

fn model_usage(events: &[AgentEvent]) -> (usize, Option<u64>, Option<u64>) {
    let usage: HashMap<_, _> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ModelUsage {
                model_request_id,
                usage,
                ..
            } => Some((model_request_id, usage)),
            _ => None,
        })
        .collect();
    let requests: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ModelRequestStarted {
                model_request_id, ..
            } => Some(model_request_id),
            _ => None,
        })
        .collect();
    let sum = |field: fn(&ModelUsage) -> Option<u64>| {
        if requests.is_empty() {
            return None;
        }
        requests
            .iter()
            .try_fold(0u64, |total, id| total.checked_add(field(usage.get(*id)?)?))
    };
    (
        requests.len(),
        sum(|usage| usage.input_tokens),
        sum(|usage| usage.output_tokens),
    )
}

fn grade(
    task: &Task,
    root: &Path,
    events: &[AgentEvent],
    timed_out: bool,
    edit_tool: &str,
) -> Value {
    let (incorrect, unexpected) = file_failures(task, root);
    let completed = match events.last() {
        Some(AgentEvent::Completed { output, .. }) if !timed_out => {
            Some(output.message.content.as_str())
        }
        _ => None,
    };
    let evidence_events = events.iter().take_while(|event| {
        !matches!(event, AgentEvent::ToolRequested { tool_call, .. }
            if tool_call.name.as_str() == edit_tool)
    });
    let successful: HashSet<_> = evidence_events
        .filter_map(|event| match event {
            AgentEvent::ToolFinished {
                tool_call_id,
                output,
                ..
            } if !output.is_error => Some(tool_call_id),
            _ => None,
        })
        .collect();
    let calls: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolRequested { tool_call, .. } => Some(tool_call),
            _ => None,
        })
        .collect();
    let missing_evidence: Vec<_> = task
        .evidence
        .iter()
        .filter(|path| {
            !calls.iter().any(|call| {
                call.name.as_str() == "fs.read"
                    && call.input.0["path"].as_str() == Some(path.as_str())
                    && successful.contains(&call.id)
            })
        })
        .collect();
    let disallowed: BTreeSet<_> = calls
        .iter()
        .map(|call| call.name.as_str())
        .filter(|tool| !allowed(task, tool, edit_tool))
        .collect();
    let answer_ok = completed.is_some_and(|answer| answer_matches(task, answer));
    let (requests, input, output) = model_usage(events);
    let failure = match events.last() {
        Some(AgentEvent::Failed { code, message, .. }) => json!({"code": code, "message": message}),
        Some(AgentEvent::Cancelled { .. }) => json!({"code":"cancelled"}),
        _ => Value::Null,
    };
    json!({
        "passed": answer_ok && incorrect.is_empty() && unexpected.is_empty()
            && missing_evidence.is_empty() && disallowed.is_empty(),
        "completed": completed.is_some(), "timed_out": timed_out, "failure": failure,
        "answer_ok": answer_ok, "incorrect_paths": incorrect, "unexpected_paths": unexpected,
        "missing_evidence": missing_evidence, "disallowed_tools": disallowed,
        "observed_model_requests": requests, "observed_tool_requests": calls.len(),
        "tools_requested": calls.iter().map(|call| call.name.as_str()).collect::<Vec<_>>(),
        "observed_tool_errors": events.iter().filter(|event| matches!(event,
            AgentEvent::ToolFinished { output, .. } if output.is_error)).count(),
        "input_tokens": input.filter(|_| !timed_out), "output_tokens": output.filter(|_| !timed_out),
        "answer_excerpt": completed.map(|answer| answer.chars().take(500).collect::<String>()),
    })
}

#[test]
fn every_task_accepts_its_reference_and_rejects_unrelated_changes() {
    let tasks = tasks();
    let mut ids = BTreeSet::new();
    assert_eq!(tasks.len(), 12);
    for group in GROUPS {
        assert_eq!(tasks.iter().filter(|task| task.group == group).count(), 3);
    }
    for task in tasks {
        assert!(ids.insert(task.id.clone()), "duplicate task {}", task.id);
        assert!(
            task.changes
                .keys()
                .all(|path| task.files.contains_key(path))
        );
        assert!(
            task.evidence
                .iter()
                .all(|path| task.files.contains_key(path))
        );
        let dir = TempDir::new(&format!("small-model-reference-{}", task.id));
        let root = dir.workspace();
        seed(&root, &task.files);
        let (incorrect, unexpected) = file_failures(&task, &root);
        assert_eq!(incorrect, task.changes.keys().cloned().collect::<Vec<_>>());
        assert!(unexpected.is_empty());
        if let Some(expected) = &task.answer {
            assert!(answer_matches(&task, &expected.to_string()));
            assert!(!answer_matches(&task, "{}"));
        }
        seed(&root, &task.changes);
        assert_eq!(file_failures(&task, &root), (vec![], vec![]));
        fs::write(root.join("extra.txt"), "unrelated").unwrap();
        assert_eq!(
            file_failures(&task, &root),
            (vec![], vec!["extra.txt".into()])
        );
    }
}

#[test]
fn rust_recovery_fixture_has_the_reported_error_and_a_compiling_reference() {
    let task = tasks()
        .into_iter()
        .find(|task| task.id == "rust-type-mismatch")
        .unwrap();
    let dir = TempDir::new("small-model-rust-fixture");
    let root = dir.workspace();
    seed(&root, &task.files);
    let original = rust_metadata(&root, &dir.0);
    let diagnostic = rust_type_diagnostic(&original);
    assert!(diagnostic.contains("limits.rs:1:"), "{diagnostic}");
    let expected = &task.changes[RUST_PROBE_PATH];
    let proposed = b"const OTHER: &str = include_str!(\"unrelated.txt\");";
    assert!(compile_reference_edit(&root, &dir.0, proposed, expected).is_none());
    assert_eq!(
        fs::read_to_string(root.join(RUST_PROBE_PATH)).unwrap(),
        task.files[RUST_PROBE_PATH]
    );
    let fixed = compile_reference_edit(&root, &dir.0, expected.as_bytes(), expected).unwrap();
    assert!(
        fixed.status.success(),
        "{}",
        String::from_utf8_lossy(&fixed.stderr)
    );
}

fn rust_metadata(workspace: &Path, artifact_dir: &Path) -> Output {
    Command::new(option_env!("RUSTC").unwrap_or("rustc"))
        .args(["--crate-type=lib", "--emit=metadata", RUST_PROBE_PATH, "-o"])
        .arg(artifact_dir.join("limits.rmeta"))
        .current_dir(workspace)
        .output()
        .unwrap()
}

fn rust_type_diagnostic(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() && stderr.contains("error[E0308]"),
        "{stderr}"
    );
    stderr
        .lines()
        .take_while(|line| !line.starts_with("error: aborting"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}

fn compile_reference_edit(
    workspace: &Path,
    artifacts: &Path,
    candidate: &[u8],
    expected: &str,
) -> Option<Output> {
    // The compiler sees only matching reference-shaped source, copied to a
    // separate workspace. Free-form model-authored Rust is never compiled.
    matches_change(RUST_PROBE_PATH, candidate, expected).then(|| {
        fs::write(workspace.join(RUST_PROBE_PATH), candidate).unwrap();
        rust_metadata(workspace, artifacts)
    })
}

#[test]
fn a_correct_read_answer_requires_a_successful_tool_read() {
    use rah_protocol::{
        AgentOutput, Message, MessageRole, SessionId, ToolCall, ToolCallId, ToolInput, ToolName,
        ToolOutput,
    };

    let task = tasks().remove(0);
    let dir = TempDir::new("small-model-read-evidence");
    let root = dir.workspace();
    seed(&root, &task.files);
    let session_id = SessionId::new();
    let tool_call_id = ToolCallId::new();
    let requested = AgentEvent::ToolRequested {
        session_id: session_id.clone(),
        tool_call: ToolCall {
            id: tool_call_id.clone(),
            name: ToolName::new("fs.read"),
            input: ToolInput(json!({ "path": "service.json" })),
        },
    };
    let completed = AgentEvent::Completed {
        session_id: session_id.clone(),
        output: AgentOutput {
            message: Message {
                role: MessageRole::Assistant,
                content: task.answer.as_ref().unwrap().to_string(),
            },
        },
    };
    let finish = |is_error| AgentEvent::ToolFinished {
        session_id: session_id.clone(),
        tool_call_id: tool_call_id.clone(),
        output: ToolOutput {
            content: vec![],
            is_error,
        },
    };
    let failed = grade(
        &task,
        &root,
        &[requested.clone(), finish(true), completed.clone()],
        false,
        "repo.edit-files",
    );
    assert_eq!(failed["missing_evidence"], json!(["service.json"]));
    assert_eq!(failed["passed"], false);
    let successful = [requested, finish(false), completed];
    assert_eq!(
        grade(&task, &root, &successful, false, "repo.edit-files")["passed"],
        true
    );
    assert_eq!(
        grade(&task, &root, &successful, true, "repo.edit-files")["passed"],
        false
    );
}

#[derive(Clone, Copy)]
enum TaskProbe {
    SingleTool,
    AssistantPrefix,
    AssistantPrefixWithLineNumbers,
    LocateConfig,
    CompareFiles,
    EditFile {
        path: &'static str,
        goal: &'static str,
    },
    EditPair {
        first: &'static str,
        second: &'static str,
        goal: &'static str,
    },
    RecoverStaleEdit,
    Frozen(frozen::Mode),
    NativeGrammar(frozen::Mode),
    NativeRetry {
        restrict_old: bool,
    },
    NativeRetryJson,
    NativeJsonValue,
}

type ExpectedCall<'a> = (&'static str, Vec<&'a str>);
const QUEUE_PATH: &str = "queue.json";
const QUEUE_BEFORE: &str =
    "{\n  \"consumer\": {\"limit\": 4},\n  \"producer\": {\"limit\": 6}\n}\n";
const QUEUE_AFTER_EXTERNAL_EDIT: &str =
    "{\n  \"consumer\": {\"limit\": 5},\n  \"producer\": {\"limit\": 4}\n}\n";

fn old_was_not_found(output: &ToolOutput) -> bool {
    output.is_error && output.content.iter().any(|content| {
        matches!(content, ToolContent::Text(text) if text.contains("`old` was not found"))
    })
}

fn number_lines(output: &mut rah_protocol::ToolOutput) {
    if output.is_error {
        return;
    }
    for content in &mut output.content {
        if let ToolContent::Text(text) = content {
            *text = text
                .lines()
                .enumerate()
                .map(|(index, line)| format!("{}: {line}", index + 1))
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
}

impl TaskProbe {
    fn name(self) -> &'static str {
        match self {
            Self::SingleTool => "single_tool",
            Self::AssistantPrefix => "assistant_prefix",
            Self::AssistantPrefixWithLineNumbers => "assistant_prefix_numbered",
            Self::LocateConfig => "locate_config",
            Self::CompareFiles => "compare_files",
            Self::EditFile { .. } => "edit_file",
            Self::EditPair { .. } => "edit_pair",
            Self::RecoverStaleEdit => "recover_stale_edit",
            Self::Frozen(_) => "frozen_staged",
            Self::NativeGrammar(_) => "native_grammar",
            Self::NativeRetry {
                restrict_old: false,
            } => "native_retry",
            Self::NativeRetry { restrict_old: true } => "native_retry_exact_lines",
            Self::NativeRetryJson => "native_retry_json_guard",
            Self::NativeJsonValue => "native_json_value",
        }
    }

    fn shape(self, mut request: ModelRequest) -> ModelRequest {
        if let Self::Frozen(mode) = self {
            return mode.shape(request, false, "repo.edit-files");
        }
        if let Self::NativeGrammar(mode) = self {
            return mode.shape(request, true, "repo.edit-files");
        }
        if matches!(self, Self::NativeJsonValue) {
            return frozen::Mode::EditOne.shape(request, true, json_value::NAME);
        }
        if matches!(self, Self::NativeRetry { .. } | Self::NativeRetryJson) {
            let restrict_old = !matches!(
                self,
                Self::NativeRetry {
                    restrict_old: false
                }
            );
            if native_retry::continue_after_rejection(&mut request, restrict_old) {
                return request;
            }
            return frozen::Mode::EditOne.shape(request, true, "repo.edit-files");
        }
        if matches!(
            self,
            Self::LocateConfig
                | Self::CompareFiles
                | Self::EditFile { .. }
                | Self::EditPair { .. }
                | Self::RecoverStaleEdit
        ) {
            return self.shape_workflow(request);
        }
        if let Some(ContextItem::ToolResult { output, .. }) = request
            .items
            .iter_mut()
            .rev()
            .find(|item| matches!(item, ContextItem::ToolResult { .. }))
        {
            request.tools.clear();
            let mut text = "From the file just read, reply with raw JSON containing workers, path and the 1-based line for workers. Do not use Markdown fences.".to_owned();
            if matches!(self, Self::AssistantPrefixWithLineNumbers) && !output.is_error {
                number_lines(output);
                text.push_str(" Use the number printed before the matching line.");
            }
            request.items.push(ContextItem::User { text });
            return request;
        }
        request.tools.retain(|tool| tool.name.as_str() == "fs.read");
        if !matches!(self, Self::SingleTool) && !request.tools.is_empty() {
            request.items.push(ContextItem::Assistant {
                text: "<tool_call>\n".into(),
            });
        }
        request
    }

    fn shape_workflow(self, mut request: ModelRequest) -> ModelRequest {
        let results = request.items.iter().filter_map(|item| match item {
            ContextItem::ToolResult { output, .. } => Some(output),
            _ => None,
        });
        let completed = results.clone().count();
        let recovery = matches!(self, Self::RecoverStaleEdit);
        let expected_error = recovery && results.clone().nth(1).is_some_and(old_was_not_found);
        let blocked = results
            .enumerate()
            .any(|(index, output)| output.is_error && !(expected_error && index == 1))
            || (recovery && completed >= 2 && !expected_error);
        let read_prompt = match (self, completed) {
            (Self::EditPair { second, .. }, 1 | 4) => format!("Call fs_read to read {second} now."),
            (Self::EditPair { first, .. }, 3) => format!("Call fs_read to read {first} now."),
            _ => String::new(),
        };
        let (tool, prompt): (Option<&str>, &str) = match (self, completed, blocked) {
            (_, _, true) => (
                None,
                "Report the failed tool call without guessing file contents.",
            ),
            (Self::LocateConfig, 0, false) => (Some("fs.list"), ""),
            (Self::LocateConfig, 1, false) => (
                Some("fs.read"),
                "Read the current worker configuration file shown in the listing. Use its exact path. The legacy file is not the current configuration.",
            ),
            (Self::LocateConfig, _, false) => (
                None,
                "Return raw JSON with workers, path and line. Use the line number printed beside workers and the path from the read call. No Markdown.",
            ),
            (Self::CompareFiles, 0, false) => (Some("fs.read"), ""),
            (Self::CompareFiles, 1, false) => {
                (Some("fs.read"), "Call fs_read to read after.json now.")
            }
            (Self::CompareFiles, _, false) => (
                None,
                "Compare timeout_ms in the two files. Return raw JSON with timeout_before, timeout_after, before_line and after_line. Use the numbers printed beside timeout_ms in each file. No Markdown.",
            ),
            (Self::EditFile { .. }, 0, false) => (Some("fs.read"), ""),
            (Self::EditFile { goal, .. }, 1, false) => (Some("repo.edit-files"), goal),
            (Self::EditFile { .. }, 2, false) => {
                (Some("fs.read"), "Call fs_read to read the edited file now.")
            }
            (Self::EditFile { .. }, _, false) => (
                None,
                "Summarize only the change confirmed by the last file read.",
            ),
            (Self::EditPair { .. }, 0, false) => (Some("fs.read"), ""),
            (Self::EditPair { .. }, 1 | 3 | 4, false) => (Some("fs.read"), &read_prompt),
            (Self::EditPair { goal, .. }, 2, false) => (Some("repo.edit-files"), goal),
            (Self::EditPair { .. }, _, false) => (
                None,
                "Summarize the changes confirmed by the last two file reads.",
            ),
            (Self::RecoverStaleEdit, 0, false) => (Some("fs.read"), ""),
            (Self::RecoverStaleEdit, 1 | 3, false) => (
                Some("repo.edit-files"),
                "Call repo_edit-files now. Set the producer limit in queue.json to 8. Use one exact replacement copied from the latest file read. Keep the consumer limit.",
            ),
            (Self::RecoverStaleEdit, 2 | 4, false) => {
                (Some("fs.read"), "Call fs_read to read queue.json now.")
            }
            (Self::RecoverStaleEdit, _, false) => (
                None,
                "Report the producer and consumer limits confirmed by the latest read.",
            ),
            _ => unreachable!("the single-read variants have their own request shaping"),
        };
        request
            .tools
            .retain(|definition| Some(definition.name.as_str()) == tool);
        // Keep paths and line numbers attached to successful read results so a
        // comparison can distinguish two otherwise similar outputs.
        let paths: HashMap<_, _> = request
            .items
            .iter()
            .filter_map(|item| match item {
                ContextItem::ToolCall {
                    call_id,
                    name,
                    arguments,
                } if name.as_str() == "fs.read" => {
                    Some((call_id.clone(), arguments["path"].as_str()?.to_owned()))
                }
                _ => None,
            })
            .collect();
        for item in &mut request.items {
            if let ContextItem::ToolResult { call_id, output } = item
                && let Some(path) = paths.get(call_id)
                && !output.is_error
            {
                if !matches!(
                    self,
                    Self::EditFile { .. } | Self::EditPair { .. } | Self::RecoverStaleEdit
                ) {
                    number_lines(output);
                }
                output
                    .content
                    .insert(0, ToolContent::Text(format!("File: {path}")));
            }
        }
        if !prompt.is_empty() {
            request.items.push(ContextItem::User {
                text: prompt.into(),
            });
        }
        if !request.tools.is_empty() {
            let prefix = if matches!(
                self,
                Self::EditFile {
                    path: RUST_PROBE_PATH,
                    ..
                }
            ) {
                "<tool_call>\n{\"name\":"
            } else {
                "<tool_call>\n"
            };
            request.items.push(ContextItem::Assistant {
                text: prefix.into(),
            });
        }
        request
    }

    fn expected_workflow(self, task: &Task) -> Option<Vec<ExpectedCall<'_>>> {
        let reads = task
            .evidence
            .iter()
            .map(|path| ("fs.read", vec![path.as_str()]));
        match self {
            Self::LocateConfig => Some(
                std::iter::once(("fs.list", vec!["."]))
                    .chain(reads)
                    .collect(),
            ),
            Self::CompareFiles => Some(reads.collect()),
            Self::EditFile { path, .. } => Some(vec![
                ("fs.read", vec![path]),
                ("repo.edit-files", vec![path]),
                ("fs.read", vec![path]),
            ]),
            Self::EditPair { first, second, .. } => Some(vec![
                ("fs.read", vec![first]),
                ("fs.read", vec![second]),
                ("repo.edit-files", vec![first, second]),
                ("fs.read", vec![first]),
                ("fs.read", vec![second]),
            ]),
            Self::RecoverStaleEdit => Some(
                [
                    "fs.read",
                    "repo.edit-files",
                    "fs.read",
                    "repo.edit-files",
                    "fs.read",
                ]
                .into_iter()
                .map(|tool| (tool, vec![QUEUE_PATH]))
                .collect(),
            ),
            _ => None,
        }
    }
}

struct TaskProbeHook {
    manifest: PluginManifest,
    probe: TaskProbe,
    workspace: PathBuf,
    compiler_diagnostic: Option<Arc<str>>,
}

#[test]
fn the_read_probe_stages_the_tool_request_and_answer() {
    use rah_protocol::{PermissionLevel, ToolDefinition, ToolName, ToolOutput};

    let tool = |name| ToolDefinition {
        name: ToolName::new(name),
        description: String::new(),
        input_schema: json!({}),
        permission: PermissionLevel::Read,
    };
    let original = ModelRequest {
        tools: vec![tool("fs.read"), tool("fs.list")],
        ..ModelRequest::default()
    };
    let single = TaskProbe::SingleTool.shape(original.clone());
    assert_eq!(single.tools, [tool("fs.read")]);
    assert!(single.items.is_empty());
    let prefixed = TaskProbe::AssistantPrefix.shape(original.clone());
    assert_eq!(prefixed.tools, single.tools);
    assert_eq!(
        prefixed.items,
        [ContextItem::Assistant {
            text: "<tool_call>\n".into()
        }]
    );
    let mut followup = original;
    followup.items.push(ContextItem::ToolResult {
        call_id: "read".into(),
        output: ToolOutput {
            content: vec![ToolContent::Text("{\n  \"workers\": 6\n}\n".into())],
            is_error: false,
        },
    });
    let answer = TaskProbe::AssistantPrefix.shape(followup.clone());
    assert!(answer.tools.is_empty());
    assert!(matches!(
        &answer.items[..],
        [ContextItem::ToolResult { .. }, ContextItem::User { .. }]
    ));
    let numbered = TaskProbe::AssistantPrefixWithLineNumbers.shape(followup);
    let ContextItem::ToolResult { output, .. } = &numbered.items[0] else {
        panic!("read result is missing");
    };
    assert_eq!(
        output.content,
        [ToolContent::Text("1: {\n2:   \"workers\": 6\n3: }".into())]
    );
}

#[test]
fn read_workflows_keep_file_labels_and_stop_after_an_error() {
    use rah_protocol::{PermissionLevel, ToolDefinition, ToolName, ToolOutput};

    let request = || ModelRequest {
        tools: ["fs.list", "fs.read"]
            .into_iter()
            .map(|name| ToolDefinition {
                name: ToolName::new(name),
                description: String::new(),
                input_schema: json!({}),
                permission: PermissionLevel::Read,
            })
            .collect(),
        ..ModelRequest::default()
    };
    let step =
        |request: &mut ModelRequest, id: &str, name: &str, path: &str, text: &str, failed| {
            request.items.extend([
                ContextItem::ToolCall {
                    call_id: id.into(),
                    name: ToolName::new(name),
                    arguments: json!({"path":path}),
                },
                ContextItem::ToolResult {
                    call_id: id.into(),
                    output: ToolOutput {
                        content: vec![ToolContent::Text(text.into())],
                        is_error: failed,
                    },
                },
            ]);
        };
    let mut locate = request();
    assert_eq!(
        TaskProbe::LocateConfig.shape(locate.clone()).tools[0]
            .name
            .as_str(),
        "fs.list"
    );
    step(
        &mut locate,
        "listing",
        "fs.list",
        ".",
        "legacy.json\nworkspace-config.json",
        false,
    );
    let next = TaskProbe::LocateConfig.shape(locate.clone());
    assert_eq!(next.tools[0].name.as_str(), "fs.read");
    assert!(
        matches!(&next.items[1], ContextItem::ToolResult { output, .. }
        if output.content == [ToolContent::Text("legacy.json\nworkspace-config.json".into())])
    );
    step(
        &mut locate,
        "missing",
        "fs.read",
        "missing.json",
        "not found",
        true,
    );
    let blocked = TaskProbe::LocateConfig.shape(locate);
    assert!(blocked.tools.is_empty());
    assert!(
        matches!(blocked.items.last(), Some(ContextItem::User { text }) if text.contains("failed tool call"))
    );

    let mut compare = request();
    step(
        &mut compare,
        "before",
        "fs.read",
        "before.json",
        "{\n700\n}",
        false,
    );
    assert_eq!(
        TaskProbe::CompareFiles.shape(compare.clone()).tools[0]
            .name
            .as_str(),
        "fs.read"
    );
    step(
        &mut compare,
        "after",
        "fs.read",
        "after.json",
        "{\n900\n}",
        false,
    );
    let answer = TaskProbe::CompareFiles.shape(compare);
    assert!(answer.tools.is_empty());
    for (index, path, value) in [(1, "before.json", 700), (3, "after.json", 900)] {
        assert!(
            matches!(&answer.items[index], ContextItem::ToolResult { output, .. }
            if output.content == [ToolContent::Text(format!("File: {path}")),
                ToolContent::Text(format!("1: {{\n2: {value}\n3: }}"))])
        );
    }
}

#[async_trait]
impl Plugin for TaskProbeHook {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let probe = self.probe;
        if matches!(
            probe,
            TaskProbe::NativeRetry { .. } | TaskProbe::NativeRetryJson
        ) {
            scope.hook_waterfall::<ToolAfterInvoke, _, _>(0, |completion| async move {
                native_retry::mark_rejection(completion)
            });
        }
        if matches!(probe, TaskProbe::RecoverStaleEdit) {
            let path = self.workspace.join(QUEUE_PATH);
            let pending = AtomicBool::new(true);
            scope.hook_waterfall::<ToolAfterInvoke, _, _>(0, move |completion| {
                let call = &completion.request.call;
                let replace = call.name.as_str() == "fs.read"
                    && call.input.0["path"] == QUEUE_PATH
                    && !completion.output.is_error
                    && pending.swap(false, Ordering::Relaxed);
                let path = path.clone();
                async move {
                    if replace {
                        tokio::fs::write(path, QUEUE_AFTER_EXTERNAL_EDIT)
                            .await
                            .unwrap();
                    }
                    completion
                }
            });
        }
        let compiler_diagnostic = self.compiler_diagnostic.clone();
        scope.hook_waterfall::<ModelBeforeRequest, _, _>(-100, move |request| {
            let compiler_diagnostic = compiler_diagnostic.clone();
            async move {
                let mut request = probe.shape(request);
                if let Some(diagnostic) = compiler_diagnostic
                    && request
                        .tools
                        .iter()
                        .any(|tool| tool.name.as_str() == "repo.edit-files")
                    && let Some(ContextItem::User { text }) = request
                        .items
                        .iter_mut()
                        .rev()
                        .find(|item| matches!(item, ContextItem::User { .. }))
                {
                    *text = format!("Compiler output:\n{diagnostic}\n\n{text}");
                }
                request
            }
        });
        Ok(())
    }
}

async fn sample(
    task: &Task,
    loop_kind: &str,
    model: &str,
    port: u16,
    index: usize,
    probe: Option<TaskProbe>,
) -> Value {
    let dir = TempDir::new(&format!("small-model-live-{}-{loop_kind}-{index}", task.id));
    let root = dir.workspace();
    seed(&root, &task.files);
    let typed_json = matches!(probe, Some(TaskProbe::NativeJsonValue));
    let edit_tool = if typed_json {
        json_value::NAME
    } else {
        "repo.edit-files"
    };
    let compiler = matches!(
        probe,
        Some(TaskProbe::EditFile {
            path: RUST_PROBE_PATH,
            ..
        })
    )
    .then(|| TempDir::new(&format!("small-model-compiler-{index}")));
    let compiler_diagnostic: Option<Arc<str>> = if let Some(compiler) = &compiler {
        let workspace = compiler.workspace();
        let artifacts = compiler.0.clone();
        seed(&workspace, &task.files);
        Some(
            tokio::task::spawn_blocking(move || {
                Arc::from(rust_type_diagnostic(&rust_metadata(&workspace, &artifacts)))
            })
            .await
            .unwrap(),
        )
    } else {
        None
    };
    let mut distribution = headless(&dir);
    // Keep the real tools, policy, loop and model provider. Fixture content is
    // synthetic. The model has no shell or external integration tools.
    distribution.plugin.retain(|spec| {
        matches!(
            spec.kind.as_str(),
            "storage-fs"
                | "session-jsonl"
                | "policy-authority"
                | "sandbox-process"
                | "tools-builtin"
                | "tools-repo-write"
                | "context-default"
                | "tool-input-repair"
                | "model-mock"
                | "loop-react"
        )
    });
    if loop_kind == "loop-plan" {
        distribution.enable("plan").unwrap();
    }
    let mut assignments = serde_json::Map::new();
    for tool in READ_TOOLS {
        assignments.insert(tool.into(), json!("read"));
    }
    if !task.changes.is_empty() {
        assignments.insert("repo.edit-files".into(), json!("write"));
    }
    let mut approved = vec!["repo.edit-files"];
    if typed_json {
        assignments.insert(edit_tool.into(), json!("write"));
        approved.push(edit_tool);
    }
    distribution.set_config("policy-authority", "tools", Value::Object(assignments));
    distribution.set_config("policy-authority", "approve", json!(approved));
    if probe.is_some() {
        distribution.set_config(
            "context-default",
            "system",
            json!("Use the available tool to read the file."),
        );
        distribution.set_config("context-default", "environment", json!(false));
    }
    distribution
        .set_model(
            &format!("openai-compatible/{model}"),
            json!({
                "base_url": format!("http://127.0.0.1:{port}/v1"),
                "temperature": 0, "max_tokens": 2048,
            })
            .as_object()
            .unwrap(),
        )
        .unwrap();

    let kernel = kernel(&distribution).await;
    if matches!(
        probe,
        Some(
            TaskProbe::NativeGrammar(_)
                | TaskProbe::NativeRetry { .. }
                | TaskProbe::NativeRetryJson
                | TaskProbe::NativeJsonValue
        )
    ) {
        kernel
            .mount(
                Arc::new(write_guard::FixtureWriteGuard::new(
                    task.changes.keys().cloned(),
                )),
                MountOptions::default(),
            )
            .await
            .unwrap();
    }
    if typed_json || matches!(probe, Some(TaskProbe::NativeRetryJson)) {
        kernel
            .mount(
                Arc::new(json_preflight::FixtureJsonGuard::new(
                    root.clone(),
                    &task.id,
                )),
                MountOptions::default(),
            )
            .await
            .unwrap();
    }
    if typed_json {
        kernel
            .mount(
                Arc::new(json_value::FixtureJsonValue::new(root.clone(), &task.id)),
                MountOptions::default(),
            )
            .await
            .unwrap();
    }
    if let Some(probe) = probe {
        kernel
            .mount(
                Arc::new(TaskProbeHook {
                    manifest: PluginManifest::new("small-model-task-probe", "1"),
                    probe,
                    workspace: root.clone(),
                    compiler_diagnostic: compiler_diagnostic.clone(),
                }),
                MountOptions::default(),
            )
            .await
            .unwrap();
    }
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let cancel = CancellationToken::new();
    let started = Instant::now();
    let mut prompt = task.prompt.clone();
    if probe.is_none() && !task.evidence.is_empty() {
        prompt.push_str(
            " Open the relevant source files with fs.read before making edits or answering.",
        );
    }
    let mut stream = handle.get(LOOP).unwrap().run_turn(TurnRequest {
        origin: TurnOrigin::User,
        session,
        input: prompt,
        cancel: cancel.clone(),
    });
    let mut events = Vec::new();
    let timed_out = tokio::time::timeout(WAIT, async {
        while let Some(event) = stream.next().await {
            events.push(event);
        }
    })
    .await
    .is_err();
    let duration_ms = started.elapsed().as_millis();
    cancel.cancel();
    drop(stream);
    kernel.shutdown().await.unwrap();
    let mut result = grade(task, &root, &events, timed_out, edit_tool);
    result["duration_ms"] = json!(duration_ms);
    result["id"] = json!(task.id);
    result["group"] = json!(task.group);
    result["loop"] = json!(loop_kind);
    result["sample"] = json!(index);
    if let Some(probe) = probe {
        result["variant"] = json!(probe.name());
        let staged_mode = match probe {
            TaskProbe::Frozen(mode) | TaskProbe::NativeGrammar(mode) => Some(mode),
            TaskProbe::NativeRetry { .. }
            | TaskProbe::NativeRetryJson
            | TaskProbe::NativeJsonValue => Some(frozen::Mode::EditOne),
            _ => None,
        };
        if let Some(mode) = staged_mode {
            let trace = workflow_trace(&events);
            let retried = matches!(
                probe,
                TaskProbe::NativeRetry { .. } | TaskProbe::NativeRetryJson
            ) && native_retry::valid_retry_trace(&trace);
            let sequence_ok = mode.valid_trace(&trace, edit_tool) || retried;
            result["workflow_ok"] = json!(sequence_ok);
            result["recovered_after_old_missing"] = json!(retried && result["passed"] == true);
            if retried || (typed_json && sequence_ok) {
                // Both traces end with a successful read of a synthetic
                // fixture. Retain it as evidence for the grader's result.
                result["verified_fixture_excerpt"] = json!(events.iter().rev().find_map(|event| {
                    let AgentEvent::ToolFinished { output, .. } = event else {
                        return None;
                    };
                    output.content.iter().find_map(|part| match part {
                        ToolContent::Text(text) => {
                            Some(text.chars().take(1024).collect::<String>())
                        }
                        _ => None,
                    })
                }));
            }
            result["passed"] = json!(sequence_ok && result["passed"] == true);
            if matches!(
                probe,
                TaskProbe::NativeRetry { .. } | TaskProbe::NativeRetryJson
            ) {
                result["fixture_bytes_unchanged"] =
                    json!(task.files.iter().all(|(path, original)| {
                        fs::read(root.join(path)).ok().as_deref() == Some(original.as_bytes())
                    }));
                result["json_preflight_denials"] =
                    json!(events.iter().filter(|event| matches!(event,
                    AgentEvent::ToolFinished { output, .. } if json_preflight::was_denied(output)
                )).count());
            }
            result["tool_trace"] = json!(trace.iter().map(|(name, paths, outcome)| json!({"name":name,"paths":paths,"successful":*outcome == CallOutcome::Success,"retryable_old_missing":*outcome == CallOutcome::RetryableOldMissing})).collect::<Vec<_>>());
        }
        if matches!(
            probe,
            TaskProbe::EditFile { .. } | TaskProbe::EditPair { .. } | TaskProbe::RecoverStaleEdit
        ) {
            result["changed_fixture_paths"] = json!(
                task.files
                    .iter()
                    .filter_map(|(path, original)| {
                        (fs::read(root.join(path)).ok().as_deref() != Some(original.as_bytes()))
                            .then_some(path)
                    })
                    .collect::<Vec<_>>()
            );
        }
        if let Some(expected) = probe.expected_workflow(task) {
            let trace = workflow_trace(&events);
            let sequence_ok = trace.len() == expected.len()
                && trace.iter().zip(expected).enumerate().all(
                    |(index, ((name, actual, outcome), (expected_name, mut paths)))| {
                        paths.sort_unstable();
                        let wanted = if matches!(probe, TaskProbe::RecoverStaleEdit) && index == 1 {
                            CallOutcome::OldMissing
                        } else {
                            CallOutcome::Success
                        };
                        *outcome == wanted && *name == expected_name && *actual == paths
                    },
                );
            result["workflow_ok"] = json!(sequence_ok);
            result["passed"] = json!(sequence_ok && result["passed"] == true);
            result["tool_trace"] = json!(trace.iter().map(|(name, paths, outcome)| json!({"name":name,"paths":paths,"successful":*outcome == CallOutcome::Success,"old_missing":*outcome == CallOutcome::OldMissing})).collect::<Vec<_>>());
        }
    }
    if let Some(compiler) = compiler {
        result["compiler_before"] = json!(compiler_diagnostic.as_deref());
        let checked = if result["passed"] == true {
            let candidate = fs::read(root.join(RUST_PROBE_PATH)).unwrap();
            let expected = task.changes[RUST_PROBE_PATH].clone();
            tokio::task::spawn_blocking(move || {
                compile_reference_edit(&compiler.workspace(), &compiler.0, &candidate, &expected)
            })
            .await
            .unwrap()
        } else {
            None
        };
        let compiles = checked.as_ref().map(|output| output.status.success());
        result["compiler_after_ok"] = json!(compiles);
        result["passed"] = json!(compiles == Some(true));
    }
    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallOutcome {
    Success,
    OldMissing,
    RetryableOldMissing,
    OtherFailure,
    Incomplete,
}

fn workflow_trace(events: &[AgentEvent]) -> Vec<(&str, Vec<&str>, CallOutcome)> {
    let outcomes: HashMap<_, _> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolFinished {
                tool_call_id,
                output,
                ..
            } => Some((
                tool_call_id,
                if !output.is_error {
                    CallOutcome::Success
                } else if native_retry::is_verified_rejection(output) {
                    CallOutcome::RetryableOldMissing
                } else if old_was_not_found(output) {
                    CallOutcome::OldMissing
                } else {
                    CallOutcome::OtherFailure
                },
            )),
            _ => None,
        })
        .collect();
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolRequested { tool_call, .. } => Some((
                tool_call.name.as_str(),
                workflow_paths(tool_call.name.as_str(), &tool_call.input.0).unwrap_or_default(),
                outcomes
                    .get(&tool_call.id)
                    .copied()
                    .unwrap_or(CallOutcome::Incomplete),
            )),
            _ => None,
        })
        .collect()
}

fn workflow_paths<'a>(name: &str, input: &'a Value) -> Option<Vec<&'a str>> {
    if name == "repo.edit-files" {
        let mut paths = input["edits"]
            .as_array()?
            .iter()
            .map(|edit| edit["path"].as_str())
            .collect::<Option<Vec<_>>>()?;
        paths.sort_unstable();
        return Some(paths);
    }
    let path = match input.get("path") {
        None | Some(Value::Null) if name == "fs.list" => ".",
        Some(path) => path.as_str()?,
        None => return None,
    };
    Some(vec![path])
}

#[test]
fn single_file_workflow_preserves_the_source_and_requires_one_edit() {
    use rah_protocol::{PermissionLevel, ToolDefinition, ToolName, ToolOutput};

    let path = "worker.json";
    let source = "{\"batch\":{\"max_items\":16}}\n";
    let probe = TaskProbe::EditFile {
        path,
        goal: "Set batch.max_items to 24.",
    };
    let edit = json!({"path":path,"old":"16","new":"24"});
    assert_eq!(
        workflow_paths("repo.edit-files", &json!({"edits":[edit]})),
        Some(vec![path])
    );
    assert_eq!(
        workflow_paths("repo.edit-files", &json!({"edits":[edit,edit]})),
        Some(vec![path, path])
    );
    let mut request = ModelRequest {
        tools: [
            ("fs.read", PermissionLevel::Read),
            ("repo.edit-files", PermissionLevel::Write),
        ]
        .into_iter()
        .map(|(name, permission)| ToolDefinition {
            name: ToolName::new(name),
            description: String::new(),
            input_schema: json!({}),
            permission,
        })
        .collect(),
        items: vec![
            ContextItem::ToolCall {
                call_id: "read".into(),
                name: ToolName::new("fs.read"),
                arguments: json!({"path":path}),
            },
            ContextItem::ToolResult {
                call_id: "read".into(),
                output: ToolOutput {
                    content: vec![ToolContent::Text(source.into())],
                    is_error: false,
                },
            },
        ],
        ..ModelRequest::default()
    };
    let next = probe.shape(request.clone());
    assert_eq!(next.tools[0].name.as_str(), "repo.edit-files");
    assert!(
        matches!(&next.items[1], ContextItem::ToolResult { output, .. }
        if output.content == [ToolContent::Text(format!("File: {path}")), ToolContent::Text(source.into())])
    );
    request.items.push(ContextItem::ToolResult {
        call_id: "edit".into(),
        output: ToolOutput {
            content: vec![],
            is_error: false,
        },
    });
    assert_eq!(probe.shape(request).tools[0].name.as_str(), "fs.read");
}

fn median_ms(rows: &[&Value]) -> Option<f64> {
    let mut durations: Vec<_> = rows
        .iter()
        .map(|row| row["duration_ms"].as_f64().unwrap())
        .collect();
    durations.sort_by(f64::total_cmp);
    let middle = durations.len() / 2;
    durations
        .get(middle)
        .map(|time| (*time + durations[(durations.len() - 1) / 2]) / 2.0)
}

fn live_settings() -> (String, String, u16) {
    let model =
        std::env::var("RAH_SMALL_MODEL").expect("set RAH_SMALL_MODEL to the served model ID");
    let run_label = std::env::var("RAH_SMALL_MODEL_RUN").expect(
        "set RAH_SMALL_MODEL_RUN to backend, quantization, context limit and thinking settings",
    );
    assert!(!model.trim().is_empty() && !run_label.trim().is_empty());
    let port = std::env::var("RAH_SMALL_MODEL_PORT")
        .expect("set RAH_SMALL_MODEL_PORT to the isolated llama.cpp test port")
        .parse::<u16>()
        .expect("invalid port");
    assert!(
        port != 0 && port != 8080,
        "use a dedicated benchmark listener"
    );
    (model, run_label, port)
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_read_probe() {
    let (model, run_label, port) = live_settings();
    let mut task = tasks()
        .into_iter()
        .find(|task| task.id == "service-default")
        .unwrap();
    task.prompt = "Call fs_read to read service.json now.".into();
    eprintln!(
        "small-model read probe configuration: {}",
        json!({"model":model,"run":run_label,"port":port})
    );
    for (index, probe) in [
        TaskProbe::SingleTool,
        TaskProbe::AssistantPrefix,
        TaskProbe::AssistantPrefixWithLineNumbers,
        TaskProbe::AssistantPrefixWithLineNumbers,
        TaskProbe::AssistantPrefix,
        TaskProbe::SingleTool,
    ]
    .into_iter()
    .enumerate()
    {
        let row = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
        eprintln!("small-model read probe sample: {row}");
    }
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_read_workflows() {
    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model read workflow configuration: {}",
        json!({"model":model,"run":run_label,"port":port})
    );
    for (index, (current, workers)) in [
        ("current-worker-settings.json", 6),
        ("live-worker-settings.json", 11),
    ]
    .into_iter()
    .enumerate()
    {
        let mut locate = tasks().remove(0);
        locate.prompt = "Call fs_list to list . now.".into();
        locate.files.remove("service.json");
        locate.files.insert(
            current.into(),
            format!("{{\n  \"workers\": {workers}\n}}\n"),
        );
        locate.evidence = vec![current.into()];
        locate.answer = Some(json!({"workers":workers,"path":current,"line":2}));
        let mut compare = tasks().remove(1);
        compare.prompt = "Call fs_read to read before.json now.".into();
        for (task, probe) in [
            (&locate, TaskProbe::LocateConfig),
            (&compare, TaskProbe::CompareFiles),
        ] {
            let result = sample(task, "loop-react", &model, port, index + 1, Some(probe)).await;
            eprintln!("small-model read workflow sample: {result}");
        }
    }
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_edit_workflows() {
    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model edit workflow configuration: {}",
        json!({"model":model,"run":run_label,"port":port})
    );
    for index in 1..=2 {
        for (id, path, goal) in [
            (
                "batch-limit",
                "worker.json",
                "Call repo_edit-files now. In worker.json set batch.max_items to 24. Submit one edit and copy the old text exactly from the file.",
            ),
            (
                "fallback-order",
                "routing.json",
                "Call repo_edit-files now. Swap the first two entries in the fallback array of routing.json. Leave the last entry and primary region as they are. Use one edit with different old and new text.",
            ),
        ] {
            let mut task = tasks().into_iter().find(|task| task.id == id).unwrap();
            task.prompt = format!("Call fs_read to read {path} now.");
            task.evidence = vec![path.into()];
            let result = sample(
                &task,
                "loop-react",
                &model,
                port,
                index,
                Some(TaskProbe::EditFile { path, goal }),
            )
            .await;
            eprintln!("small-model edit workflow sample: {result}");
        }
    }
}

#[test]
fn cross_file_grader_rejects_half_a_change() {
    for task in tasks().into_iter().filter(|task| task.group == "cross") {
        let dir = TempDir::new(&format!("small-model-half-edit-{}", task.id));
        let root = dir.workspace();
        seed(&root, &task.files);
        let [(first, content), (second, _)] = task.changes.iter().collect::<Vec<_>>()[..] else {
            panic!("{} must have two expected edits", task.id);
        };
        fs::write(root.join(first), content).unwrap();
        assert_eq!(file_failures(&task, &root), (vec![second.clone()], vec![]));
    }
    let first = json!({"path":"client.json","old":"2","new":"3"});
    let second = json!({"path":"service.json","old":"2","new":"3"});
    assert_eq!(
        workflow_paths("repo.edit-files", &json!({"edits":[second,first]})),
        Some(vec!["client.json", "service.json"])
    );
    assert_ne!(
        workflow_paths("repo.edit-files", &json!({"edits":[first,first]})),
        Some(vec!["client.json", "service.json"])
    );
}

fn stale_edit_fixture() -> Task {
    let mut task = tasks()
        .into_iter()
        .find(|task| task.id == "stale-edit")
        .unwrap();
    task.prompt = "Call fs_read to read queue.json now.".into();
    task.files.insert(QUEUE_PATH.into(), QUEUE_BEFORE.into());
    task.changes.insert(
        QUEUE_PATH.into(),
        json!({"consumer":{"limit":5},"producer":{"limit":8}}).to_string(),
    );
    task.evidence = vec![QUEUE_PATH.into()];
    task
}

#[tokio::test]
async fn stale_replacement_cannot_overwrite_the_external_change() {
    use rah_api::tools::{Invocation, ToolInvoker};
    use rah_protocol::{ToolCall, ToolCallId, ToolInput, ToolName};

    let task = stale_edit_fixture();
    let dir = TempDir::new("small-model-stale-tool");
    let root = dir.workspace();
    seed(&root, &task.files);
    let mut distribution = headless(&dir);
    distribution.set_config("policy-authority", "approve", json!(["repo.edit-files"]));
    let kernel = kernel(&distribution).await;
    kernel
        .mount(
            Arc::new(TaskProbeHook {
                manifest: PluginManifest::new("stale-tool-fixture", "1"),
                probe: TaskProbe::RecoverStaleEdit,
                workspace: root.clone(),
                compiler_diagnostic: None,
            }),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let invoker = ToolInvoker::new(handle);
    let call = |name, input| {
        invoker.invoke(
            Invocation {
                session: session.clone(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
    };
    let read = call("fs.read", json!({"path":QUEUE_PATH})).await.output;
    assert_eq!(read.content, [ToolContent::Text(QUEUE_BEFORE.into())]);
    assert_eq!(
        fs::read_to_string(root.join(QUEUE_PATH)).unwrap(),
        QUEUE_AFTER_EXTERNAL_EDIT
    );
    let edit =
        |old| json!({"edits":[{"path":QUEUE_PATH,"old":old,"new":"\"producer\": {\"limit\": 8}"}]});
    let stale = call("repo.edit-files", edit("\"producer\": {\"limit\": 6}"))
        .await
        .output;
    assert!(old_was_not_found(&stale), "{stale:?}");
    assert_eq!(
        fs::read_to_string(root.join(QUEUE_PATH)).unwrap(),
        QUEUE_AFTER_EXTERNAL_EDIT
    );
    let current = call("fs.read", json!({"path":QUEUE_PATH})).await.output;
    assert_eq!(
        current.content,
        [ToolContent::Text(QUEUE_AFTER_EXTERNAL_EDIT.into())]
    );
    let fixed = call("repo.edit-files", edit("\"producer\": {\"limit\": 4}"))
        .await
        .output;
    assert!(!fixed.is_error, "{fixed:?}");
    assert_eq!(file_failures(&task, &root), (vec![], vec![]));
    kernel.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_stale_edit_recovery() {
    let (model, run_label, port) = live_settings();
    let task = stale_edit_fixture();
    eprintln!(
        "small-model stale edit configuration: {}",
        json!({"model":model,"run":run_label,"port":port})
    );
    for index in 1..=2 {
        let row = sample(
            &task,
            "loop-react",
            &model,
            port,
            index,
            Some(TaskProbe::RecoverStaleEdit),
        )
        .await;
        eprintln!("small-model stale edit sample: {row}");
    }
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_rust_type_recovery() {
    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model rust recovery configuration: {}",
        json!({"model":model,"run":run_label,"port":port})
    );
    for (index, intended_limit) in [3, 13].into_iter().enumerate() {
        let mut task = tasks()
            .into_iter()
            .find(|task| task.id == "rust-type-mismatch")
            .unwrap();
        task.prompt = "Call fs_read to read limits.rs now.".into();
        task.files.insert(
            RUST_PROBE_PATH.into(),
            format!(
                "pub const RETRY_LIMIT: u32 = \"{intended_limit}\";\npub const WORKERS: u32 = 3;\n"
            ),
        );
        task.changes.insert(
            RUST_PROBE_PATH.into(),
            format!(
                "pub const RETRY_LIMIT: u32 = {intended_limit};\npub const WORKERS: u32 = 3;\n"
            ),
        );
        task.evidence = vec![RUST_PROBE_PATH.into()];
        let probe = TaskProbe::EditFile {
            path: RUST_PROBE_PATH,
            goal: "Call repo_edit-files now. Fix the compiler error in limits.rs. Keep RETRY_LIMIT as u32 with the same numeric value. Preserve WORKERS. Use one exact replacement.",
        };
        let row = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
        eprintln!("small-model rust recovery sample: {row}");
    }
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_cross_file_workflows() {
    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model cross-file configuration: {}",
        json!({"model":model,"run":run_label,"port":port})
    );
    for index in 1..=2 {
        for (id, first, second, goal) in [
            (
                "protocol-version",
                "client.json",
                "service.json",
                "Call repo_edit-files now. Set protocol_version to 3 in client.json and service.json. Use one batch with one exact replacement per file. Leave cache_version unchanged.",
            ),
            (
                "queue-rename",
                "scheduler.json",
                "worker.json",
                "Call repo_edit-files now. Rename mail to outbound in scheduler.json and worker.json. Use one batch with one exact replacement per file.",
            ),
        ] {
            let mut task = tasks().into_iter().find(|task| task.id == id).unwrap();
            task.prompt = format!("Call fs_read to read {first} now.");
            task.evidence = vec![first.into(), second.into()];
            let row = sample(
                &task,
                "loop-react",
                &model,
                port,
                index,
                Some(TaskProbe::EditPair {
                    first,
                    second,
                    goal,
                }),
            )
            .await;
            eprintln!("small-model cross-file sample: {row}");
        }
    }
}

#[tokio::test]
#[ignore = "requires explicit opt-in and a local OpenAI-compatible model on 127.0.0.1"]
async fn live_small_model_baseline() {
    let (model, run_label, port) = live_settings();
    let set = std::env::var("RAH_SMALL_MODEL_SET").unwrap_or_else(|_| "development".into());
    assert!(matches!(set.as_str(), "development" | "holdout"));
    eprintln!(
        "small-model configuration: {}",
        json!({"model":model,"run":run_label,
        "port":port,"set":set,"temperature":0,"max_output_tokens":2048,"timeout_secs":WAIT.as_secs()})
    );
    let mut results = Vec::new();
    for (index, task) in tasks().iter().enumerate() {
        let holdout = index % 3 == 2;
        if holdout != (set == "holdout") {
            continue;
        }
        let order = if index % 2 == 0 {
            [0, 1, 1, 0]
        } else {
            [1, 0, 0, 1]
        };
        for (sample_index, side) in order.into_iter().enumerate() {
            let row = sample(task, LOOPS[side], &model, port, sample_index + 1, None).await;
            eprintln!("small-model sample: {row}");
            results.push(row);
        }
    }
    assert_eq!(results.len(), if set == "holdout" { 16 } else { 32 });
    for loop_kind in LOOPS {
        let rows: Vec<_> = results
            .iter()
            .filter(|row| row["loop"] == loop_kind)
            .collect();
        let passed: Vec<_> = rows
            .iter()
            .copied()
            .filter(|row| row["passed"] == true)
            .collect();
        eprintln!(
            "small-model summary: {}",
            json!({"loop":loop_kind,"samples":rows.len(),
            "passed":passed.len(),"timeouts":rows.iter().filter(|row| row["timed_out"] == true).count(),
            "median_ms":median_ms(&rows),"median_passed_ms":median_ms(&passed)})
        );
    }
}
