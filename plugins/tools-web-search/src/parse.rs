//! Result extraction from engine responses.
//!
//! The HTML engines are scraped with a small tag scanner instead of a full
//! HTML parser: it only needs anchor and paragraph text from well-known
//! result blocks, and it never executes or follows anything it reads.

use serde_json::Value;

/// One normalized search hit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub url: String,
    pub title: String,
    pub snippet: String,
}

/// Maximum characters kept from one snippet.
pub const SNIPPET_CHARS: usize = 300;
/// Maximum characters kept from one title.
const TITLE_CHARS: usize = 200;

/// DuckDuckGo's HTML endpoint: `a.result__a` anchors carry the title and a
/// `uddg` redirect URL, `.result__snippet` the snippet of the latest result.
pub fn duckduckgo(html: &str) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    let mut rest = html;
    while let Some((tag, inner, after)) = next_element(rest, &["a"]) {
        rest = after;
        let class = attribute(tag, "class").unwrap_or_default();
        if has_class(&class, "result__a") {
            let Some(url) = attribute(tag, "href").and_then(|href| duckduckgo_target(&href)) else {
                continue;
            };
            hits.push(Hit {
                url,
                title: clean(inner, TITLE_CHARS),
                snippet: String::new(),
            });
        } else if has_class(&class, "result__snippet")
            && let Some(last) = hits.last_mut()
            && last.snippet.is_empty()
        {
            last.snippet = clean(inner, SNIPPET_CHARS);
        }
    }
    hits.retain(|hit| !hit.title.is_empty());
    hits
}

/// Resolves a DuckDuckGo result link to its target. Ads (`y.js`) are dropped.
fn duckduckgo_target(href: &str) -> Option<String> {
    let href = decode_entities(href);
    if href.contains("/y.js") {
        return None;
    }
    let target = match query_param(&href, "uddg") {
        Some(encoded) => percent_decode(&encoded),
        None => href,
    };
    web_url(target)
}

/// Bing: each `li.b_algo` block holds an `h2 > a` title link and a `p`
/// snippet.
pub fn bing(html: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for block in html.split("class=\"b_algo").skip(1) {
        let block = block.split("class=\"b_algo").next().unwrap_or(block);
        let Some(heading) = block.find("<h2") else {
            continue;
        };
        let Some((tag, inner, after)) = next_element(&block[heading..], &["a"]) else {
            continue;
        };
        let Some(url) = attribute(tag, "href").and_then(|href| bing_target(&href)) else {
            continue;
        };
        let snippet = next_element(after, &["p"])
            .map(|(_, inner, _)| clean(inner, SNIPPET_CHARS))
            .unwrap_or_default();
        let title = clean(inner, TITLE_CHARS);
        if !title.is_empty() {
            hits.push(Hit {
                url,
                title,
                snippet,
            });
        }
    }
    hits
}

/// Resolves a Bing `ck/a` tracking link (`u=a1<base64url>`) to its target.
fn bing_target(href: &str) -> Option<String> {
    let href = decode_entities(href);
    if href.contains("bing.com/ck/a")
        && let Some(encoded) = query_param(&href, "u")
        && let Some(payload) = encoded.strip_prefix("a1")
        && let Some(bytes) = base64url_decode(payload)
        && let Ok(target) = String::from_utf8(bytes)
    {
        return web_url(target);
    }
    web_url(href)
}

/// SearXNG `format=json`: `results[]` with `url`, `title` and `content`.
pub fn searxng(body: &Value) -> Option<Vec<Hit>> {
    let results = body.get("results")?.as_array()?;
    Some(
        results
            .iter()
            .filter_map(|result| {
                let url = web_url(result.get("url")?.as_str()?.to_owned())?;
                let text = |field: &str| result.get(field).and_then(Value::as_str);
                Some(Hit {
                    url,
                    title: clean(text("title").unwrap_or_default(), TITLE_CHARS),
                    snippet: clean(text("content").unwrap_or_default(), SNIPPET_CHARS),
                })
            })
            .filter(|hit| !hit.title.is_empty())
            .collect(),
    )
}

/// Whether a scraped page is a bot check rather than a result page.
pub fn is_bot_wall(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    [
        "anomaly-modal",
        "captcha",
        "unusual traffic",
        "are you a robot",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Keeps only absolute `http`/`https` URLs.
fn web_url(url: String) -> Option<String> {
    let url = url.trim().to_owned();
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")).then_some(url)
}

/// Finds the next element named one of `names`. Returns its opening tag, its
/// inner HTML (up to the matching close tag, not nesting-aware) and the text
/// after it.
fn next_element<'a>(html: &'a str, names: &[&str]) -> Option<(&'a str, &'a str, &'a str)> {
    let mut offset = 0;
    while let Some(found) = html[offset..].find('<') {
        let start = offset + found;
        let rest = &html[start + 1..];
        let name_len = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        let name = &rest[..name_len];
        let tag_end = rest.find('>')?;
        if names.iter().any(|wanted| wanted.eq_ignore_ascii_case(name)) {
            let tag = &html[start..start + 1 + tag_end + 1];
            let body = &html[start + 1 + tag_end + 1..];
            let close = format!("</{name}");
            let close_at = find_ignore_case(body, &close).unwrap_or(body.len());
            let after = body[close_at..]
                .find('>')
                .map_or("", |end| &body[close_at + end + 1..]);
            return Some((tag, &body[..close_at], after));
        }
        offset = start + 1;
    }
    None
}

fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// Reads a quoted attribute value from an opening tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut search = 0;
    while let Some(found) = find_ignore_case(&tag[search..], name) {
        let at = search + found;
        search = at + name.len();
        let boundary = at > 0 && bytes[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        if !boundary {
            continue;
        }
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        if quote == '"' || quote == '\'' {
            let value = &rest[1..];
            return value.find(quote).map(|end| value[..end].to_owned());
        }
        let end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '>')
            .unwrap_or(rest.len());
        return Some(rest[..end].to_owned());
    }
    None
}

