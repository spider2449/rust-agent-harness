//! Shared structural assertions for opt-in live-gate examples.
//!
//! These helpers deliberately accept no model text. Live-gate success is
//! determined by host-observed lifecycle and postcondition evidence.

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
