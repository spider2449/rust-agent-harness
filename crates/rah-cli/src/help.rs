//! Help for the configuration keys of the plugins that do not carry their own
//! (`<Plugin>::CONFIG_HELP`), shown by the Web UI's configuration editor.
//! A test requires a help entry for every key of every plugin's
//! `CONFIG_EXAMPLE`, so a new key cannot go undocumented.

use crate::{PROCESS_PLUGIN, WASM_PLUGIN};

type Help = &'static [(&'static str, &'static str)];

/// Help for `kind`, as `(key, help)` pairs in display order.
pub fn fallback(kind: &str) -> Help {
    match kind {
        "context-compact" => &[
            (
                "trigger_chars",
                "History size above which old items are summarized (60000).",
            ),
            (
                "keep_chars",
                "Newest history kept as it is after summarizing (20000).",
            ),
            (
                "chunk_chars",
                "Largest piece of history sent to the model per summary request (40000).",
            ),
            ("max_summary_chars", "Longest summary kept (6000)."),
            (
                "timeout_secs",
                "Time limit of one summary request (120). On failure the older summary is kept.",
            ),
            (
                "model",
                "Provider model id used for summaries; the mounted model's default when unset.",
            ),
            (
                "instructions",
                "Replaces the built-in summarizing instructions.",
            ),
        ],
        "context-default" => &[
            ("system", "System prompt, placed first in every request."),
            ("max_items", "Newest history items sent to the model (200)."),
            (
                "max_chars",
                "Newest history characters sent to the model (200000).",
            ),
            (
                "memory_dir",
                "Directory of memory notes, one file per workspace (relative to the data directory).",
            ),
            (
                "memory_file",
                "One memory file for everything, instead of `memory_dir`.",
            ),
            (
                "max_memory_bytes",
                "Largest memory read into a request (16384).",
            ),
        ],
        "frontend-cli" => &[
            ("session", "Session id to resume."),
            ("prompt", "Run this one prompt, then exit."),
        ],
        "frontend-web" => &[
            (
                "bind",
                "Address of the Web UI; loopback only, port 0 picks a free one (127.0.0.1:8765).",
            ),
            (
                "approval_timeout_secs",
                "How long an approval waits for your answer before it is denied (300).",
            ),
            (
                "mode",
                "Mode to enter at start, such as `daily` or `coding`.",
            ),
            ("modes", "Your own modes; replaces the built-in ones."),
        ],
        "loop-plan" => &[
            ("max_steps", "Most plan steps run (8)."),
            (
                "max_rounds_per_step",
                "Most model requests that may call tools per step (16).",
            ),
        ],
        "loop-react" => &[("max_rounds", "Most model requests per turn (32).")],
        "model-anthropic" => &[
            ("model", "Anthropic model id."),
            (
                "api_key_env",
                "Name of the environment variable that holds the API key, not the key itself.",
            ),
            (
                "endpoint",
                "Official endpoint or a loopback URL only; credentials never go elsewhere.",
            ),
            ("max_tokens", "Most tokens in one reply."),
            (
                "effort",
                "Reasoning effort sent with the request; the model's default when unset.",
            ),
            (
                "refusal_fallback",
                "Rerun a declined request on a fallback model where supported (true).",
            ),
        ],
        "model-chat-completions" => &[
            (
                "provider",
                "Known server that fills in defaults: `deepseek` or `llamacpp`.",
            ),
            (
                "base_url",
                "Server address; requests go to `<base_url>/chat/completions`.",
            ),
            ("model", "Model id; the sole served model when unset."),
            (
                "api_key_env",
                "Name of the environment variable that holds the API key, not the key itself.",
            ),
            ("max_tokens", "Most tokens in one reply (8192)."),
            ("temperature", "Sampling temperature (0.7)."),
        ],
        "model-llamacpp" => &[
            (
                "endpoint",
                "llama-server address; a loopback origin only (http://127.0.0.1:8080).",
            ),
            (
                "model",
                "Served model id; the sole served model when unset.",
            ),
            (
                "api_key_env",
                "Name of the environment variable that holds the key (RAH_LLAMA_CPP_API_KEY); optional.",
            ),
        ],
        "model-mock" => &[("label", "Name shown in replies.")],
        "model-openai" => &[
            ("model", "OpenAI model id."),
            (
                "api_key_env",
                "Name of the environment variable that holds the API key, not the key itself.",
            ),
            (
                "endpoint",
                "Official endpoint or a loopback URL only; credentials never go elsewhere.",
            ),
        ],
        "policy-authority" => &[
            (
                "allow",
                "Permission levels run without asking (none, read).",
            ),
            (
                "ask",
                "Permission levels that ask you first (write, execute).",
            ),
            (
                "tools",
                "Permission level per tool: an exact name or `prefix*`. A tool without one is denied.",
            ),
            (
                "decisions",
                "Fixed `allow`, `ask` or `deny` per tool name or `prefix*`; replaces the level rules for it.",
            ),
            ("max_in_flight", "Most tool calls running at once (128)."),
        ],
        "scheduler-basic" => &[
            ("max_depth", "How deep subagents may nest (2)."),
            ("max_concurrent", "Most jobs running at once (2)."),
            (
                "delegate_timeout_secs",
                "Time limit of one `agent.delegate` call (600).",
            ),
            (
                "jobs",
                "Background prompts: `input`, `delay_secs`, `every_secs` and an optional `session`.",
            ),
        ],
        "session-jsonl" => &[(
            "dir",
            "Where session logs are written (sessions, in the data directory).",
        )],
        "session-titles" => &[("timeout_secs", "Time limit of one title request (60).")],
        "skills-fs" => &[
            (
                "dir",
                "Directory of skills, each `<name>/SKILL.md` (skills, in the data directory).",
            ),
            ("max_skills", "Most skills listed to the model (64)."),
            ("max_skill_bytes", "Largest skill file read (65536)."),
        ],
        "storage-fs" => &[
            (
                "dir",
                "Where plugin state is kept, one file per entry (storage, in the data directory).",
            ),
            ("max_value_bytes", "Largest single entry (16777216)."),
        ],
        "telemetry-log" => &[
            (
                "dir",
                "Where daily JSONL files go (telemetry, in the data directory).",
            ),
            (
                "max_file_bytes",
                "Size at which a day's file continues in a new one (10485760).",
            ),
            ("keep_days", "Files older than this are deleted (14)."),
            (
                "include_arguments",
                "Also record tool arguments (false); message text is never recorded.",
            ),
        ],
        "tools-builtin" => &[
            (
                "root",
                "Workspace the file tools can see; they cannot leave it.",
            ),
            (
                "max_bytes",
                "Largest file `fs.read` and `fs.grep` handle (262144).",
            ),
            ("max_results", "Most lines a search returns (500)."),
            ("max_scan", "Most entries a search visits (20000)."),
            (
                "skip_dirs",
                "Directories listed but not searched (.git, target, node_modules).",
            ),
        ],
        "tools-http" => &[
            ("base_url", "Address every tool path is joined to."),
            ("headers", "Fixed request headers."),
            (
                "header_env",
                "Request headers read from environment variables: `{ header = \"VAR\" }`.",
            ),
            ("timeout_secs", "Time limit of one request (15)."),
            ("max_chars", "Longest response kept (20000)."),
            (
                "tools",
                "The tools: `name`, `description`, `method`, `path`, `permission` and typed `params`.",
            ),
        ],
        "tools-mcp" => &[
            (
                "server",
                "Name of the server; tools appear as `mcp.<server>.<tool>`.",
            ),
            ("program", "Server program, started through the sandbox."),
            ("args", "Arguments of the server program."),
            (
                "tools",
                "Tools to expose and the permission level of each; others stay hidden.",
            ),
            ("call_timeout_ms", "Time limit of one call (30000)."),
            (
                "startup_timeout_ms",
                "Time limit for the server to start (10000).",
            ),
            ("cwd", "Working directory, relative to the sandbox root."),
            ("env", "Environment variables passed to the server."),
        ],
        "tools-mcp-http" => &[
            (
                "server",
                "Name of the server; tools appear as `mcp.<server>.<tool>`.",
            ),
            ("url", "Server address: https, or http for a loopback host."),
            (
                "bearer_token_env",
                "Name of the environment variable that holds the token, not the token itself.",
            ),
            (
                "tools",
                "Tools to expose and the permission level of each; others stay hidden.",
            ),
            ("call_timeout_ms", "Time limit of one call (30000)."),
        ],
        "tools-repo" => &[
            ("git", "Git program to run (git)."),
            ("timeout_ms", "Time limit of one Git run (15000)."),
            ("max_output_bytes", "Largest Git output kept (1048576)."),
            ("max_results", "Most paths or lines returned (500)."),
            ("cwd", "Repository directory, relative to the sandbox root."),
        ],
        "tools-repo-write" => &[
            (
                "root",
                "Repository the write tools may change; they cannot leave it.",
            ),
            ("git", "Git program to run (git)."),
            (
                "max_file_bytes",
                "Largest file written or edited (1048576).",
            ),
            ("timeout_ms", "Time limit of one Git run (30000)."),
            ("cwd", "Repository directory, relative to the sandbox root."),
        ],
        "tools-web-search" => &[
            (
                "engines",
                "Search engines in failover order: duckduckgo, bing, searxng.",
            ),
            ("searxng", "SearXNG instance addresses."),
            ("max_results", "Most results per search (8)."),
            ("timeout_secs", "Time limit of one request (15)."),
            ("accept_language", "Language header sent to the engines."),
            (
                "fetch_max_chars",
                "Longest page text `web.fetch` returns (20000).",
            ),
        ],
        PROCESS_PLUGIN => &[
            (
                "program",
                "Program that runs the plugin; JSON-RPC over stdio.",
            ),
            ("args", "Arguments of the program."),
            ("plugin", "Settings passed on to the plugin itself."),
        ],
        WASM_PLUGIN => &[
            ("path", "The .wasm file."),
            ("fuel", "Most work one call may do before it is stopped."),
            ("max_memory_mb", "Memory limit in MiB."),
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use crate::{PROCESS_PLUGIN, WASM_PLUGIN, catalog, config_example, config_help};

    #[test]
    fn every_example_key_has_help() {
        let mut kinds: Vec<String> = catalog().keys().map(|kind| kind.to_string()).collect();
        kinds.push(PROCESS_PLUGIN.to_owned());
        kinds.push(WASM_PLUGIN.to_owned());
        for kind in kinds {
            let Some(example) = config_example(&kind) else {
                continue;
            };
            let example: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(example).unwrap();
            let help = config_help(&kind);
            for key in example.keys() {
                assert!(
                    help.iter().any(|(name, _)| name == key),
                    "`{kind}` has no help for `{key}`"
                );
            }
        }
    }
}