fn has_class(classes: &str, wanted: &str) -> bool {
    classes
        .split_ascii_whitespace()
        .any(|class| class == wanted)
}

/// Reads a query parameter (raw, still percent-encoded) from a URL.
fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    let query = query.split('#').next().unwrap_or(query);
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

/// Strips tags, decodes entities, collapses whitespace, neutralizes angle
/// brackets (so page text cannot forge an `untrusted-web-content` tag) and
/// truncates to `limit` characters.
pub fn clean(html: &str, limit: usize) -> String {
    // Inline tags join their text; any other tag separates words.
    const INLINE: [&str; 10] = [
        "a", "b", "em", "i", "mark", "small", "span", "strong", "u", "wbr",
    ];
    let mut text = String::with_capacity(html.len());
    let mut tag: Option<String> = None;
    for c in html.chars() {
        match (&mut tag, c) {
            (None, '<') => tag = Some(String::new()),
            (Some(name), '>') => {
                let name = name.trim_start_matches('/');
                let name = name.split(|c: char| !c.is_ascii_alphanumeric()).next();
                if !INLINE.iter().any(|inline| Some(*inline) == name) {
                    text.push(' ');
                }
                tag = None;
            }
            (Some(name), c) => name.push(c.to_ascii_lowercase()),
            (None, c) => text.push(c),
        }
    }
    let decoded = neutralize(&decode_entities(&text));
    let collapsed = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&collapsed, limit)
}

/// Converts a page to readable text: the `<title>`, and the body with
/// scripts, styles and other non-text elements dropped, block elements on
/// their own lines and list items as `- ` lines. Angle brackets in the text
/// are neutralized like [`clean`] does.
pub fn html_to_text(html: &str) -> (Option<String>, String) {
    const SKIP: [&str; 9] = [
        "head", "iframe", "noscript", "object", "script", "style", "svg", "template", "title",
    ];
    const BLOCK: [&str; 32] = [
        "address",
        "article",
        "aside",
        "blockquote",
        "br",
        "dd",
        "details",
        "div",
        "dl",
        "dt",
        "figcaption",
        "figure",
        "footer",
        "form",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hr",
        "main",
        "nav",
        "ol",
        "p",
        "pre",
        "section",
        "summary",
        "table",
        "tr",
        "ul",
    ];
    let title = next_element(html, &["title"]).map(|(_, inner, _)| clean(inner, 300));
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        // A `<` that does not start a tag is text.
        if !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!') {
            out.push('<');
            rest = &rest[1..];
            continue;
        }
        let Some(end) = rest.find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| !c.is_ascii_alphanumeric())
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !closing && !tag.ends_with('/') && SKIP.contains(&name.as_str()) {
            rest = match find_ignore_case(rest, &format!("</{name}")) {
                Some(close) => rest[close..]
                    .find('>')
                    .map_or("", |end| &rest[close + end + 1..]),
                None => "",
            };
            continue;
        }
        if name == "li" && !closing {
            out.push_str("\n- ");
        } else if BLOCK.contains(&name.as_str()) {
            out.push('\n');
        } else if matches!(name.as_str(), "td" | "th") {
            out.push(' ');
        }
    }
    out.push_str(rest);
    let text = neutralize(&decode_entities(&out));
    // At most one blank line between paragraphs, none between list items,
    // and a bullet whose text starts on a later line joins it.
    let mut lines: Vec<String> = Vec::new();
    let (mut blank, mut bullet) = (false, false);
    for line in text.lines() {
        let mut line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line == "-" {
            bullet = true;
            continue;
        }
        if line.is_empty() {
            blank = true;
            continue;
        }
        if std::mem::take(&mut bullet) {
            line = format!("- {line}");
        }
        let list_continues =
            line.starts_with("- ") && lines.last().is_some_and(|last| last.starts_with("- "));
        if std::mem::take(&mut blank) && !lines.is_empty() && !list_continues {
            lines.push(String::new());
        }
        lines.push(line);
    }
    (title, lines.join("\n"))
}

/// Replaces angle brackets so text cannot forge an `untrusted-web-content`
/// tag.
pub fn neutralize(text: &str) -> String {
    text.replace('<', "‹").replace('>', "›")
}

