mod common;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use common::{GREETER, Log, consumer, plugin, provider};
use rah_kernel::{
    KERNEL_API, Kernel, KernelError, MountOptions, PluginId, PluginManifest, PluginStatus,
    PluginStatusChanged, ServiceName,
};
use serde_json::json;

fn id(value: &str) -> PluginId {
    PluginId::new(value)
}

fn pending(missing: &[&str]) -> PluginStatus {
    PluginStatus::Pending {
        missing: missing.iter().map(|s| ServiceName::new(*s)).collect(),
    }
}

#[tokio::test]
async fn consumer_waits_for_provider_and_starts_when_it_appears() {
    let (kernel, log, seen) = (Kernel::new(), Log::default(), Log::default());
    let status = kernel
        .mount(consumer("consumer", &log, &seen), MountOptions::default())
        .await
        .unwrap();
    assert_eq!(status, pending(&["greeter"]));

    let status = kernel
        .mount(provider("provider", &log), MountOptions::default())
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);
    assert_eq!(kernel.status(&id("consumer")), Some(PluginStatus::Active));
    assert_eq!(log.take(), ["start:provider", "start:consumer"]);
    assert_eq!(seen.take(), ["consumer=provider"]);
}

#[tokio::test]
async fn unmounting_provider_stops_dependents_first_and_hot_swap_recovers() {
    let (kernel, log, seen) = (Kernel::new(), Log::default(), Log::default());
    kernel
        .mount(provider("provider-a", &log), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(consumer("consumer", &log, &seen), MountOptions::default())
        .await
        .unwrap();
    log.take();

    kernel.unmount(&id("provider-a")).await.unwrap();
    assert_eq!(log.take(), ["dispose:consumer", "dispose:provider-a"]);
    assert_eq!(kernel.status(&id("consumer")), Some(pending(&["greeter"])));
    assert_eq!(kernel.status(&id("provider-a")), None);
    assert!(kernel.handle().get(GREETER).is_none());

    kernel
        .mount(provider("provider-b", &log), MountOptions::default())
        .await
        .unwrap();
    assert_eq!(kernel.status(&id("consumer")), Some(PluginStatus::Active));
    assert_eq!(seen.take(), ["consumer=provider-a", "consumer=provider-b"]);
}

#[tokio::test]
async fn cascade_stops_transitive_dependents_in_reverse_activation_order() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let link =
        |name: &'static str, requires: Option<&'static str>, provides: Option<&'static str>| {
            let log = log.clone();
            let mut manifest = PluginManifest::new(name, "1.0.0");
            manifest = manifest.requires(requires).provides(provides);
            plugin(manifest, move |scope| {
                if let Some(service) = requires {
                    scope.inject(rah_kernel::ServiceKey::<String>::new(service))?;
                }
                if let Some(service) = provides {
                    scope.provide(
                        rah_kernel::ServiceKey::<String>::new(service),
                        Arc::new(name.to_owned()),
                    )?;
                }
                let log = log.clone();
                scope.on_dispose(move || async move { log.push(format!("dispose:{name}")) });
                Ok(())
            })
        };
    let options = MountOptions::default;
    kernel
        .mount(link("c", Some("y"), None), options())
        .await
        .unwrap();
    kernel
        .mount(link("b", Some("x"), Some("y")), options())
        .await
        .unwrap();
    kernel
        .mount(link("a", None, Some("x")), options())
        .await
        .unwrap();
    assert!(
        kernel
            .plugins()
            .iter()
            .all(|(_, s)| *s == PluginStatus::Active)
    );

    kernel.unmount(&id("a")).await.unwrap();
    assert_eq!(log.take(), ["dispose:c", "dispose:b", "dispose:a"]);
    assert_eq!(kernel.status(&id("b")), Some(pending(&["x"])));
    assert_eq!(kernel.status(&id("c")), Some(pending(&["y"])));

    kernel
        .mount(link("a", None, Some("x")), options())
        .await
        .unwrap();
    assert!(
        kernel
            .plugins()
            .iter()
            .all(|(_, s)| *s == PluginStatus::Active)
    );
}

