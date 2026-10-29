//! Named subagents: instructions a delegation can start from.

use std::path::Path;

/// One named subagent, read from `<agents_dir>/<name>.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Agent {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

/// The most agents read from the directory.
const MAX_AGENTS: usize = 32;
/// The largest agent file read.
const MAX_AGENT_BYTES: u64 = 64 * 1024;
const MAX_DESCRIPTION_CHARS: usize = 300;

/// Agent names: 1-64 of `[a-z0-9_-]`, as skill names.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Splits optional front matter from the body: `(name, description, body)`.
/// Front matter without its closing `---` is an error.
fn parse(text: &str) -> Result<(Option<String>, Option<String>, &str), &'static str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim) != Some("---") {
        return Ok((None, None, text));
    }
    let (mut name, mut description) = (None, None);
    let mut consumed = text.split_inclusive('\n').next().map_or(0, str::len);
    for line in lines {
        consumed += line.len();
        let trimmed = line.trim();
        if trimmed == "---" {
            return Ok((name, description, &text[consumed..]));
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let value = value.trim().trim_matches('"').to_owned();
            match key.trim() {
                "name" => name = Some(value),
                "description" => description = Some(value),
                _ => {}
            }
        }
    }
    Err("its front matter has no closing `---`")
}

/// The agents in `dir`, sorted by name, and one line for each file that
/// cannot be used. A missing directory has no agents.
pub(crate) async fn read_dir(dir: &Path) -> (Vec<Agent>, Vec<String>) {
    let (mut agents, mut problems) = (Vec::new(), Vec::new());
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return (agents, problems);
    };
    let mut files = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        let shown = path.display();
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => continue,
        };
        if metadata.len() > MAX_AGENT_BYTES {
            problems.push(format!(
                "agent `{shown}` has more than {MAX_AGENT_BYTES} bytes; it is skipped"
            ));
            continue;
        }
        let text = match tokio::fs::read(&path).await.map(String::from_utf8) {
            Ok(Ok(text)) => text,
            _ => {
                problems.push(format!(
                    "agent `{shown}` is not readable UTF-8 text; it is skipped"
                ));
                continue;
            }
        };
        let (declared, description, body) = match parse(&text) {
            Ok(parts) => parts,
            Err(reason) => {
                problems.push(format!("agent `{shown}`: {reason}; it is skipped"));
                continue;
            }
        };
        let name = declared.unwrap_or(stem);
        if !valid_name(&name) {
            problems.push(format!(
                "agent `{shown}` is named `{name}`, which is not an agent name (1-64 of \
                 a-z, 0-9, `-` and `_`); it is skipped"
            ));
            continue;
        }
        let description: String = description
            .unwrap_or_default()
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect();
        if description.trim().is_empty() {
            problems.push(format!(
                "agent `{shown}` has no `description`, so the model cannot tell when to use \
                 it; it is skipped"
            ));
            continue;
        }
        if agents.iter().any(|agent: &Agent| agent.name == name) {
            problems.push(format!(
                "agent `{shown}` is named `{name}`, like an earlier agent; it is skipped"
            ));
            continue;
        }
        agents.push(Agent {
            name,
            description,
            instructions: body.trim().to_owned(),
        });
    }
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    if agents.len() > MAX_AGENTS {
        problems.push(format!(
            "`{}` has more than {MAX_AGENTS} agents; the rest are left out",
            dir.display()
        ));
        agents.truncate(MAX_AGENTS);
    }
    (agents, problems)
}

/// The first input of a subagent's session: its instructions, then the task.
pub(crate) fn input(agent: &Agent, task: &str) -> String {
    format!(
        "You are the `{}` subagent. Follow these instructions:\n\n{}\n\nYour task:\n\n{task}",
        agent.name, agent.instructions
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn usable_agents_are_read_and_the_others_are_reported() {
        let dir = std::env::temp_dir().join(format!("rah-agents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
        write(
            "b-review.md",
            "---\ndescription: Reviews.\n---\nBe strict.\n",
        );
        write(
            "a-plan.md",
            "---\nname: planner\ndescription: Plans.\n---\nList steps.\n",
        );
        write("open.md", "---\ndescription: never closed\n");
        write("Bad.md", "---\ndescription: d\n---\nx\n");
        write("bare.md", "No front matter, so no description.\n");
        write("twin.md", "---\nname: b-review\ndescription: d\n---\nx\n");
        write("notes.txt", "not an agent");

        let (agents, problems) = read_dir(&dir).await;
        let _ = std::fs::remove_dir_all(&dir);
        let names: Vec<&str> = agents.iter().map(|agent| agent.name.as_str()).collect();
        assert_eq!(names, ["b-review", "planner"]);
        assert_eq!(agents[1].instructions, "List steps.");
        for part in [
            "`Bad`, which is not an agent name",
            "has no `description`",
            "no closing `---`",
            "like an earlier agent",
        ] {
            assert_eq!(
                problems.iter().filter(|p| p.contains(part)).count(),
                1,
                "`{part}` in {problems:#?}"
            );
        }
        assert_eq!(problems.len(), 4, "{problems:#?}");
        assert!(read_dir(&dir).await.0.is_empty());
    }
}
