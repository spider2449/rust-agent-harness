//! Events exchanged between plugins and the host process.

/// Emitted by a frontend to ask the host process to shut down and exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitRequested {
    pub code: i32,
}

/// Emitted by a frontend that listens for connections, so the host can show
/// where. `url` may carry an access secret; show it only to the local user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendListening {
    pub plugin: rah_kernel::PluginId,
    pub url: String,
}

/// Offered by the host on an interrupt from the user (Ctrl-C), before it
/// exits. A frontend that cancels running work in response returns
/// `Some(())`, and the host keeps running; without such an answer it exits.
pub struct Interrupt;

impl rah_kernel::BailHook for Interrupt {
    const NAME: &'static str = "host.interrupt";
    type Input = ();
    type Output = ();
}

/// Emitted by a plugin to tell the user something it cannot show itself, such
/// as a default formatter that cannot start (ADR 0057). The host prints it and
/// the Web UI shows it as a notice; it never reaches the model. It is for a
/// cause the user can act on, not a log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostNotice {
    pub plugin: rah_kernel::PluginId,
    /// One line.
    pub message: String,
}

/// The longest notice shown.
const NOTICE_CHARS: usize = 300;

/// Emits a [`HostNotice`] once per key for the life of a plugin instance.
pub struct NoticeOnce {
    kernel: rah_kernel::KernelHandle,
    plugin: rah_kernel::PluginId,
    reported: std::sync::Mutex<std::collections::BTreeSet<String>>,
}

impl NoticeOnce {
    pub fn new(kernel: rah_kernel::KernelHandle, plugin: rah_kernel::PluginId) -> Self {
        Self {
            kernel,
            plugin,
            reported: Default::default(),
        }
    }

    /// Emits `message` unless `key` was reported before. The message is
    /// flattened to one line and bounded.
    pub fn emit(&self, key: &str, message: &str) {
        let first = self
            .reported
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key.to_owned());
        if !first {
            return;
        }
        let message = message
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(NOTICE_CHARS)
            .collect();
        self.kernel.emit(&HostNotice {
            plugin: self.plugin.clone(),
            message,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn a_notice_is_emitted_once_per_key_as_one_bounded_line() {
        let kernel = rah_kernel::Kernel::new();
        let seen = Arc::new(Mutex::new(Vec::<HostNotice>::new()));
        let sink = seen.clone();
        let _guard = kernel
            .handle()
            .subscribe(move |notice: &HostNotice| sink.lock().unwrap().push(notice.clone()));
        let notices = NoticeOnce::new(kernel.handle(), rah_kernel::PluginId::new("p"));
        notices.emit("a", "first\nline\twith controls");
        notices.emit("a", "the same cause again");
        notices.emit("b", &"x".repeat(1000));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert_eq!(seen[0].plugin, rah_kernel::PluginId::new("p"));
        assert_eq!(seen[0].message, "first line with controls");
        assert_eq!(seen[1].message.chars().count(), NOTICE_CHARS);
    }
}
