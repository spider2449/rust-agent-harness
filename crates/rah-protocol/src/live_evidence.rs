//! Best-effort process-local JSONL evidence append support.

use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::{Mutex, OnceLock},
};

use serde_json::Value;

static APPEND_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Appends one complete JSON object as one JSONL record when evidence is enabled.
pub fn append(record: &Value) {
    let Some(path) = std::env::var_os("RAH_LIVE_EVIDENCE_PATH") else {
        return;
    };
    append_to_path(Path::new(&path), record);
}

fn append_to_path(path: &Path, record: &Value) {
    let Ok(mut line) = serde_json::to_vec(record) else {
        return;
    };
    line.push(b'\n');

    let lock = APPEND_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    if file.write_all(&line).is_err() {
        return;
    }
    let _ = file.flush();
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Barrier, OnceLock},
        thread,
    };

    use serde_json::json;

    use super::{append, append_to_path};

    fn test_path(name: &str) -> PathBuf {
        static COUNTER: OnceLock<std::sync::atomic::AtomicUsize> = OnceLock::new();
        let number = COUNTER
            .get_or_init(|| std::sync::atomic::AtomicUsize::new(0))
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "rah-live-evidence-{name}-{}-{number}.jsonl",
            std::process::id()
        ))
    }

    fn environment_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    fn read_records(path: &PathBuf) -> Vec<serde_json::Value> {
        let contents = fs::read_to_string(path).expect("evidence file should be readable");
        let lines = contents.lines().collect::<Vec<_>>();
        assert!(lines.iter().all(|line| !line.is_empty()));
        lines
            .into_iter()
            .map(|line| serde_json::from_str(line).expect("each JSONL line should parse"))
            .collect()
    }

    #[test]
    fn sequential_appends_produce_independently_parseable_records() {
        let path = test_path("sequential");
        for index in 0..8 {
            append_to_path(&path, &json!({"event": "sequential", "index": index}));
        }

        let records = read_records(&path);
        assert_eq!(records.len(), 8);
        assert!(records.iter().all(serde_json::Value::is_object));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn concurrent_independent_producers_share_record_serialization() {
        let path = Arc::new(test_path("concurrent"));
        let start = Arc::new(Barrier::new(3));
        let writers = (0..2)
            .map(|producer| {
                let path = Arc::clone(&path);
                let start = Arc::clone(&start);
                thread::spawn(move || {
                    start.wait();
                    for index in 0..128 {
                        append_to_path(
                            &path,
                            &json!({
                                "event": "producer_record",
                                "producer": producer,
                                "index": index,
                                "text_result": "TASK175D_SHARED_A"
                            }),
                        );
                    }
                })
            })
            .collect::<Vec<_>>();
        start.wait();
        for writer in writers {
            writer.join().expect("evidence producer should not panic");
        }

        let records = read_records(&path);
        assert_eq!(records.len(), 256);
        assert!(records.iter().all(|record| record.is_object()));
        assert!(
            records
                .iter()
                .all(|record| record["text_result"] == "TASK175D_SHARED_A")
        );
        let _ = fs::remove_file(path.as_ref());
    }

    #[test]
    fn environment_gate_has_no_file_side_effect_when_unset() {
        let _environment_guard = environment_lock().lock().unwrap();
        let path = test_path("unset");
        unsafe { std::env::set_var("RAH_LIVE_EVIDENCE_PATH", &path) };
        unsafe { std::env::remove_var("RAH_LIVE_EVIDENCE_PATH") };
        append(&json!({"event": "should_not_be_written"}));
        assert!(!path.exists());
    }

    #[test]
    fn environment_gate_writes_valid_jsonl_when_set() {
        let _environment_guard = environment_lock().lock().unwrap();
        let path = test_path("set");
        unsafe { std::env::set_var("RAH_LIVE_EVIDENCE_PATH", &path) };
        append(&json!({"event": "should_be_written"}));
        assert_eq!(
            read_records(&path),
            vec![json!({"event": "should_be_written"})]
        );
        unsafe { std::env::remove_var("RAH_LIVE_EVIDENCE_PATH") };
        let _ = fs::remove_file(path);
    }

    #[test]
    fn failed_append_is_best_effort_and_does_not_panic() {
        let directory = std::env::temp_dir();
        append_to_path(&directory, &json!({"event": "unwritable_target"}));
        assert!(directory.is_dir());
    }
}
