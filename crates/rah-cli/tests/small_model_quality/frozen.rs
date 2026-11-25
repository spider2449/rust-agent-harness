//! Fixed read/edit stages for the holdout evaluation. The request shaper
//! receives a workload class and the model's conversation, never the grader.

use std::collections::HashMap;

use rah_protocol::{ContextItem, ModelRequest, ToolContent};
use serde_json::json;

use super::{CallOutcome, TaskProbe, live_settings, median_ms, number_lines, sample, tasks};

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Read,
    EditOne,
    EditTwo,
}

impl Mode {
    fn for_group(group: &str) -> Self {
        match group {
            "read" => Self::Read,
            "single" | "recovery" => Self::EditOne,
            "cross" => Self::EditTwo,
            _ => panic!("unknown workload group: {group}"),
        }
    }

    fn reads(self) -> usize {
        match self {
            Self::Read | Self::EditTwo => 2,
            Self::EditOne => 1,
        }
    }

    pub(super) fn shape(
        self,
        mut request: ModelRequest,
        native_grammar: bool,
        edit_tool: &'static str,
    ) -> ModelRequest {
        let goal = request
            .items
            .iter()
            .find_map(|item| match item {
                ContextItem::User { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let calls: HashMap<_, _> = request
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
        let mut paths = Vec::new();
        let mut completed = 0;
        let mut failed = false;
        for item in &mut request.items {
            if let ContextItem::ToolResult { call_id, output } = item {
                completed += 1;
                failed |= output.is_error;
                if let Some(path) = calls.get(call_id).filter(|_| !output.is_error) {
                    paths.push(path.as_str());
                    if matches!(self, Self::Read) {
                        number_lines(output);
                    }
                    output
                        .content
                        .insert(0, ToolContent::Text(format!("File: {path}")));
                }
            }
        }
        let reads = self.reads();
        let (tool, instruction) = if failed {
            (
                None,
                "Report the failed call and what remains unfinished.".to_owned(),
            )
        } else if completed == 0 {
            let prompt = if native_grammar {
                "Call fs_list now, with no arguments."
            } else {
                "Call fs_list to list . now."
            };
            (Some("fs.list"), prompt.to_owned())
        } else if completed <= reads {
            let previously_read = if paths.is_empty() {
                "none".into()
            } else {
                paths.join(", ")
            };
            (
                Some("fs.read"),
                format!(
                    "Files already read: {previously_read}. Use the listing and the task to pick the next relevant file. If every relevant file has been read, reread the most useful one. Call fs_read now."
                ),
            )
        } else if matches!(self, Self::Read) {
            (
                None,
                format!(
                    "Answer the original task from the files just read. Use the printed line numbers when asked and follow the requested response format.\n{goal}"
                ),
            )
        } else if completed == reads + 1 && edit_tool == super::json_value::NAME {
            (
                Some(edit_tool),
                format!(
                    "{goal}\nCall fixture_set-json-value now. Supply only the requested new integer value for the configured field."
                ),
            )
        } else if completed == reads + 1 {
            (
                Some("repo.edit-files"),
                format!(
                    "{goal}\nCopy old text exactly from the file contents. Submit one batch with one replacement for each target file. Call repo_edit-files now."
                ),
            )
        } else if completed <= reads * 2 + 1 {
            match paths.get(completed - reads - 2) {
                Some(path) => (Some("fs.read"), format!("Call fs_read to read {path} now.")),
                None => (
                    None,
                    "Report which edited files could not be verified.".to_owned(),
                ),
            }
        } else {
            (
                None,
                "Summarize the changes confirmed by the final file reads.".to_owned(),
            )
        };
        request
            .tools
            .retain(|definition| Some(definition.name.as_str()) == tool);
        request.items.push(ContextItem::User { text: instruction });
        if native_grammar && tool == Some("fs.list") {
            // With no arguments the real tool lists the workspace root.
            // Later stages still have to choose what to read.
            for definition in &mut request.tools {
                definition.input_schema =
                    json!({"type":"object","properties":{},"additionalProperties":false});
            }
        }
        if !native_grammar && !request.tools.is_empty() {
            request.items.push(ContextItem::Assistant {
                text: "<tool_call>\n{\"name\":".into(),
            });
        }
        request
    }

    pub(super) fn valid_trace(
        self,
        trace: &[(&str, Vec<&str>, CallOutcome)],
        edit_tool: &str,
    ) -> bool {
        let reads = self.reads();
        let expected_count = if matches!(self, Self::Read) {
            1 + reads
        } else {
            2 + 2 * reads
        };
        if trace.len() != expected_count
            || trace
                .iter()
                .any(|(_, _, outcome)| *outcome != CallOutcome::Success)
        {
            return false;
        }
        if trace[0].0 != "fs.list" || trace[0].1 != ["."] {
            return false;
        }
        let initial = &trace[1..=reads];
        if initial
            .iter()
            .any(|(name, paths, _)| *name != "fs.read" || paths.len() != 1)
        {
            return false;
        }
        if matches!(self, Self::Read) {
            return true;
        }
        let mut expected_paths: Vec<_> = initial.iter().map(|(_, paths, _)| paths[0]).collect();
        expected_paths.sort_unstable();
        expected_paths.dedup();
        let (name, edited, _) = &trace[reads + 1];
        let targets_ok = if edit_tool == super::json_value::NAME {
            edited.is_empty()
        } else {
            *edited == expected_paths
        };
        expected_paths.len() == reads
            && *name == edit_tool
            && targets_ok
            && trace[reads + 2..].iter().zip(initial).all(
                |((name, paths, _), (_, read_paths, _))| *name == "fs.read" && paths == read_paths,
            )
    }
}

#[tokio::test]
#[ignore = "requires an isolated llama.cpp with Qwen3-Coder template and the native tool choice proxy"]
async fn live_small_model_native_grammar() {
    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model native configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":"development"})
    );
    for index in 1..=2 {
        for task in tasks().into_iter().filter(|task| {
            matches!(
                task.id.as_str(),
                "service-default" | "batch-limit" | "queue-rename" | "stale-edit"
            )
        }) {
            let probe = TaskProbe::NativeGrammar(Mode::for_group(&task.group));
            let result = sample(&task, "loop-react", &model, port, index, Some(probe)).await;
            eprintln!("small-model native sample: {result}");
        }
    }
}

#[test]
fn edit_trace_rejects_one_sided_writes_duplicate_paths_and_unverified_files() {
    let ok = CallOutcome::Success;
    let original = vec![
        ("fs.list", vec!["."], ok),
        ("fs.read", vec!["a.json"], ok),
        ("fs.read", vec!["b.json"], ok),
        ("repo.edit-files", vec!["a.json", "b.json"], ok),
        ("fs.read", vec!["a.json"], ok),
        ("fs.read", vec!["b.json"], ok),
    ];
    assert!(Mode::EditTwo.valid_trace(&original, "repo.edit-files"));
    for (index, paths) in [
        (3, vec!["a.json"]),
        (3, vec!["a.json", "a.json"]),
        (5, vec!["a.json"]),
    ] {
        let mut wrong = original.clone();
        wrong[index].1 = paths;
        assert!(!Mode::EditTwo.valid_trace(&wrong, "repo.edit-files"));
    }
}

#[tokio::test]
#[ignore = "requires explicit opt-in and an isolated local OpenAI-compatible model"]
async fn live_small_model_frozen() {
    let (model, run_label, port) = live_settings();
    let set = std::env::var("RAH_SMALL_MODEL_SET").unwrap_or_else(|_| "development".into());
    assert!(matches!(set.as_str(), "development" | "holdout"));
    eprintln!(
        "small-model frozen configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":set,"policy":"staged-v1"})
    );
    let mut results = Vec::new();
    for (index, task) in tasks().iter().enumerate() {
        if (index % 3 == 2) != (set == "holdout") {
            continue;
        }
        let probe = TaskProbe::Frozen(Mode::for_group(&task.group));
        for index in 1..=2 {
            let row = sample(task, "loop-react", &model, port, index, Some(probe)).await;
            eprintln!("small-model frozen sample: {row}");
            results.push(row);
        }
    }
    assert_eq!(results.len(), if set == "holdout" { 8 } else { 16 });
    let rows: Vec<_> = results.iter().collect();
    let passed: Vec<_> = rows
        .iter()
        .copied()
        .filter(|row| row["passed"] == true)
        .collect();
    eprintln!(
        "small-model frozen summary: {}",
        json!({"set":set,"samples":rows.len(),"passed":passed.len(),
        "timeouts":rows.iter().filter(|row| row["timed_out"] == true).count(),
        "median_ms":median_ms(&rows),"median_passed_ms":median_ms(&passed)})
    );
}
