//! `rah config init` on a terminal (ADR 0067 §6): asks for the provider,
//! model, permissions and workspace, and returns the configuration file's
//! text. It never reads or writes an API key: it only says whether the
//! provider's key variable is set, and the file names the variable.

use std::{
    io::{self, BufRead, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    time::Duration,
};

use crate::{PROVIDERS, Permissions, Provider, user_file_text};

/// Where a local llama.cpp server answers by default.
pub const LLAMA_CPP_ADDRESS: &str = "127.0.0.1:8080";

/// Whether something accepts connections at `address` within `timeout`.
pub fn answers(address: &str, timeout: Duration) -> bool {
    address
        .parse::<SocketAddr>()
        .is_ok_and(|address| TcpStream::connect_timeout(&address, timeout).is_ok())
}

/// What the questions can say about the machine, gathered by the caller so
/// they stay testable.
pub struct Environment<'a> {
    /// Whether an environment variable is set to a non-empty value.
    pub is_set: &'a dyn Fn(&str) -> bool,
    /// Whether the local llama.cpp server answered.
    pub llama_cpp_answers: bool,
    /// Already given on the command line; not asked again.
    pub permissions: Option<Permissions>,
    pub root: Option<String>,
    pub features: &'a [String],
}

impl Environment<'_> {
    /// What the provider list says about `provider`.
    pub fn status(&self, provider: &Provider) -> String {
        match provider.name {
            "openai-compatible" => "any Chat Completions server; asks for its URL".to_owned(),
            "llamacpp" if self.llama_cpp_answers => {
                format!("a server answers at {LLAMA_CPP_ADDRESS}")
            }
            "llamacpp" => format!("nothing answers at {LLAMA_CPP_ADDRESS}"),
            "mock" => "no model; a canned reply, to try RAH out".to_owned(),
            _ => match provider.key_env {
                Some(variable) if (self.is_set)(variable) => format!("{variable} is set"),
                Some(variable) => format!("{variable} is not set"),
                None => String::new(),
            },
        }
    }

    /// The provider offered first: one whose key is set, else a running
    /// llama.cpp, else the mock model. An index into [`PROVIDERS`].
    pub fn suggested(&self) -> usize {
        let ready = |provider: &Provider| match provider.name {
            "llamacpp" => self.llama_cpp_answers,
            "openai-compatible" | "mock" => false,
            _ => provider
                .key_env
                .is_some_and(|variable| (self.is_set)(variable)),
        };
        PROVIDERS
            .iter()
            .position(ready)
            .or_else(|| {
                PROVIDERS
                    .iter()
                    .position(|provider| provider.name == "mock")
            })
            .unwrap_or(0)
    }
}

/// Accepts the URL of an OpenAI-compatible server.
fn check_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err("the URL starts with http:// or https://".to_owned())
    }
}

/// What a setup form (such as `rah-desktop`'s first start) sends back.
/// Empty strings mean "not given" and take the same defaults as the terminal
/// questions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answers {
    /// A name from [`PROVIDERS`].
    pub provider: String,
    pub model_id: String,
    /// `openai-compatible` only.
    pub base_url: String,
    /// `openai-compatible` only: the variable that holds its API key.
    pub api_key_env: String,
    pub permissions: String,
    pub root: String,
}

impl Answers {
    /// Checks the answers as the terminal questions do and returns the
    /// configuration file's text.
    pub fn text(&self, features: &[String]) -> Result<String, String> {
        let name = self.provider.trim();
        let provider = PROVIDERS
            .iter()
            .find(|provider| provider.name == name)
            .ok_or_else(|| format!("choose a provider (unknown `{name}`)"))?;
        let id = self.model_id.trim();
        let model = if id.is_empty() || provider.name == "mock" {
            provider.name.to_owned()
        } else {
            format!("{}/{id}", provider.name)
        };
        let mut options = Vec::new();
        if provider.name == "openai-compatible" {
            let url = self.base_url.trim();
            check_url(url)?;
            options.push(("base_url".to_owned(), url.to_owned()));
            let variable = self.api_key_env.trim();
            if !variable.is_empty() {
                options.push(("api_key_env".to_owned(), variable.to_owned()));
            }
        }
        let permissions = match self.permissions.trim() {
            "" => Permissions::Ask,
            permissions => permissions.parse()?,
        };
        let root = self.root.trim();
        user_file_text(
            &model,
            &options,
            permissions,
            (!root.is_empty()).then_some(root),
            features,
        )
    }
}

/// Writes a new configuration file at `path`, creating its folder. An
/// existing file is never overwritten.
pub fn write_new(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create `{}`: {error}", parent.display()))?;
    }
    // `create_new` refuses an existing file without a check-then-write race.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => already_exists(path),
            _ => format!("cannot create `{}`: {error}", path.display()),
        })?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("cannot write `{}`: {error}", path.display()))
}

/// Why a configuration file is not written.
pub fn already_exists(path: &Path) -> String {
    format!(
        "`{}` already exists; edit it with `rah config edit`, or remove it first",
        path.display()
    )
}

