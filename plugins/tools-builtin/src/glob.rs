//! A small glob matcher for `/`-separated relative paths.
//!
//! Supported syntax: `*` (any characters within one path segment), `?` (one
//! character within a segment), `**` as a whole segment (zero or more
//! segments) and `{a,b}` alternatives (not nested). A pattern without `/`
//! matches the file name at any depth, so `*.rs` is `**/*.rs`.

/// Upper bound on the alternatives that `{a,b}` groups expand to.
const MAX_ALTERNATIVES: usize = 64;

#[derive(Debug)]
pub(crate) struct Glob {
    alternatives: Vec<Vec<String>>,
}

impl Glob {
    pub(crate) fn new(pattern: &str) -> Result<Self, String> {
        let pattern = pattern.trim().replace('\\', "/");
        let pattern = pattern.trim_start_matches("./");
        if pattern.is_empty() {
            return Err("the glob pattern is empty".into());
        }
        if pattern.starts_with('/') || pattern.split('/').any(|segment| segment == "..") {
            return Err("the glob pattern must be relative and must not contain `..`".into());
        }
        let alternatives = expand_braces(pattern)?
            .into_iter()
            .map(|alternative| {
                let alternative = if alternative.contains('/') {
                    alternative
                } else {
                    format!("**/{alternative}")
                };
                alternative
                    .split('/')
                    .filter(|segment| !segment.is_empty() && *segment != ".")
                    .map(str::to_owned)
                    .collect()
            })
            .collect();
        Ok(Self { alternatives })
    }

    /// Whether `path` (relative, `/`-separated) matches.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        self.alternatives
            .iter()
            .any(|pattern| match_segments(pattern, &segments))
    }
}

fn expand_braces(pattern: &str) -> Result<Vec<String>, String> {
    let Some(open) = pattern.find('{') else {
        if pattern.contains('}') {
            return Err("unbalanced `}` in the glob pattern".into());
        }
        return Ok(vec![pattern.to_owned()]);
    };
    let close = pattern[open..]
        .find('}')
        .map(|offset| open + offset)
        .ok_or("unbalanced `{` in the glob pattern")?;
    let inner = &pattern[open + 1..close];
    if inner.contains('{') {
        return Err("nested `{` groups are not supported".into());
    }
    let (head, tail) = (&pattern[..open], &pattern[close + 1..]);
    let mut expanded = Vec::new();
    for rest in expand_braces(tail)? {
        for choice in inner.split(',') {
            expanded.push(format!("{head}{choice}{rest}"));
            if expanded.len() > MAX_ALTERNATIVES {
                return Err(format!(
                    "the glob pattern expands to more than {MAX_ALTERNATIVES} alternatives"
                ));
            }
        }
    }
    Ok(expanded)
}

fn match_segments(pattern: &[String], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((first, rest)) if first == "**" => {
            (0..=path.len()).any(|skip| match_segments(rest, &path[skip..]))
        }
        Some((first, rest)) => match path.split_first() {
            Some((segment, path)) => match_segment(first, segment) && match_segments(rest, path),
            None => false,
        },
    }
}

/// Wildcard match of one segment with `*` and `?`.
fn match_segment(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                backtrack = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match backtrack {
                Some((star, matched)) => {
                    p = star + 1;
                    t = matched + 1;
                    backtrack = Some((star, matched + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, path: &str) -> bool {
        Glob::new(pattern).unwrap().matches(path)
    }

    #[test]
    fn segment_wildcards_stay_within_a_segment() {
        assert!(matches("src/*.rs", "src/lib.rs"));
        assert!(!matches("src/*.rs", "src/a/lib.rs"));
        assert!(matches("src/?ib.rs", "src/lib.rs"));
        assert!(!matches("src/?.rs", "src/lib.rs"));
        assert!(matches("x/a*b*c", "x/axxbyyc"));
        assert!(!matches("x/a*b*c", "x/axxbyy"));
    }

    #[test]
    fn double_star_spans_zero_or_more_segments() {
        assert!(matches("src/**/*.rs", "src/lib.rs"));
        assert!(matches("src/**/*.rs", "src/a/b/lib.rs"));
        assert!(!matches("src/**/*.rs", "tests/lib.rs"));
        assert!(matches("**", "a/b/c"));
    }

    #[test]
    fn a_pattern_without_slash_matches_the_name_at_any_depth() {
        assert!(matches("*.toml", "Cargo.toml"));
        assert!(matches("*.toml", "plugins/x/Cargo.toml"));
        assert!(!matches("*.toml", "Cargo.lock"));
    }

    #[test]
    fn braces_expand_to_alternatives() {
        assert!(matches("*.{rs,toml}", "a/lib.rs"));
        assert!(matches("*.{rs,toml}", "Cargo.toml"));
        assert!(!matches("*.{rs,toml}", "README.md"));
        assert!(matches("{src,tests}/*.rs", "tests/a.rs"));
    }

    #[test]
    fn invalid_patterns_are_rejected() {
        for pattern in ["", "/abs/*", "../x", "a/../b", "{a,b", "a}", "{a,{b}}"] {
            assert!(Glob::new(pattern).is_err(), "{pattern}");
        }
        let many = "{a,b,c,d}{a,b,c,d}{a,b,c,d}{a,b}";
        assert!(Glob::new(many).is_err());
    }
}