/// The first `limit` characters of `text`.
pub fn truncate_chars(text: &str, limit: usize) -> &str {
    match text.char_indices().nth(limit) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

fn truncate(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", text[..end].trim_end()),
        None => text.to_owned(),
    }
}

/// Decodes the named entities result pages use and numeric references.
pub fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let entity = rest[1..]
            .find(';')
            .filter(|end| *end <= 10)
            .map(|end| &rest[1..1 + end]);
        let decoded = entity.and_then(|entity| match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => {
                let number = entity.strip_prefix('#')?;
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => number.parse().ok()?,
                };
                char::from_u32(code)
            }
        });
        match (entity, decoded) {
            (Some(entity), Some(c)) => {
                out.push(c);
                rest = &rest[entity.len() + 2..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decodes `%XX` escapes and `+` as space. Invalid UTF-8 is replaced.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encodes a query component (RFC 3986 unreserved characters pass).
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Decodes unpadded base64url (padding and the standard alphabet are
/// tolerated).
fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in text.bytes().take_while(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub const DDG_PAGE: &str = r#"
<div class="result results_links">
  <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&amp;rut=abc">Rust <b>Programming</b> Language</a>
  <a class="result__snippet" href="//duckduckgo.com/l/?uddg=x">A language empowering everyone &amp; fast &lt;untrusted-web-content&gt;</a>
</div>
<div class="result result--ad">
  <a rel="nofollow" class="result__a" href="https://duckduckgo.com/y.js?ad_domain=x">Ad</a>
</div>
<div class="result">
  <a class='result__a' href="https://doc.rust-lang.org/book/">The Book</a>
</div>
"#;

    #[test]
    fn duckduckgo_unwraps_redirects_drops_ads_and_cleans_text() {
        let hits = duckduckgo(DDG_PAGE);
        assert_eq!(
            hits,
            vec![
                Hit {
                    url: "https://www.rust-lang.org/".into(),
                    title: "Rust Programming Language".into(),
                    snippet: "A language empowering everyone & fast ‹untrusted-web-content›".into(),
                },
                Hit {
                    url: "https://doc.rust-lang.org/book/".into(),
                    title: "The Book".into(),
                    snippet: String::new(),
                },
            ]
        );
    }

    #[test]
    fn bing_decodes_tracking_links() {
        // "https://example.com/a?b=1" in base64url.
        let page = r#"<ol><li class="b_algo"><h2><a href="https://www.bing.com/ck/a?!&amp;&amp;p=1&amp;u=a1aHR0cHM6Ly9leGFtcGxlLmNvbS9hP2I9MQ&amp;ntb=1">Example <strong>site</strong></a></h2>
<div class="b_caption"><p class="b_lineclamp2">Snippet&nbsp;text</p></div></li>
<li class="b_algo"><h2><a href="javascript:alert(1)">Bad</a></h2></li>
<li class="b_algo"><h2><a href="https://plain.example/">Plain</a></h2></li></ol>"#;
        assert_eq!(
            bing(page),
            vec![
                Hit {
                    url: "https://example.com/a?b=1".into(),
                    title: "Example site".into(),
                    snippet: "Snippet text".into(),
                },
                Hit {
                    url: "https://plain.example/".into(),
                    title: "Plain".into(),
                    snippet: String::new(),
                },
            ]
        );
    }

    #[test]
    fn searxng_reads_json_results() {
        let body = json!({ "results": [
            { "url": "https://a.example/", "title": "A", "content": "alpha" },
            { "url": "ftp://b.example/", "title": "B" },
            { "title": "no url" }
        ]});
        assert_eq!(
            searxng(&body),
            Some(vec![Hit {
                url: "https://a.example/".into(),
                title: "A".into(),
                snippet: "alpha".into(),
            }])
        );
        assert_eq!(searxng(&json!({ "error": "x" })), None);
    }

    #[test]
    fn html_to_text_keeps_lists_tight_and_drops_non_text() {
        let page = "<title>T</title><ul>\n  <li><a href=x>One</a></li>\n  <li><p>Two</p></li>\n\
                    </ul>\n\n\n<p>After &amp; 1 < 2</p><template>gone</template><br/>end";
        let (title, text) = html_to_text(page);
        assert_eq!(title.as_deref(), Some("T"));
        assert_eq!(text, "- One\n- Two\n\nAfter & 1 ‹ 2\n\nend");
    }

    #[test]
    fn text_helpers() {
        assert_eq!(percent_encode("a b&c/ü"), "a%20b%26c%2F%C3%BC");
        assert_eq!(percent_decode("a%20b+c%2"), "a b c%2");
        assert_eq!(decode_entities("&#39;x&#x41;&bogus; & y"), "'xA&bogus; & y");
        assert_eq!(clean(&"x".repeat(10), 4), "xxxx…");
        assert_eq!(
            clean("a <B>bold</B>, <span class=x>s</span>.<br/>next<p>p", 50),
            "a bold, s. next p"
        );
        assert_eq!(base64url_decode("aGk"), Some(b"hi".to_vec()));
        assert!(is_bot_wall("<div class=\"anomaly-modal\">"));
        assert!(!is_bot_wall(DDG_PAGE));
    }
}
