use futures::executor::block_on;
use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
use rah_tools::{EchoTool, Tool, ToolContext};
use serde_json::json;

async fn assert_tool_conformance(
    tool: &dyn Tool,
    valid_input: ToolInput,
    invalid_input: ToolInput,
) {
    let definition = tool.definition();
    assert!(!definition.name.as_str().is_empty());
    assert!(!definition.description.is_empty());
    assert!(definition.input_schema.is_object());
    assert_eq!(definition.permission, PermissionLevel::None);

    let output = tool
        .execute(valid_input, ToolContext::default())
        .await
        .expect("conforming tool accepts valid input");
    assert!(!output.is_error);
    assert!(!output.content.is_empty());
    assert!(
        tool.execute(invalid_input, ToolContext::default())
            .await
            .is_err()
    );
}

#[test]
fn echo_satisfies_tool_contract() {
    block_on(async {
        let tool = EchoTool::new();
        assert_tool_conformance(
            &tool,
            ToolInput(json!({ "text": "hello" })),
            ToolInput(json!({ "text": 7 })),
        )
        .await;
        let output = tool
            .execute(
                ToolInput(json!({ "text": "hello" })),
                ToolContext::default(),
            )
            .await
            .expect("echo output");
        assert_eq!(output.content, [ToolContent::Text("hello".to_owned())]);
    });
}
