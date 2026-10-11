mod common;

use common::{Log, plugin};
use rah_kernel::{
    BailHook, HookError, Kernel, MountOptions, PluginId, PluginManifest, SerialHook, WaterfallHook,
};

struct Decide;
impl BailHook for Decide {
    const NAME: &'static str = "test.decide";
    type Input = String;
    type Output = String;
}

struct Rewrite;
impl WaterfallHook for Rewrite {
    const NAME: &'static str = "test.rewrite";
    type Value = String;
}

struct Notify;
impl SerialHook for Notify {
    const NAME: &'static str = "test.notify";
    type Input = u32;
}

#[derive(Debug, PartialEq)]
struct Ping(u32);

#[tokio::test]
async fn bail_runs_by_priority_until_a_handler_decides() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let hooks = {
        let log = log.clone();
        plugin(PluginManifest::new("hooks", "1"), move |scope| {
            for (priority, name, decision) in [
                (1, "low", Some("low")),
                (10, "high", None),
                (5, "mid-a", Some("mid-a")),
                (5, "mid-b", Some("mid-b")),
            ] {
                let log = log.clone();
                scope.hook_bail::<Decide, _, _>(priority, move |input: String| {
                    log.push(format!("{name}:{input}"));
                    async move { decision.map(str::to_owned) }
                });
            }
            Ok(())
        })
    };
    kernel.mount(hooks, MountOptions::default()).await.unwrap();
    let handle = kernel.handle();
    assert_eq!(
        handle.bail::<Decide>("req".into()).await,
        Ok(Some("mid-a".to_owned()))
    );
    assert_eq!(log.take(), ["high:req", "mid-a:req"]);

    kernel.unmount(&PluginId::new("hooks")).await.unwrap();
    assert_eq!(handle.bail::<Decide>("req".into()).await, Ok(None));
}

#[tokio::test]
async fn waterfall_and_serial_run_every_handler_in_priority_order() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let hooks = {
        let log = log.clone();
        plugin(PluginManifest::new("hooks", "1"), move |scope| {
            scope.hook_waterfall::<Rewrite, _, _>(0, |v: String| async move { v + "b" });
            scope.hook_waterfall::<Rewrite, _, _>(10, |v: String| async move { v + "a" });
            for (priority, name) in [(0, "second"), (3, "first")] {
                let log = log.clone();
                scope.hook_serial::<Notify, _, _>(priority, move |n: u32| {
                    log.push(format!("{name}:{n}"));
                    async {}
                });
            }
            Ok(())
        })
    };
    let handle = kernel.handle();
    assert_eq!(
        handle.waterfall::<Rewrite>("x".into()).await,
        Ok("x".to_owned())
    );
    kernel.mount(hooks, MountOptions::default()).await.unwrap();
    assert_eq!(
        handle.waterfall::<Rewrite>("x".into()).await,
        Ok("xab".to_owned())
    );
    handle.serial::<Notify>(7).await.unwrap();
    assert_eq!(log.take(), ["first:7", "second:7"]);
}

#[tokio::test]
async fn panicking_hook_handlers_surface_as_errors() {
    let kernel = Kernel::new();
    let hooks = plugin(PluginManifest::new("broken", "1"), |scope| {
        scope.hook_bail::<Decide, _, _>(0, |_input: String| async move {
            panic!("policy bug");
        });
        scope.hook_waterfall::<Rewrite, _, _>(0, |_v: String| -> std::future::Ready<String> {
            panic!("synchronous panic")
        });
        Ok(())
    });
    kernel.mount(hooks, MountOptions::default()).await.unwrap();
    let handle = kernel.handle();
    assert_eq!(
        handle.bail::<Decide>("req".into()).await,
        Err(HookError::Panicked {
            hook: "test.decide",
            owner: "broken".into()
        })
    );
    assert!(matches!(
        handle.waterfall::<Rewrite>("x".into()).await,
        Err(HookError::Panicked {
            hook: "test.rewrite",
            ..
        })
    ));
}

#[tokio::test]
async fn hooks_report_kernel_gone_after_drop() {
    let kernel = Kernel::new();
    let handle = kernel.handle();
    drop(kernel);
    assert_eq!(
        handle.bail::<Decide>("req".into()).await,
        Err(HookError::KernelGone)
    );
}

#[tokio::test]
async fn plugin_event_handlers_follow_scope_and_contain_panics() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let listeners = {
        let log = log.clone();
        plugin(PluginManifest::new("listeners", "1"), move |scope| {
            scope.on(|_: &Ping| panic!("listener bug"));
            let log = log.clone();
            scope.on(move |ping: &Ping| log.push(format!("plugin:{}", ping.0)));
            Ok(())
        })
    };
    kernel
        .mount(listeners, MountOptions::default())
        .await
        .unwrap();
    let handle = kernel.handle();
    handle.emit(&Ping(1));
    kernel.unmount(&PluginId::new("listeners")).await.unwrap();
    handle.emit(&Ping(2));
    assert_eq!(log.take(), ["plugin:1"]);
}

#[tokio::test]
async fn host_subscriptions_end_when_dropped() {
    let (kernel, log) = (Kernel::new(), Log::default());
    let handle = kernel.handle();
    let subscription = {
        let log = log.clone();
        handle.subscribe(move |ping: &Ping| log.push(format!("host:{}", ping.0)))
    };
    handle.emit(&Ping(1));
    drop(subscription);
    handle.emit(&Ping(2));
    assert_eq!(log.take(), ["host:1"]);
}

#[tokio::test]
async fn has_handlers_tracks_registered_hooks() {
    let kernel = Kernel::new();
    let handle = kernel.handle();
    assert!(!handle.has_handlers::<Decide>());
    let hooks = plugin(PluginManifest::new("hooks", "1"), |scope| {
        scope.hook_bail::<Decide, _, _>(0, |_input: String| async { None });
        Ok(())
    });
    kernel.mount(hooks, MountOptions::default()).await.unwrap();
    assert!(handle.has_handlers::<Decide>());
    kernel.unmount(&PluginId::new("hooks")).await.unwrap();
    assert!(!handle.has_handlers::<Decide>());
}
