//! The host tells the user when the kernel stopped waiting for a plugin to
//! stop (ADR 0076).

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use rah_api::{
    Kernel, MountOptions, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    host::HostNotice,
};

struct NeverStops(PluginManifest);

#[async_trait]
impl Plugin for NeverStops {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.on_dispose(std::future::pending::<()>);
        Ok(())
    }
}

#[tokio::test]
async fn a_plugin_that_does_not_stop_in_time_is_reported_as_a_host_notice() {
    let kernel = Kernel::with_dispose_limit(Duration::from_millis(50));
    let _overruns = rah_cli::report_dispose_overruns(&kernel);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let _notices = kernel
        .handle()
        .subscribe(move |notice: &HostNotice| sink.lock().unwrap().push(notice.clone()));
    let plugin = Arc::new(NeverStops(PluginManifest::new("slow", "1")));
    kernel.mount(plugin, MountOptions::default()).await.unwrap();

    kernel.unmount(&PluginId::new("slow")).await.unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].plugin, PluginId::new("slow"));
    assert!(
        seen[0]
            .message
            .contains("did not stop in time (0 task(s) and 1 cleanup step(s)"),
        "{}",
        seen[0].message
    );
}