#[tokio::test]
async fn multiple_instances_default_to_earliest_and_bindings_select() {
    let (kernel, log, seen) = (Kernel::new(), Log::default(), Log::default());
    kernel
        .mount(provider("one", &log), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(provider("two", &log), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(consumer("default", &log, &seen), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(
            consumer("bound", &log, &seen),
            MountOptions::default().bind("greeter", "two"),
        )
        .await
        .unwrap();
    let status = kernel
        .mount(
            consumer("absent", &log, &seen),
            MountOptions::default().bind("greeter", "three"),
        )
        .await
        .unwrap();
    assert_eq!(status, pending(&["greeter"]));
    assert_eq!(seen.take(), ["default=one", "bound=two"]);
    assert_eq!(kernel.handle().get(GREETER).unwrap().greet(), "one");
    assert_eq!(
        kernel
            .handle()
            .get_instance(GREETER, "two")
            .unwrap()
            .greet(),
        "two"
    );

    // Losing `one` restarts only its dependent, which falls back to `two`.
    kernel.unmount(&id("one")).await.unwrap();
    assert_eq!(seen.take(), ["default=two"]);
    assert_eq!(kernel.status(&id("bound")), Some(PluginStatus::Active));
}

#[tokio::test]
async fn dependency_cycles_are_reported_separately_from_missing_services() {
    let kernel = Kernel::new();
    let empty = |manifest: PluginManifest| plugin(manifest, |_| Ok(()));
    let manifest = |name: &str| PluginManifest::new(name, "1.0.0");
    kernel
        .mount(
            empty(manifest("a").requires(["x"]).provides(["y"])),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel
        .mount(
            empty(manifest("b").requires(["y"]).provides(["x"])),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel
        .mount(
            empty(manifest("c").requires(["z"])),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel
        .mount(
            empty(manifest("d").requires(["w"]).provides(["w"])),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let cycle = |ids: &[&str]| PluginStatus::DependencyCycle {
        cycle: ids.iter().map(|s| id(s)).collect(),
    };
    assert_eq!(kernel.status(&id("a")), Some(cycle(&["a", "b"])));
    assert_eq!(kernel.status(&id("b")), Some(cycle(&["a", "b"])));
    assert_eq!(kernel.status(&id("c")), Some(pending(&["z"])));
    assert_eq!(kernel.status(&id("d")), Some(cycle(&["d"])));
}

#[tokio::test]
async fn apply_failure_discards_registrations_and_disposes_scope() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let fail = Arc::new(AtomicBool::new(true));
    let failing = {
        let (log, fail) = (log.clone(), fail.clone());
        plugin(
            PluginManifest::new("flaky", "1.0.0").provides(["greeter"]),
            move |scope| {
                scope.provide(GREETER, Arc::new(common::Fixed("flaky".into())))?;
                let guard = common::DropGuard(log.clone(), "task-dropped");
                scope.spawn(async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                });
                let log = log.clone();
                scope.on_dispose(move || async move { log.push("effect") });
                if fail.load(Ordering::SeqCst) {
                    return Err("configuration rejected".into());
                }
                Ok(())
            },
        )
    };
    let status = kernel
        .mount(failing, MountOptions::default())
        .await
        .unwrap();
    assert_eq!(
        status,
        PluginStatus::Failed {
            message: "configuration rejected".into()
        }
    );
    assert!(kernel.handle().get(GREETER).is_none());
    assert_eq!(log.take(), ["task-dropped", "effect"]);

    fail.store(false, Ordering::SeqCst);
    let status = kernel.restart(&id("flaky")).await.unwrap();
    assert_eq!(status, PluginStatus::Active);
    assert_eq!(kernel.handle().get(GREETER).unwrap().greet(), "flaky");
}

#[tokio::test]
async fn apply_panic_marks_plugin_failed() {
    let kernel = Kernel::new();
    let panicking = plugin(PluginManifest::new("boom", "1.0.0"), |_| panic!("boom"));
    let status = kernel
        .mount(panicking, MountOptions::default())
        .await
        .unwrap();
    assert_eq!(
        status,
        PluginStatus::Failed {
            message: "apply panicked".into()
        }
    );
}

#[tokio::test]
async fn update_config_restarts_plugin_and_its_dependents() {
    let (kernel, log, seen) = (Kernel::new(), Log::default(), Log::default());
    kernel
        .mount(
            provider("provider", &log),
            MountOptions::with_config(json!({"greeting": "hello"})),
        )
        .await
        .unwrap();
    kernel
        .mount(consumer("consumer", &log, &seen), MountOptions::default())
        .await
        .unwrap();
    log.take();

    let status = kernel
        .update_config(&id("provider"), json!({"greeting": "bonjour"}))
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);
    assert_eq!(
        log.take(),
        [
            "dispose:consumer",
            "dispose:provider",
            "start:provider",
            "start:consumer"
        ]
    );
    assert_eq!(seen.take(), ["consumer=hello", "consumer=bonjour"]);
}

#[tokio::test]
async fn shutdown_disposes_in_reverse_activation_order_and_rejects_mounts() {
    let (kernel, log, seen) = (Kernel::new(), Log::default(), Log::default());
    kernel
        .mount(provider("p", &log), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(consumer("c", &log, &seen), MountOptions::default())
        .await
        .unwrap();
    log.take();
    kernel.shutdown().await.unwrap();
    assert_eq!(log.take(), ["dispose:c", "dispose:p"]);
    assert!(kernel.plugins().is_empty());
    assert_eq!(
        kernel
            .mount(provider("p", &log), MountOptions::default())
            .await,
        Err(KernelError::ShutDown)
    );
}

#[tokio::test]
async fn lifecycle_operations_inside_apply_or_dispose_are_rejected() {
    let kernel = Kernel::new();
    let outcomes = Log::default();
    let nested = {
        let outcomes = outcomes.clone();
        plugin(PluginManifest::new("nested", "1.0.0"), move |scope| {
            let handle = scope.kernel();
            let child = plugin(PluginManifest::new("child", "1.0.0"), |_| Ok(()));
            let result = futures::executor::block_on(handle.mount(child, MountOptions::default()));
            outcomes.push(format!("apply:{result:?}"));
            let outcomes = outcomes.clone();
            scope.on_dispose(move || async move {
                let result = handle.unmount(&PluginId::new("other")).await;
                outcomes.push(format!("dispose:{result:?}"));
            });
            Ok(())
        })
    };
    kernel.mount(nested, MountOptions::default()).await.unwrap();
    kernel.unmount(&id("nested")).await.unwrap();
    assert_eq!(
        outcomes.take(),
        ["apply:Err(Reentrant)", "dispose:Err(Reentrant)"]
    );
}

#[tokio::test]
async fn manifests_and_mounts_are_validated() {
    let kernel = Kernel::new();
    let empty = |manifest: PluginManifest| plugin(manifest, |_| Ok(()));
    kernel
        .mount(
            empty(PluginManifest::new("ok", "1.0.0")),
            MountOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        kernel
            .mount(
                empty(PluginManifest::new("ok", "1.0.0")),
                MountOptions::default()
            )
            .await,
        Err(KernelError::DuplicatePlugin(id("ok")))
    );
    let mut future_api = PluginManifest::new("future", "1.0.0");
    future_api.kernel_api = KERNEL_API + 1;
    assert!(matches!(
        kernel
            .mount(empty(future_api), MountOptions::default())
            .await,
        Err(KernelError::IncompatibleApi { .. })
    ));
    for (manifest, options) in [
        (PluginManifest::new("bad id!", "1"), MountOptions::default()),
        (
            PluginManifest::new("both", "1")
                .requires(["x"])
                .optional(["x"]),
            MountOptions::default(),
        ),
        (
            PluginManifest::new("dup", "1").requires(["x", "x"]),
            MountOptions::default(),
        ),
        (
            PluginManifest::new("unbound", "1"),
            MountOptions::default().bind("model", "a"),
        ),
    ] {
        assert!(matches!(
            kernel.mount(empty(manifest), options).await,
            Err(KernelError::InvalidManifest { .. })
        ));
    }
    assert_eq!(
        kernel.unmount(&id("missing")).await,
        Err(KernelError::UnknownPlugin(id("missing")))
    );
}

#[tokio::test]
async fn status_changes_are_emitted_as_events() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let events = Log::default();
    let _subscription = {
        let events = events.clone();
        kernel
            .handle()
            .subscribe(move |change: &PluginStatusChanged| {
                events.push(format!("{}:{:?}", change.plugin, change.status));
            })
    };
    kernel
        .mount(provider("p", &log), MountOptions::default())
        .await
        .unwrap();
    kernel.unmount(&id("p")).await.unwrap();
    assert_eq!(events.take(), ["p:Starting", "p:Active", "p:Stopping"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_lifecycle_operations_are_serialized() {
    let kernel = Arc::new(Kernel::new());
    let (log, seen) = (Log::default(), Log::default());
    let mut tasks = Vec::new();
    for index in 0..8 {
        let (kernel, log, seen) = (kernel.clone(), log.clone(), seen.clone());
        tasks.push(tokio::spawn(async move {
            let name = format!("consumer-{index}");
            kernel
                .mount(consumer(&name, &log, &seen), MountOptions::default())
                .await
                .unwrap();
        }));
    }
    {
        let (kernel, log) = (kernel.clone(), log.clone());
        tasks.push(tokio::spawn(async move {
            kernel
                .mount(provider("provider", &log), MountOptions::default())
                .await
                .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert!(
        kernel
            .plugins()
            .iter()
            .all(|(_, s)| *s == PluginStatus::Active)
    );
    assert_eq!(seen.take().len(), 8);

    kernel.unmount(&id("provider")).await.unwrap();
    let disposals = log
        .take()
        .into_iter()
        .filter(|l| l.starts_with("dispose:"))
        .collect::<Vec<_>>();
    assert_eq!(disposals.len(), 9);
    assert_eq!(
        disposals.last().map(String::as_str),
        Some("dispose:provider")
    );
}

#[tokio::test]
async fn get_all_lists_instances_oldest_first_without_binding() {
    let (kernel, log) = (Kernel::new(), Log::default());
    kernel
        .mount(provider("one", &log), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(provider("two", &log), MountOptions::default())
        .await
        .unwrap();
    let all: Vec<(String, String)> = kernel
        .handle()
        .get_all(GREETER)
        .into_iter()
        .map(|(instance, greeter)| (instance, greeter.greet()))
        .collect();
    assert_eq!(
        all,
        [
            ("one".to_owned(), "one".to_owned()),
            ("two".to_owned(), "two".to_owned())
        ]
    );
    kernel.unmount(&id("one")).await.unwrap();
    assert_eq!(kernel.handle().get_all(GREETER).len(), 1);
}
