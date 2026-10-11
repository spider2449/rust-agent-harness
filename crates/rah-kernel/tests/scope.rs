mod common;

use std::sync::{Arc, Mutex};

use common::{DropGuard, Fixed, GREETER, Log, plugin};
use rah_kernel::{
    Kernel, MountOptions, PluginId, PluginManifest, PluginStatus, ScopeError, ServiceKey, Spawner,
};
use serde::Deserialize;
use serde_json::json;

fn failed_with(status: PluginStatus, error: ScopeError) {
    assert_eq!(
        status,
        PluginStatus::Failed {
            message: error.to_string()
        }
    );
}

#[tokio::test]
async fn undeclared_provide_and_inject_fail_the_plugin() {
    let kernel = Kernel::new();
    let rogue_provider = plugin(PluginManifest::new("rogue-provider", "1"), |scope| {
        scope.provide(GREETER, Arc::new(Fixed("x".into())))?;
        Ok(())
    });
    failed_with(
        kernel
            .mount(rogue_provider, MountOptions::default())
            .await
            .unwrap(),
        ScopeError::UndeclaredProvide("greeter".into()),
    );

    kernel
        .mount(
            common::provider("provider", &Log::default()),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let rogue_consumer = plugin(PluginManifest::new("rogue-consumer", "1"), |scope| {
        scope.inject(GREETER)?;
        Ok(())
    });
    failed_with(
        kernel
            .mount(rogue_consumer, MountOptions::default())
            .await
            .unwrap(),
        ScopeError::UndeclaredInject("greeter".into()),
    );
}

#[tokio::test]
async fn optional_injection_tolerates_absence_but_binds_when_present() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let seen = Log::default();
    let watcher = {
        let seen = seen.clone();
        plugin(
            PluginManifest::new("watcher", "1").optional(["greeter"]),
            move |scope| {
                let greeting = scope.inject_optional(GREETER)?.map(|g| g.greet());
                seen.push(format!("{greeting:?}"));
                Ok(())
            },
        )
    };
    kernel
        .mount(watcher, MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(common::provider("provider", &log), MountOptions::default())
        .await
        .unwrap();
    kernel.restart(&PluginId::new("watcher")).await.unwrap();
    // Bound to the provider now, so losing it restarts the watcher.
    kernel.unmount(&PluginId::new("provider")).await.unwrap();
    assert_eq!(seen.take(), ["None", "Some(\"provider\")", "None"]);
}

#[tokio::test]
async fn service_type_mismatch_and_duplicate_instances_are_rejected() {
    let kernel = Kernel::new();
    kernel
        .mount(
            common::provider("provider", &Log::default()),
            MountOptions::default(),
        )
        .await
        .unwrap();
    const WRONG: ServiceKey<String> = ServiceKey::new("greeter");
    let mismatched = plugin(
        PluginManifest::new("mismatched", "1").requires(["greeter"]),
        |scope| {
            scope.inject(WRONG)?;
            Ok(())
        },
    );
    let status = kernel
        .mount(mismatched, MountOptions::default())
        .await
        .unwrap();
    assert!(
        matches!(&status, PluginStatus::Failed { message } if message.contains("registered as")),
        "{status:?}"
    );

    let duplicate = plugin(
        PluginManifest::new("duplicate", "1").provides(["greeter"]),
        |scope| {
            scope.provide_instance(GREETER, "provider", Arc::new(Fixed("dup".into())))?;
            Ok(())
        },
    );
    failed_with(
        kernel
            .mount(duplicate, MountOptions::default())
            .await
            .unwrap(),
        ScopeError::DuplicateInstance {
            service: "greeter".into(),
            instance: "provider".into(),
        },
    );
}

#[tokio::test]
async fn dispose_cancels_owned_tasks_then_runs_effects_in_reverse_order() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let spawner_slot: Arc<Mutex<Option<Spawner>>> = Arc::default();
    let worker = {
        let (log, spawner_slot) = (log.clone(), spawner_slot.clone());
        plugin(PluginManifest::new("worker", "1"), move |scope| {
            let guard = DropGuard(log.clone(), "task-dropped");
            assert!(scope.spawn(async move {
                let _guard = guard;
                std::future::pending::<()>().await;
            }));
            *spawner_slot.lock().unwrap() = Some(scope.spawner());
            for name in ["first-effect", "second-effect"] {
                let log = log.clone();
                scope.on_dispose(move || async move { log.push(name) });
            }
            Ok(())
        })
    };
    kernel.mount(worker, MountOptions::default()).await.unwrap();
    tokio::task::yield_now().await;
    assert!(log.snapshot().is_empty());

    kernel.unmount(&PluginId::new("worker")).await.unwrap();
    assert_eq!(
        log.take(),
        ["task-dropped", "second-effect", "first-effect"]
    );
    let spawner = spawner_slot.lock().unwrap().take().unwrap();
    assert!(spawner.cancellation().is_cancelled());
    assert!(!spawner.spawn(async {}));
}

#[tokio::test]
async fn typed_configuration_is_available_to_apply() {
    #[derive(Deserialize)]
    struct Config {
        endpoint: String,
        retries: u8,
    }
    let kernel = Kernel::new();
    let seen = Log::default();
    let configured = {
        let seen = seen.clone();
        plugin(PluginManifest::new("configured", "1"), move |scope| {
            let config: Config = scope.config_as()?;
            seen.push(format!("{}#{}", config.endpoint, config.retries));
            Ok(())
        })
    };
    let options =
        MountOptions::with_config(json!({"endpoint": "http://127.0.0.1:8080", "retries": 3}));
    kernel.mount(configured, options).await.unwrap();
    assert_eq!(seen.take(), ["http://127.0.0.1:8080#3"]);

    let status = kernel
        .update_config(&PluginId::new("configured"), json!({"endpoint": 1}))
        .await
        .unwrap();
    assert!(matches!(status, PluginStatus::Failed { .. }));
}
