# ADR 0067: One User-Facing Configuration

Status: Proposed

Date: 2026-11-06

## Context

Running RAH means understanding its plugin structure. What a start runs is
composed from five sources, in this order:

1. a built-in preset (`--preset`), or `rah.toml` in the data directory when it
   exists (silently), or the `default` preset;
2. opt-in features (`features = [...]`, `--with`);
3. command-line flags that each set one key of one plugin (`--prompt` sets
   `frontend-cli.prompt`, `--session-dir` sets `session-jsonl.dir`, ...);
4. `--approve`, which adds exact tool names to the policy;
5. the Web UI's stored settings (`storage/frontend-web/settings-*`, Plugins
   panel changes, the web search switch), which replace the configuration of
   whole plugins at the next start.

Each layer was reasonable when it was added (ADRs 0054, 0058, 0059, 0066), but
together they leave the user without one place that says what runs. Editing
`rah.toml` can have no effect because a stored Web UI setting replaces it. The
`default` preset runs llama.cpp, so a first `rah` without a local llama.cpp
server fails. Choosing a provider means choosing a preset whose name does not
say it fixes the provider. `rah --help` lists 21 flags, most of them tagged with
the plugin they configure.

The plugin kernel (ADR 0036) is not the problem: plugins should keep receiving
one `config` value at mount. The problem is that the distribution layer
(`rah-cli`) exposes plugin composition as the user interface.

## Decision

### 1. `rah.toml` is the single source of what runs

`rah.toml` in the data directory (or `--config`) is the only persistent
configuration. Nothing else replaces plugin configuration at start. The Web UI
saves its settings into this file (section 5), not into `storage`.

`storage` keeps UI state that is not configuration: preferences, layout,
recent workspaces, the active mode.

### 2. Top-level user keys

A distribution file gains top-level keys that describe intent rather than
plugins:

```toml
model       = "anthropic/claude-sonnet-5-5"  # <provider>/<model id>
root        = "D:/work/project"              # workspace; default: current directory
permissions = "ask"                           # read-only | ask | auto-edit | full
features    = ["shell=git,cargo", "web-search"]

[model_options]                               # optional, provider-specific
api_key_env = "MY_KEY"
```

- **`model`** selects the model plugin and its `model` key from the provider
  prefix: `anthropic` (`model-anthropic`), `openai` (`model-openai`),
  `deepseek` (`model-chat-completions` with `provider = "deepseek"`),
  `openai-compatible` (`model-chat-completions`; `model_options.base_url`
  names the server), `llamacpp` (`model-llamacpp`) and `mock`
  (`model-mock`, no id). Without an id the plugin's default model is used. An unknown provider is refused at parse time with
  the list of known ones. `model_options` is merged into that plugin's
  configuration. API keys stay in environment variables; the file only names
  the variable.
- **`permissions`** compiles to `policy-authority`'s `allow` and `ask`:

  | mode        | allow                              | ask                 |
  | ----------- | ---------------------------------- | ------------------- |
  | `read-only` | `none`, `read`                     | (writes denied)     |
  | `ask`       | `none`, `read`                     | `write`, `execute`  |
  | `auto-edit` | `none`, `read`, `write`            | `execute`           |
  | `full`      | `none`, `read`, `write`, `execute` |                     |

  Every mode keeps the policy mounted and fail-closed: a tool without a
  permission assignment is still denied, and `decisions` and approved
  `commands` still apply. `full` prints a warning at every start, like
  `--approve` does today. Running without a policy remains possible only by
  removing `policy-authority` in an advanced `[[plugin]]` section, and still
  prints the `UNGUARDED` warning.
- **`root`** sets the root of every workspace plugin, as `--root` does.
- **`features`** is unchanged (opt-in capabilities, applied after the
  composition).

### 3. Composition

A file with any top-level user key is composed as:

