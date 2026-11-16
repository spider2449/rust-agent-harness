//! Deferred native keyboard focus recovery after window activation.

#[derive(Default)]
pub(crate) struct Recovery {
    pending: bool,
}

impl Recovery {
    pub(crate) fn changed(&mut self, focused: bool) {
        self.pending = focused;
    }

    pub(crate) fn settle(&mut self, minimized: bool, mut focus: impl FnMut() -> bool) {
        if self.pending && !minimized && focus() {
            self.pending = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Recovery;

    #[test]
    fn activation_waits_for_settle_and_runs_only_once() {
        let mut recovery = Recovery::default();
        let mut calls = 0;
        recovery.changed(true);
        assert_eq!(calls, 0);
        for _ in 0..2 {
            recovery.settle(false, || {
                calls += 1;
                true
            });
        }
        assert_eq!(calls, 1);
    }

    #[test]
    fn minimized_and_failed_handoffs_remain_pending() {
        let mut recovery = Recovery::default();
        recovery.changed(true);
        recovery.settle(true, || panic!("minimized windows must not take focus"));
        recovery.settle(false, || false);
        let mut restored = false;
        recovery.settle(false, || {
            restored = true;
            true
        });
        assert!(restored);
    }

    #[test]
    fn deactivation_cancels_pending_focus() {
        let mut recovery = Recovery::default();
        recovery.changed(true);
        recovery.changed(false);
        recovery.settle(false, || panic!("inactive windows must not take focus"));
        recovery.changed(true);
        let mut restored = false;
        recovery.settle(false, || {
            restored = true;
            true
        });
        assert!(restored);
    }
}
