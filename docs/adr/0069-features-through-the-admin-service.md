# ADR 0069: Features Through The Admin Service

Status: Accepted

Date: 2026-11-09

## Context

ADR 0068 turned five plugins of the base composition into features
(`titles`, `skills`, `scheduler`, `telemetry`, `plugin-builder`), joining
`shell`, `format`, `lsp`, `compact`, `instructions`, `todo`, `plan` and
`web-search`. Only `web-search` had a switch in the Web UI, written for that
one feature in `frontend-web`. Turning any other feature on meant editing
`rah.toml` and restarting.

Features are defined in the distribution (`rah-cli`): which plugin each
mounts, where, and which policy entries it needs. A frontend cannot apply one
without that knowledge, and plugins must not depend on `rah-cli`.

## Decision

1. `rah-api::admin::PluginAdmin` gains two operations with refusing
   defaults, so other `admin` providers are unaffected:

   ```rust
   fn features(&self) -> Vec<FeatureInfo>;            // name, summary, enabled
   async fn set_feature(&self, name: &str, enabled: bool) -> Result<(), AdminError>;
   ```

2. The `rah` distribution applies a feature at runtime as the web search
   switch does: on adds the feature's policy entries to every running policy
   first (an entry the policy already has is kept), then starts a stopped
   instance of its plugin or mounts one; a feature that swaps a plugin
   (`plan`) replaces it. A feature whose required plugin (such as a sandbox)
   is not running is refused. Off unmounts the plugin, or swaps back, and
   leaves the policy entries, which then match no tool. `enabled` means the
   feature's plugin is running.
3. `set_feature` does not persist; the caller persists with
   `ConfigChange::Feature` (ADR 0067) once the feature took effect.
4. `frontend-web` serves `GET /api/features` and `PUT /api/features/{name}
   {enabled}`, and Settings gets a **Features** tab with one switch each.
   `web-search` keeps its switch under Permissions and `plan` its choice under
   Loop, so no capability has two controls. (Since 0.45.10 Settings has four
   tabs: Model with a "How the assistant works" section holding the loop
   choice, Assistant, Permissions, and Features & display.)

## Consequences

- Every feature can be turned on from the Web UI without a restart.
- The plugin protocol is unchanged: the process and WASM hosts do not proxy
  `admin`. The kernel and `rah-protocol` are unchanged.
- Like every `admin` operation, the switches are reachable only by the
  local, token-holding user of the Web UI, never by model output; they grant
  nothing the policy does not ask about.
