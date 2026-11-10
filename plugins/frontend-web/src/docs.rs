//! The Web UI's Help: the project's documentation, built into the binary so
//! it matches the running version and needs no network. `GET /api/docs`
//! lists the documents (`[{id, title, path}]`, in reading order) and
//! `GET /api/docs/{id}` returns one with its Markdown source (`markdown`).
//! `path` is the document's place in the repository, which the UI uses to
//! follow a relative link from one document to another.

use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

use crate::{ApiError, ApiResult};

struct Doc {
    id: &'static str,
    title: &'static str,
    path: &'static str,
    markdown: &'static str,
}

macro_rules! doc {
    ($id:literal, $title:literal, $path:literal) => {
        Doc {
            id: $id,
            title: $title,
            path: $path,
            markdown: include_str!(concat!("../../../", $path)),
        }
    };
}

const DOCS: &[Doc] = &[
    doc!("overview", "Overview", "README.md"),
    doc!(
        "configuration",
        "Configuring and running",
        "docs/CONFIGURATION.md"
    ),
    doc!("mcp-servers", "MCP servers", "docs/MCP_SERVERS.md"),
    doc!(
        "process-plugins",
        "Process plugins",
        "docs/PROCESS_PLUGINS.md"
    ),
    doc!("wasm-plugins", "WASM plugins", "docs/WASM_PLUGINS.md"),
    doc!("security", "Security", "docs/SECURITY.md"),
    doc!("architecture", "Architecture", "docs/ARCHITECTURE.md"),
    doc!("harness2", "harness2", "docs/HARNESS2.md"),
    doc!("known-issues", "Known issues", "docs/KNOWN_ISSUES.md"),
];

fn summary(doc: &Doc) -> Value {
    json!({ "id": doc.id, "title": doc.title, "path": doc.path })
}

/// `GET /api/docs`.
pub(crate) async fn list() -> ApiResult {
    Ok(Json(DOCS.iter().map(summary).collect()))
}

/// `GET /api/docs/{id}`.
pub(crate) async fn read(Path(id): Path<String>) -> ApiResult {
    let doc = DOCS
        .iter()
        .find(|doc| doc.id == id)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no document `{id}`")))?;
    let mut reply = summary(doc);
    reply["markdown"] = json!(doc.markdown);
    Ok(Json(reply))
}

#[cfg(test)]
mod tests {
    use super::DOCS;

    #[test]
    fn documents_have_distinct_ids_and_content() {
        for (index, doc) in DOCS.iter().enumerate() {
            assert!(!doc.markdown.trim().is_empty(), "`{}` is empty", doc.id);
            assert!(
                DOCS[..index].iter().all(|other| other.id != doc.id),
                "`{}` is listed twice",
                doc.id
            );
        }
    }
}
