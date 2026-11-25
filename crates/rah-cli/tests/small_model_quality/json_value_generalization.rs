//! Held-out fixtures for the typed integer tool. Prompts and writable
//! locations are separate from the expected documents used by the grader.

use serde_json::json;

use super::{Task, TaskProbe, live_settings, sample};

const CHECKS: &str = "{\"scope\":\"fixture ownership check\"}\n";

struct Case {
    id: &'static str,
    file: &'static str,
    pointer: &'static str,
    prompt: &'static str,
    source: &'static str,
    expected: &'static str,
}

const CASES: [Case; 4] = [
    Case {
        id: "cache-capacity",
        file: "cache.json",
        pointer: "/cache/max_entries",
        prompt: "Set cache.max_entries in cache.json to 96. Preserve the other keys and all other files.",
        source: "{\"cache\":{\"max_entries\":32,\"ttl_s\":32},\"shards\":2}",
        expected: "{\"cache\":{\"max_entries\":96,\"ttl_s\":32},\"shards\":2}",
    },
    Case {
        id: "sampling-window",
        file: "sampling.json",
        pointer: "/window/seconds",
        prompt: "Double the current window.seconds in sampling.json. Leave the rest of the document and workspace unchanged.",
        source: "{\r\n  \"window\": {\"seconds\": 15, \"grace\": 4},\r\n  \"samples_per_second\": 15\r\n}\r\n",
        expected: "{\"window\":{\"seconds\":30,\"grace\":4},\"samples_per_second\":15}",
    },
    Case {
        id: "requests-backoff",
        file: "requests.json",
        pointer: "/retry/backoff_ms",
        prompt: "In requests.json, set retry.backoff_ms to the current value of timing.minimum_backoff_ms. Change just retry.backoff_ms.",
        source: "{\n  \"retry\": {\"attempts\": 4, \"backoff_ms\": 60},\n  \"timing\": {\"minimum_backoff_ms\": 180, \"max_ms\": 240}\n}\n",
        expected: "{\"retry\":{\"attempts\":4,\"backoff_ms\":180},\"timing\":{\"minimum_backoff_ms\":180,\"max_ms\":240}}",
    },
    Case {
        id: "worker-timeout",
        file: "service.json",
        pointer: "/worker/timeout_ms",
        prompt: "Reduce worker.timeout_ms in service.json by 120 milliseconds. Preserve the health timeout and every other setting.",
        source: "{\n  \"health\": {\"timeout_ms\": 1200},\n  \"worker\": {\"timeout_ms\": 1200, \"count\": 2},\n  \"enabled\": true\n}\n",
        expected: "{\"health\":{\"timeout_ms\":1200},\"worker\":{\"timeout_ms\":1080,\"count\":2},\"enabled\":true}",
    },
];

pub(super) fn target(id: &str) -> Option<(&'static str, &'static str)> {
    CASES
        .iter()
        .find(|case| case.id == id)
        .map(|case| (case.file, case.pointer))
}

impl Case {
    fn task(&self) -> Task {
        Task {
            id: self.id.into(),
            group: "single".into(),
            prompt: self.prompt.into(),
            files: [
                (self.file.into(), self.source.into()),
                ("checks.json".into(), CHECKS.into()),
            ]
            .into(),
            changes: [(self.file.into(), self.expected.into())].into(),
            answer: None,
            evidence: vec![self.file.into()],
        }
    }
}

#[test]
fn the_holdout_reference_changes_only_its_configured_integer() {
    use super::{file_failures, seed};
    use crate::common::TempDir;
    use serde_json::Value;

    for case in CASES {
        let original: Value = serde_json::from_str(case.source).unwrap();
        let mut expected: Value = serde_json::from_str(case.expected).unwrap();
        let current = original.pointer(case.pointer).unwrap();
        let new = expected.pointer_mut(case.pointer).unwrap();
        assert!(
            current.is_u64() && new.is_u64() && current != new,
            "{}",
            case.id
        );
        *new = current.clone();
        assert_eq!(original, expected, "{}", case.id);

        let task = case.task();
        let dir = TempDir::new(&format!("small-model-integer-holdout-{}", case.id));
        let root = dir.workspace();
        seed(&root, &task.files);
        assert_eq!(
            file_failures(&task, &root),
            (vec![case.file.into()], vec![])
        );
        seed(&root, &task.changes);
        assert_eq!(file_failures(&task, &root), (vec![], vec![]));
    }
}

#[tokio::test]
#[ignore = "requires an isolated llama.cpp with Qwen3-Coder template and the native tool choice proxy"]
async fn live_small_model_integer_holdout() {
    let (model, run_label, port) = live_settings();
    let selection = std::env::var("RAH_SMALL_MODEL_CASES").ok();
    let selected: Vec<_> = selection
        .as_deref()
        .map(|ids| ids.split(',').map(str::trim).collect())
        .unwrap_or_else(|| CASES.iter().map(|case| case.id).collect());
    for id in &selected {
        assert!(target(id).is_some(), "unknown integer holdout case `{id}`");
    }
    eprintln!(
        "small-model integer holdout configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":"integer_holdout","cases":selected})
    );
    for (case_index, case) in CASES.iter().enumerate() {
        if !selected.contains(&case.id) {
            continue;
        }
        let task = case.task();
        let order = if case_index % 2 == 0 {
            [false, true, true, false]
        } else {
            [true, false, false, true]
        };
        for (index, typed) in order.into_iter().enumerate() {
            let probe = if typed {
                TaskProbe::NativeJsonValue
            } else {
                TaskProbe::NativeRetryJson
            };
            let row = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
            eprintln!("small-model integer holdout sample: {row}");
        }
    }
}
