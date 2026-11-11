//! The first start of `rah-desktop`: with no configuration file, the window
//! shows a form with the questions `rah config init` asks on a terminal
//! (ADR 0067 §6), writes `rah.toml` from the answers and starts RAH. Like the
//! terminal questions, it never reads or writes an API key; it only says
//! whether the provider's key variable is set.

use rah_cli::{
    PROVIDERS, Permissions,
    setup::{Answers, Environment},
};
use serde_json::Value;

pub(crate) const APP_CSS: &str = include_str!("../../../plugins/frontend-web/assets/app.css");

/// `text` safe inside HTML text and double-quoted attributes.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The setup form. `root` prefills the workspace folder.
pub fn page(title: &str, environment: &Environment<'_>, root: &str) -> String {
    let suggested = environment.suggested();
    let providers: String = PROVIDERS
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            format!(
                "<label class=\"choice\"><input type=\"radio\" name=\"provider\" value=\"{name}\"{checked}> \
                 <b>{name}</b> <span>{status}</span></label>",
                name = escape(provider.name),
                checked = if index == suggested { " checked" } else { "" },
                status = escape(&environment.status(provider)),
            )
        })
        .collect();
    let chosen = environment.permissions.unwrap_or(Permissions::Ask);
    let permissions: String = Permissions::ALL
        .iter()
        .map(|permissions| {
            format!(
                "<option{selected}>{name}</option>",
                selected = if *permissions == chosen {
                    " selected"
                } else {
                    ""
                },
                name = permissions.name(),
            )
        })
        .collect();
    let title = escape(title);
    let root = escape(root);
    format!(
        r#"<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title><style>{APP_CSS}</style>
<body class="startup-page"><main class="startup-card">
<h1>Welcome to {title}</h1>
<p>RAH needs a model. These answers are written to <code>rah.toml</code>; Settings can change them later.</p>
<form id="setup">
<fieldset><legend>Provider</legend>{providers}</fieldset>
<fieldset><legend>Model id</legend><input type="text" name="model_id" placeholder="empty: the provider's default">
<p class="hint">API keys are read from the variable shown above and never written to the file.</p></fieldset>
<fieldset id="compatible" hidden><legend>Server</legend>
<input type="text" name="base_url" placeholder="https://host/v1">
<p class="hint">Environment variable that holds its API key (empty: none)</p>
<input type="text" name="api_key_env"></fieldset>
<fieldset><legend>Permissions</legend><select name="permissions">{permissions}</select>
<p class="hint">read-only: tools only read · ask: every change asks · auto-edit: edits run, programs ask · full: nothing asks</p></fieldset>
<fieldset><legend>Workspace folder</legend><input type="text" name="root" value="{root}"></fieldset>
<p id="error" role="alert"></p>
<button id="start" type="submit">Start</button>
</form>
</main>
<script>
const form = document.getElementById("setup");
const compatible = document.getElementById("compatible");
function shown() {{
  compatible.hidden = form.provider.value !== "openai-compatible";
}}
form.addEventListener("change", shown);
shown();
form.addEventListener("submit", (event) => {{
  event.preventDefault();
  document.getElementById("error").textContent = "";
  document.getElementById("start").disabled = true;
  window.ipc.postMessage(JSON.stringify(Object.fromEntries(new FormData(form))));
}});
function setupFailed(message) {{
  document.getElementById("error").textContent = message;
  document.getElementById("start").disabled = false;
}}
</script></body></html>"#
    )
}

/// The form's answers, sent as a JSON object of strings.
pub fn answers(body: &str) -> Result<Answers, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| format!("unreadable answers: {error}"))?;
    let field = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    Ok(Answers {
        provider: field("provider"),
        model_id: field("model_id"),
        base_url: field("base_url"),
        api_key_env: field("api_key_env"),
        permissions: field("permissions"),
        root: field("root"),
    })
}

/// A script call that shows `message` under the form.
pub fn failed_script(message: &str) -> String {
    format!("setupFailed({})", Value::from(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment<'a>(is_set: &'a dyn Fn(&str) -> bool) -> Environment<'a> {
        Environment {
            is_set,
            llama_cpp_answers: false,
            permissions: Some(Permissions::AutoEdit),
            root: None,
            features: &[],
        }
    }

    #[test]
    fn the_form_offers_every_provider_and_the_ready_one_first() {
        let key = |name: &str| name == "OPENAI_API_KEY";
        let page = page("RAH 1", &environment(&key), "C:\\Users\\a <b>");
        for provider in &PROVIDERS {
            assert!(page.contains(&format!("value=\"{}\"", provider.name)));
        }
        assert!(page.contains("value=\"openai\" checked"), "{page}");
        assert!(page.contains("OPENAI_API_KEY is set"));
        assert!(page.contains("<option selected>auto-edit</option>"));
        assert!(page.contains("value=\"C:\\Users\\a &lt;b&gt;\""));
    }

    #[test]
    fn answers_are_read_from_the_form() {
        let answers =
            answers(r#"{"provider":"llamacpp","model_id":"qwen","permissions":"ask","extra":1}"#)
                .unwrap();
        assert_eq!(answers.provider, "llamacpp");
        assert_eq!(answers.model_id, "qwen");
        assert_eq!(answers.root, "");
        assert!(super::answers("not json").is_err());
        assert_eq!(failed_script("a \"b\""), r#"setupFailed("a \"b\"")"#);
    }
}