1. the guarded base composition (today's guarded preset, without a model);
2. the model plugin from `model` and `model_options`;
3. `permissions` and `root`;
4. `[[plugin]]` entries and `remove`, with the change semantics of ADR 0058
   (implicit `extends`);
5. `features`.

A file without user keys keeps today's meaning: a whole plugin list, or
`extends = "<preset>"` with changes. Existing files keep working unchanged.

Built-in presets stop being a user concept and `--preset` is removed. The
preset names remain valid for `extends` and for tests.

### 4. Command line

Command-line flags override the file for one run and never write it. The
surface becomes subcommands with a few global flags:

```text
rah                      terminal REPL
rah run "<prompt>"       one prompt and exit (--summary, --session)
rah web                  Web UI (--bind, --mode)
rah sessions list|export <id> --format md|json|trace
rah config show|check|init|edit
rah features

global: --model <provider/id> --permissions <mode> --root <dir>
        --with <feature> --config <file> --data-dir <dir>
```

`rah config show` prints the user keys and where each value came from (file,
flag, default) by default; `--plugins` adds the plugin list with
configurations. The old flags (`--preset`, `--prompt`, `--web`, `--export`,
`--show-config`, `--check-config`, `--init-config`, `--list-features`,
`--approve`, `--session-dir`) are removed without aliases. External executors
(harness2) move to `rah run`; `CHANGELOG.md` lists the old and new forms.

### 5. Settings saved from the Web UI go to `rah.toml`

The `admin` service (`rah-api::admin::PluginAdmin`) gains one operation that
asks the distribution to persist a change:

```rust
async fn persist(&self, change: ConfigChange) -> Result<PathBuf, AdminError>;

pub enum ConfigChange {
    Model { model: String, options: serde_json::Map<String, Value> },
    Permissions(String),
    Feature { name: String, enabled: bool },
    /// A `[[plugin]]` change in the sense of ADR 0058, by instance id.
    Plugin { id: PluginId, config: Value },
    /// An approved command (ADR 0066), added to the policy's `commands`.
    Command(Vec<String>),
}
```

The distribution plugin edits the file with `toml_edit` so comments and order
are kept, writes it atomically, and refuses when the start used `--config`
pointing at a read-only file or a file without user keys (the change would
need an `extends` rewrite the user did not ask for). `frontend-web` applies a
change at runtime through the existing admin operations first, and persists it
only once the plugin is active, as it stores settings today.

Existing stored settings are not applied any more. At the first start that
finds them, RAH reports them through a `HostNotice` and `rah config migrate`
folds them into `rah.toml` and removes them.

### 6. First start

Without `rah.toml`, `--config` and `--model`, `rah` does not start a model it
guessed:

- on a terminal, it runs `rah config init` interactively: it lists the
  providers, marks the ones whose key variable is set and whether a local
  llama.cpp answers, asks for the model id, permissions and root, and writes
  `rah.toml`;
- otherwise it exits with status 2 and a message naming `--model` and
  `rah config init`.

## Consequences

- The user reads one file to know what runs, and the Web UI and the file can
  no longer disagree.
- `rah-api` changes: `PluginAdmin::persist` and `ConfigChange`. Process
  plugins do not get `admin` unless they require it, as before. The kernel,
  `rah-protocol` and the plugin protocol are unchanged; plugins still receive
  one `config` at mount.
- `rah-cli` gains a dependency edge on `toml_edit` (already in the lock file
  through `toml`), owned by section 5.
- Scripts using the old flags break; `CHANGELOG.md` maps each one to its
  replacement.
- `permissions = "full"` makes approval prompts disappear for assigned tools.
  It does not grant unassigned tools and does not bypass hooks; `SECURITY.md`
  documents it. It replaces `--approve` (ADR 0054), whose per-tool form is
  still available as the policy's `approve` key in an advanced `[[plugin]]`
  section.
- Supersedes the parts of ADR 0058 that make presets the user's starting
  point, and the storage-backed settings of the Web UI (ADR 0066's
  persistence moves to `rah.toml`; its approval behavior is unchanged).