/// Reads one answer; end of input is an error, so a closed terminal never
/// writes a half-answered file.
fn ask<R: BufRead, W: Write>(input: &mut R, out: &mut W, question: &str) -> io::Result<String> {
    write!(out, "{question}")?;
    out.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "no answer; nothing was written",
        ));
    }
    Ok(line.trim().to_owned())
}

/// Asks until `parse` accepts the answer.
fn ask_until<R: BufRead, W: Write, T>(
    input: &mut R,
    out: &mut W,
    question: &str,
    parse: impl Fn(&str) -> Result<T, String>,
) -> io::Result<T> {
    loop {
        let answer = ask(input, out, question)?;
        match parse(&answer) {
            Ok(value) => return Ok(value),
            Err(message) => writeln!(out, "  {message}")?,
        }
    }
}

/// Asks the questions and returns the configuration file's text.
pub fn interview<R: BufRead, W: Write>(
    input: &mut R,
    out: &mut W,
    environment: &Environment<'_>,
) -> io::Result<String> {
    writeln!(out, "RAH needs a model. Providers:")?;
    for (index, provider) in PROVIDERS.iter().enumerate() {
        writeln!(
            out,
            "  {}) {:<18} {}",
            index + 1,
            provider.name,
            environment.status(provider)
        )?;
    }
    let suggested = environment.suggested();
    let provider = ask_until(
        input,
        out,
        &format!("Provider [{}]: ", suggested + 1),
        |answer| {
            if answer.is_empty() {
                return Ok(&PROVIDERS[suggested]);
            }
            answer
                .parse::<usize>()
                .ok()
                .and_then(|number| number.checked_sub(1))
                .and_then(|index| PROVIDERS.get(index))
                .or_else(|| PROVIDERS.iter().find(|provider| provider.name == answer))
                .ok_or_else(|| format!("answer 1 to {} or a provider name", PROVIDERS.len()))
        },
    )?;
    let mut model = provider.name.to_owned();
    if provider.name != "mock" {
        let id = ask(input, out, "Model id (empty: the provider's default): ")?;
        if !id.is_empty() {
            model = format!("{}/{id}", provider.name);
        }
    }
    let mut options = Vec::new();
    if provider.name == "openai-compatible" {
        let url = ask_until(
            input,
            out,
            "Server URL, such as https://host/v1: ",
            |answer| check_url(answer).map(|()| answer.to_owned()),
        )?;
        options.push(("base_url".to_owned(), url));
        let variable = ask(
            input,
            out,
            "Environment variable that holds its API key (empty: none): ",
        )?;
        if !variable.is_empty() {
            options.push(("api_key_env".to_owned(), variable));
        }
    } else if let Some(variable) = provider.key_env
        && !(environment.is_set)(variable)
        && provider.name != "llamacpp"
    {
        writeln!(
            out,
            "  Set {variable} before starting rah; the key is never written to the file."
        )?;
    }
    let permissions = match environment.permissions {
        Some(permissions) => permissions,
        None => ask_until(
            input,
            out,
            "Permissions: read-only, ask, auto-edit or full [ask]: ",
            |answer| {
                if answer.is_empty() {
                    Ok(Permissions::Ask)
                } else {
                    answer.parse()
                }
            },
        )?,
    };
    let root = match &environment.root {
        Some(root) => Some(root.clone()),
        None => {
            let root = ask(
                input,
                out,
                "Workspace folder (empty: the folder rah starts in): ",
            )?;
            (!root.is_empty()).then_some(root)
        }
    };
    user_file_text(
        &model,
        &options,
        permissions,
        root.as_deref(),
        environment.features,
    )
    .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;
    use crate::{Distribution, UserSettings};

    fn run(answers: &str, environment: &Environment<'_>) -> (io::Result<String>, String) {
        let mut out = Vec::new();
        let result = interview(&mut answers.as_bytes(), &mut out, environment);
        (result, String::from_utf8(out).unwrap())
    }

    fn environment<'a>(is_set: &'a dyn Fn(&str) -> bool, llama: bool) -> Environment<'a> {
        Environment {
            is_set,
            llama_cpp_answers: llama,
            permissions: None,
            root: None,
            features: &[],
        }
    }

    #[test]
    fn defaults_follow_what_is_ready_and_the_file_parses() {
        let anthropic_key = |name: &str| name == "ANTHROPIC_API_KEY";
        let (text, shown) = run("\n\n\n\n", &environment(&anthropic_key, false));
        let text = text.unwrap();
        assert!(shown.contains("ANTHROPIC_API_KEY is set"), "{shown}");
        assert!(shown.contains("OPENAI_API_KEY is not set"), "{shown}");
        assert!(shown.contains("Provider [1]"), "{shown}");
        let distribution = Distribution::parse(&text).unwrap();
        assert_eq!(
            distribution.user,
            Some(UserSettings {
                model: Some("anthropic".into()),
                permissions: Some(Permissions::Ask),
                root: None,
            })
        );

        // Nothing ready but a local llama.cpp: it is offered first.
        let nothing = |_: &str| false;
        let (_, shown) = run("\n\n\n\n", &environment(&nothing, true));
        let llama = PROVIDERS.iter().position(|p| p.name == "llamacpp").unwrap() + 1;
        assert!(shown.contains(&format!("Provider [{llama}]")), "{shown}");
        assert!(shown.contains("a server answers at"), "{shown}");
        // Nothing at all: the mock model.
        let (text, _) = run("\n\n\n", &environment(&nothing, false));
        assert!(text.unwrap().contains("model = \"mock\""));
    }

    #[test]
    fn answers_are_checked_and_asked_again() {
        let nothing = |_: &str| false;
        let compatible = PROVIDERS
            .iter()
            .position(|p| p.name == "openai-compatible")
            .unwrap()
            + 1;
        let answers = format!(
            "99\n{compatible}\nqwen3\nftp://x\nhttp://10.0.0.5:8000/v1\nMY_KEY\nyolo\nauto-edit\nD:/work\n"
        );
        let (text, shown) = run(&answers, &environment(&nothing, false));
        let text = text.unwrap();
        assert!(shown.contains("answer 1 to"), "{shown}");
        assert!(shown.contains("starts with http://"), "{shown}");
        assert!(shown.contains("unknown permissions `yolo`"), "{shown}");
        let distribution = Distribution::parse(&text).unwrap();
        let model = distribution
            .plugin
            .iter()
            .find(|spec| spec.kind == "model-chat-completions")
            .unwrap();
        assert_eq!(
            model.config,
            serde_json::json!({
                "model": "qwen3",
                "base_url": "http://10.0.0.5:8000/v1",
                "api_key_env": "MY_KEY"
            })
        );
        let user = distribution.user.unwrap();
        assert_eq!(user.permissions, Some(Permissions::AutoEdit));
        assert_eq!(user.root.as_deref(), Some("D:/work"));
    }

    #[test]
    fn a_missing_key_is_named_and_given_options_are_not_asked() {
        let nothing = |_: &str| false;
        let features = ["todo".to_owned()];
        let given = Environment {
            permissions: Some(Permissions::ReadOnly),
            root: Some("D:/given".into()),
            features: &features,
            ..environment(&nothing, false)
        };
        let (text, shown) = run("openai\ngpt-x\n", &given);
        let text = text.unwrap();
        assert!(shown.contains("Set OPENAI_API_KEY"), "{shown}");
        assert!(!shown.contains("Permissions:"), "{shown}");
        assert!(!shown.contains("Workspace folder"), "{shown}");
        assert!(text.contains("model = \"openai/gpt-x\""), "{text}");
        assert!(text.contains("permissions = \"read-only\""), "{text}");
        assert!(text.contains("root = \"D:/given\""), "{text}");
        assert!(text.contains("features = [\"todo\"]"), "{text}");
    }

    #[test]
    fn form_answers_are_checked_like_the_questions() {
        let compatible = Answers {
            provider: "openai-compatible".into(),
            model_id: " qwen3 ".into(),
            base_url: "http://10.0.0.5:8000/v1".into(),
            api_key_env: "MY_KEY".into(),
            permissions: "auto-edit".into(),
            root: "D:/work".into(),
        };
        let distribution = Distribution::parse(&compatible.text(&[]).unwrap()).unwrap();
        let user = distribution.user.clone().unwrap();
        assert_eq!(user.model.as_deref(), Some("openai-compatible/qwen3"));
        assert_eq!(user.permissions, Some(Permissions::AutoEdit));
        assert_eq!(user.root.as_deref(), Some("D:/work"));

        // Empty answers take the defaults; a mock model takes no id.
        let mock = Answers {
            provider: "mock".into(),
            model_id: "ignored".into(),
            ..Answers::default()
        };
        let text = mock.text(&["todo".to_owned()]).unwrap();
        assert!(text.contains("model = \"mock\""), "{text}");
        assert!(text.contains("permissions = \"ask\""), "{text}");
        assert!(text.contains("features = [\"todo\"]"), "{text}");

        let refused = [
            Answers::default(),
            Answers {
                base_url: "ftp://x".into(),
                ..compatible.clone()
            },
            Answers {
                permissions: "yolo".into(),
                ..compatible
            },
        ];
        for answers in refused {
            assert!(answers.text(&[]).is_err(), "{answers:?}");
        }
    }

    #[test]
    fn a_new_file_is_written_once() {
        let dir = std::env::temp_dir().join(format!("rah-setup-{}", std::process::id()));
        let path = dir.join("nested").join("rah.toml");
        let _ = std::fs::remove_dir_all(&dir);
        write_new(&path, "model = \"mock\"\n").unwrap();
        let again = write_new(&path, "model = \"openai\"\n").unwrap_err();
        assert!(again.contains("already exists"), "{again}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "model = \"mock\"\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn end_of_input_writes_nothing() {
        let nothing = |_: &str| false;
        let (text, _) = run("1\n", &environment(&nothing, false));
        assert_eq!(text.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn a_listening_port_answers_and_a_closed_one_does_not() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        assert!(answers(&address, Duration::from_secs(2)));
        drop(listener);
        assert!(!answers("not an address", Duration::from_millis(10)));
    }
}
