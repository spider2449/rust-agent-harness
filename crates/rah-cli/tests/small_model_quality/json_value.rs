//! Live small-model comparison for the optional JSON integer tool.

#[path = "json_value_lifecycle.rs"]
mod lifecycle;

use rah_cli::PluginSpec;
pub(super) use rah_plugin_tools_json_edit::TOOL_NAME as NAME;
use serde_json::json;

pub(super) fn plugin(task: &str) -> PluginSpec {
    let (path, pointer) = super::json_preflight::target(task);
    PluginSpec {
        kind: "tools-json-edit".into(),
        id: None,
        config: json!({"path":path,"pointer":pointer}),
        bind: Default::default(),
    }
}

#[tokio::test]
#[ignore = "requires an isolated llama.cpp with Qwen3-Coder template and the native tool choice proxy"]
async fn live_small_model_json_value() {
    use super::{TaskProbe, live_settings, sample, tasks};

    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model JSON value configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":"development"})
    );
    for task in tasks()
        .into_iter()
        .filter(|task| matches!(task.id.as_str(), "batch-limit" | "stale-edit"))
    {
        for (index, probe) in [
            TaskProbe::NativeRetryJson,
            TaskProbe::NativeJsonValue,
            TaskProbe::NativeJsonValue,
            TaskProbe::NativeRetryJson,
        ]
        .into_iter()
        .enumerate()
        {
            let row = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
            eprintln!("small-model JSON value sample: {row}");
        }
    }
}
