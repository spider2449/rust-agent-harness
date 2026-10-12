use rah_api::{
    CancellationToken, Kernel, MountOptions,
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{ModelRequest, SessionId, ToolCall, ToolCallId, ToolInput};

use super::*;

#[test]
fn front_matter_is_optional() {
    let (name, description, body) =
        parse("---\nname: commit\ndescription: \"Writes commits\"\n---\nBody\n");
    assert_eq!(
        (name.as_deref(), description.as_deref(), body),
        (Some("commit"), Some("Writes commits"), "Body\n")
    );
    let (name, description, body) = parse("# Title\nBody\n");
    assert_eq!((name, description, body), (None, None, "# Title\nBody\n"));
    let (name, _, body) = parse("---\nname: open\nno closing marker\n");
    assert_eq!((name, body), (None, "---\nname: open\nno closing marker\n"));
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn library(name: &str) -> TempDir {
    let dir =
        TempDir(std::env::temp_dir().join(format!("rah-skills-{name}-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    let skill = |directory: &str, text: &str| {
        std::fs::create_dir_all(dir.0.join(directory)).unwrap();
        std::fs::write(dir.0.join(directory).join("SKILL.md"), text).unwrap();
    };
    skill(
        "commit",
        "---\nname: commit-message\ndescription: Writes commit messages.\n---\nUse the imperative mood.\n",
    );
    skill(
        "review",
        "\n# Review code carefully\n\nCheck tests first.\n",
    );
    skill("Bad Name", "ignored: invalid directory name");
    skill("huge", &"x".repeat(200));
    std::fs::create_dir_all(dir.0.join("empty")).unwrap();
    dir
}

async fn mounted(dir: &TempDir) -> Kernel {
    let kernel = Kernel::new();
    let config = json!({ "dir": dir.0, "max_skill_bytes": 100 });
    kernel
        .mount(
            Arc::new(SkillsPlugin::new("skills")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    kernel
}

#[tokio::test]
async fn requests_list_the_skills_after_leading_system_items() {
    let dir = library("list");
    let kernel = mounted(&dir).await;
    let request = ModelRequest {
        items: vec![
            ContextItem::System {
                text: "prompt".into(),
            },
            ContextItem::User { text: "hi".into() },
        ],
        ..ModelRequest::default()
    };
    let items = kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request)
        .await
        .unwrap()
        .items;
    assert_eq!(items.len(), 3);
    let ContextItem::System { text } = &items[1] else {
        panic!("{items:?}")
    };
    assert!(
        text.contains("- commit-message: Writes commit messages.\n"),
        "{text}"
    );
    assert!(text.contains("- review: Review code carefully\n"), "{text}");
    assert!(
        !text.contains("huge") && !text.contains("Bad") && !text.contains("empty"),
        "{text}"
    );
}

#[tokio::test]
async fn skills_load_on_demand_by_declared_or_directory_name() {
    let dir = library("load");
    let kernel = mounted(&dir).await;
    let invoker = ToolInvoker::new(kernel.handle());
    let load = |name: &str| {
        let invoker = invoker.clone();
        let name = name.to_owned();
        async move {
            invoker
                .invoke(
                    Invocation {
                        session: SessionId::new(),
                        call: ToolCall {
                            id: ToolCallId::new(),
                            name: ToolName::new("skills.load"),
                            input: ToolInput(json!({ "name": name })),
                        },
                        cancel: CancellationToken::new(),
                    },
                    || {},
                )
                .await
                .output
        }
    };
    assert_eq!(
        load("commit-message").await.content,
        vec![ToolContent::Text("Use the imperative mood.".into())]
    );
    assert_eq!(
        load("review").await.content,
        vec![ToolContent::Text(
            "# Review code carefully\n\nCheck tests first.".into()
        )]
    );
    for missing in ["huge", "../commit", "nope"] {
        assert!(load(missing).await.is_error, "{missing}");
    }
}

#[tokio::test]
async fn a_missing_directory_adds_nothing() {
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(SkillsPlugin::new("skills")),
            MountOptions::with_config(json!({ "dir": "rah-no-such-skills-dir" })),
        )
        .await
        .unwrap();
    let request = ModelRequest {
        items: vec![ContextItem::User { text: "hi".into() }],
        ..ModelRequest::default()
    };
    let items = kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request.clone())
        .await
        .unwrap()
        .items;
    assert_eq!(items, request.items);
}
