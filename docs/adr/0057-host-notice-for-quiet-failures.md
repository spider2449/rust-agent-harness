# ADR 0057: A `HostNotice` Event For Failures A Plugin Skips Quietly

Status: Accepted

Date: 2026-10-19

## Context

`format-on-write` and `lsp-diagnostics` try a default formatter or language
server for a file's extension and skip it without a word when it fails. That
is deliberate: a missing `gofmt` is the normal case for someone who writes
Rust, and a note about it on every write would only add noise to the model's
context (ADR 0048, ADR 0053).

The cost showed on 2026-10-19. With the feature on and nothing happening, the
cause took many steps to find: a `rustup` kept outside the home directory, and
a toolchain without the `rust-analyzer` component. Only an explicitly
configured program reports its errors, so the user had to know to configure
one to see why the default did not work.

## Decision

1. **`rah_api::host::HostNotice {plugin, message}`** is an event a plugin
   emits to tell the user something it cannot show itself. It is not a log:
   it is for a cause the user can act on.
2. **The host or a frontend shows it to the user, never to the model.** `rah`
   prints `rah: <plugin>: <message>` on standard error, and the Web UI shows it
   as a notice (so `rah-desktop`, which embeds the Web UI, shows it too).
   Nothing is added to a tool's output, so the model's context does not change.
3. **`NoticeOnce`** (also in `rah_api::host`) emits a notice once per key
   for the life of a plugin instance. A default formatter or server that
   fails is reported the first time, with the reason (bounded and flattened
   to one line), and then stays quiet. Mounting the plugin again, for
   example after a configuration change, reports it again.
4. **Only defaults report this way.** A formatter or server the user named
   in the plugin's configuration keeps reporting its errors in the tool's
   output as before.
5. **What triggers a notice:** a default program that could not be started, did
   not finish in time, or exited with an error. A program that is simply not
   used for the file (no matching extension, a project that does not use it)
   is not a failure.

## Consequences

- One new event type in `rah-api`; the kernel, the hook contracts and the
  plugin protocol do not change. A process plugin can emit nothing of this
  kind yet.
- The notice goes to whoever is watching standard error or the Web UI, which
  is the user of that run. In a headless run it is a line on standard error.
- The message names the program and the reason, and says how to see every
  error or turn the program off. It never includes file contents.
