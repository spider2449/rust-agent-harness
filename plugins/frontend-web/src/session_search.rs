//! Read-only history search through the mounted session service.
use crate::{ApiResult, AppState, sessions};
use axum::extract::{Query, State};
use rah_protocol::SessionEntry;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    q: String,
}

fn excerpt(text: &str, query: &str) -> Option<String> {
    let folded = text.to_lowercase();
    let matched_byte = folded.find(query)?;
    let chars: Vec<char> = text.chars().collect();
    let width = query.chars().count();
    // Map the lowercase byte position back to the original character. A
    // character can expand during lowercase conversion (for example U+0130).
    // Contextual lowercase forms have the same UTF-8 length as their simple
    // forms, so counting bytes also handles the Greek final sigma.
    let mut folded_bytes = 0;
    let start = chars.iter().position(|character| {
        folded_bytes += character.to_lowercase().map(char::len_utf8).sum::<usize>();
        folded_bytes > matched_byte
    })?;
    let left = start.saturating_sub(40);
    let right = (start + width + 100).min(chars.len());
    Some(format!(
        "{}{}{}",
        if left > 0 { "\u{2026}" } else { "" },
        chars[left..right].iter().collect::<String>(),
        if right < chars.len() { "\u{2026}" } else { "" }
    ))
}

pub(crate) async fn search(
    State(shared): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult {
    let query = query.q.trim().to_lowercase();
    if query.is_empty() {
        return Ok(axum::Json(json!([])));
    }
    if query.chars().count() > 256 {
        return Err(crate::ApiError(
            axum::http::StatusCode::BAD_REQUEST,
            "search query exceeds 256 characters".into(),
        ));
    }
    let log = sessions(&shared)?;
    let mut infos = log.list().await?;
    infos.sort_by_key(|info| std::cmp::Reverse(info.updated.max(info.created)));
    let mut results = Vec::new();
    for info in infos {
        let metadata = format!(
            "{} {} {} {}",
            info.title.as_deref().unwrap_or(""),
            info.preview.as_deref().unwrap_or(""),
            info.id,
            info.workspace.as_deref().unwrap_or("")
        );
        let mut hit = None;
        for record in log.read(&info.id).await? {
            let text = match &record.entry {
                SessionEntry::TurnStarted { input, .. } => input,
                SessionEntry::AssistantMessage { text, .. } => text,
                _ => continue,
            };
            if let Some(snippet) = excerpt(text, &query) {
                hit = Some((record.seq, snippet));
                break;
            }
        }
        if hit.is_some() || metadata.to_lowercase().contains(&query) {
            results.push(json!({"id": info.id, "seq": hit.as_ref().map(|h| h.0),
                "snippet": hit.map(|h| h.1)}));
        }
        if results.len() == 100 {
            break;
        }
    }
    Ok(axum::Json(json!(results)))
}

#[cfg(test)]
mod tests {
    use super::excerpt;
    #[test]
    #[ignore = "long-message search performance measurement"]
    fn long_message_search_latency() {
        let query = "needle".repeat(32);
        let text = format!("{}{}", "ordinary context ".repeat(65536), query);
        let start = std::time::Instant::now();
        for _ in 0..5 {
            assert!(
                excerpt(std::hint::black_box(&text), &query)
                    .unwrap()
                    .contains(&query)
            );
        }
        eprintln!(
            "five end-of-message matches in {} bytes: {:?}",
            text.len(),
            start.elapsed()
        );
    }

    #[test]
    fn excerpt_maps_expanded_and_contextual_lowercase_to_source() {
        assert_eq!(
            excerpt("\u{130}stanbul", "\u{307}st").as_deref(),
            Some("\u{130}stanbul")
        );
        assert_eq!(
            excerpt("\u{39f}\u{3a3}", "\u{3c2}").as_deref(),
            Some("\u{39f}\u{3a3}")
        );
    }

    #[test]
    fn searches_later_text_case_insensitively_and_preserves_unicode() {
        assert!(excerpt("first message", "later").is_none());
        assert_eq!(
            excerpt("before \u{6c7a}\u{7b56} after", "\u{6c7a}\u{7b56}").as_deref(),
            Some("before \u{6c7a}\u{7b56} after")
        );
        assert_eq!(
            excerpt(
                "\u{5f8c}\u{4f86}\u{6c7a}\u{5b9a}\u{4f7f}\u{7528} SQLite",
                "sqlite"
            )
            .as_deref(),
            Some("\u{5f8c}\u{4f86}\u{6c7a}\u{5b9a}\u{4f7f}\u{7528} SQLite")
        );
        assert_eq!(
            excerpt("\u{130}stanbul", "i\u{307}stanbul").as_deref(),
            Some("\u{130}stanbul")
        );
        let text = format!("{}needle{}", "\u{4e2d}".repeat(80), "\u{6587}".repeat(150));
        let found = excerpt(&text, "needle").unwrap();
        assert!(
            found.starts_with('\u{2026}')
                && found.ends_with('\u{2026}')
                && found.contains("needle")
        );
    }
}
