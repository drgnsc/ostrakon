//! Markdown -> sanitized HTML. Everything the webview receives passes through ammonia here;
//! the CSP in tauri.conf.json is the second, independent layer.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;

/// Local images are served by the `art` protocol handler, never by path.
pub const ART_ORIGIN: &str = "https://art.localhost/";

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub id: String,
}

#[derive(Debug, Default)]
pub struct Rendered {
    pub html: String,
    pub toc: Vec<TocEntry>,
    pub meta: Vec<(String, String)>,
    pub words: usize,
}

pub fn render(source: &str) -> Rendered {
    let (meta, body) = split_front_matter(source);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;

    let mut events: Vec<Event> = Vec::new();
    let mut toc = Vec::new();
    let mut slugs = Slugger::default();
    let mut words = 0usize;
    let mut heading: Option<(usize, String)> = None;

    for event in Parser::new_ext(body, options) {
        match &event {
            Event::Text(t) | Event::Code(t) => {
                words += t.split_whitespace().count();
                if let Some((_, text)) = heading.as_mut() {
                    text.push_str(t);
                }
            }
            Event::Start(Tag::Heading { .. }) => heading = Some((events.len(), String::new())),
            Event::End(TagEnd::Heading(level)) => {
                if let Some((start, text)) = heading.take() {
                    let id = slugs.next(&text);
                    if let Event::Start(Tag::Heading { id: slot, .. }) = &mut events[start] {
                        *slot = Some(CowStr::from(id.clone()));
                    }
                    toc.push(TocEntry { level: level_number(*level), text, id });
                }
            }
            _ => {}
        }
        events.push(match event {
            Event::Start(Tag::CodeBlock(kind)) => Event::Html(CowStr::from(code_open(&kind))),
            Event::End(TagEnd::CodeBlock) => Event::Html(CowStr::from("</code></pre>\n")),
            Event::TaskListMarker(done) => Event::InlineHtml(CowStr::from(if done {
                r#"<span class="task is-done"></span>"#
            } else {
                r#"<span class="task"></span>"#
            })),
            other => other,
        });
    }

    let mut raw = String::with_capacity(body.len() * 3 / 2);
    html::push_html(&mut raw, events.into_iter());

    Rendered { html: sanitize(&raw), toc, meta, words }
}

fn code_open(kind: &CodeBlockKind) -> String {
    let lang = match kind {
        CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or(""),
        CodeBlockKind::Indented => "",
    };
    let clean: String = lang
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '+' | '#' | '.'))
        .take(24)
        .collect();
    if clean.is_empty() {
        "<pre><code>".to_string()
    } else {
        format!(r#"<pre data-lang="{clean}"><code>"#)
    }
}

fn sanitize(raw: &str) -> String {
    // style="" attributes are blocked by the CSP, so table alignment travels as a class.
    let raw = raw
        .replace(r#"style="text-align: left""#, r#"class="ta-l""#)
        .replace(r#"style="text-align: center""#, r#"class="ta-c""#)
        .replace(r#"style="text-align: right""#, r#"class="ta-r""#);

    let mut builder = ammonia::Builder::default();
    builder
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .url_relative(ammonia::UrlRelative::PassThrough)
        .link_rel(Some("noopener noreferrer"))
        .strip_comments(true)
        .add_tag_attributes("pre", ["data-lang"])
        .add_tag_attributes("a", ["title"])
        .add_tag_attributes("img", ["title"])
        .add_generic_attributes(["id"])
        .attribute_filter(|element, attribute, value| match (element, attribute) {
            ("img", "src") => image_source(value).map(Cow::Owned),
            _ => Some(Cow::Borrowed(value)),
        });
    builder.allowed_classes(allowed_classes());
    builder.clean(&raw).to_string()
}

/// Remote images are dropped (no tracking pixels, no network). Relative ones go through `art`.
fn image_source(value: &str) -> Option<String> {
    let lower = value.trim().to_ascii_lowercase();
    let absolute = lower.starts_with("//")
        || lower.starts_with('\\')
        || lower.starts_with('/')
        || lower.split_once(':').is_some_and(|(scheme, _)| !scheme.contains('/'));
    if absolute {
        return None;
    }
    Some(format!(
        "{ART_ORIGIN}?p={}",
        utf8_percent_encode(value.trim(), NON_ALPHANUMERIC)
    ))
}

fn allowed_classes() -> HashMap<&'static str, HashSet<&'static str>> {
    let align: HashSet<&str> = ["ta-l", "ta-c", "ta-r"].into_iter().collect();
    let alerts: HashSet<&str> = [
        "markdown-alert-note",
        "markdown-alert-tip",
        "markdown-alert-important",
        "markdown-alert-warning",
        "markdown-alert-caution",
    ]
    .into_iter()
    .collect();
    HashMap::from([
        ("span", ["task", "is-done"].into_iter().collect()),
        ("th", align.clone()),
        ("td", align),
        ("blockquote", alerts),
        ("sup", ["footnote-reference", "footnote-definition-label"].into_iter().collect()),
        ("div", ["footnote-definition"].into_iter().collect()),
    ])
}

fn level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// GitHub-style anchors, so links written for GitHub keep working.
#[derive(Default)]
struct Slugger {
    seen: HashMap<String, usize>,
}

impl Slugger {
    fn next(&mut self, text: &str) -> String {
        let base: String = text
            .trim()
            .to_lowercase()
            .chars()
            .filter_map(|c| match c {
                ' ' | '-' => Some('-'),
                '_' => Some('_'),
                c if c.is_alphanumeric() => Some(c),
                _ => None,
            })
            .collect();
        let count = self.seen.entry(base.clone()).or_insert(0);
        let slug = if *count == 0 { base.clone() } else { format!("{base}-{count}") };
        *count += 1;
        slug
    }
}

/// Splits a leading `---` YAML block off. Only flat display pairs are needed, so no YAML parser.
fn split_front_matter(source: &str) -> (Vec<(String, String)>, &str) {
    let Some(rest) = source.strip_prefix("---\n").or_else(|| source.strip_prefix("---\r\n")) else {
        return (Vec::new(), source);
    };
    let Some((block, body)) = find_closing_fence(rest) else {
        return (Vec::new(), source);
    };

    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let top_level = !line.starts_with(' ') && !line.starts_with('\t');
        match trimmed.split_once(':') {
            Some((key, value)) if top_level && !trimmed.starts_with('-') => {
                pairs.push((key.trim().to_string(), unquote(value.trim()).to_string()));
            }
            _ => {
                if let Some((_, value)) = pairs.last_mut() {
                    if !value.is_empty() {
                        value.push_str(", ");
                    }
                    value.push_str(unquote(trimmed.trim_start_matches("- ")));
                }
            }
        }
    }
    (pairs, body)
}

fn find_closing_fence(text: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&text[..offset], &text[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn unquote(value: &str) -> &str {
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        return &value[1..value.len() - 1];
    }
    // An inline YAML comment only starts after whitespace, so `a#b` stays intact.
    value.split_once(" #").map_or(value, |(before, _)| before.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(source: &str) -> String {
        render(source).html
    }

    #[test]
    fn strips_scripts_and_event_handlers() {
        let out = html("<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n<div onclick=\"x()\">a</div>");
        assert!(!out.contains("<script"));
        assert!(!out.contains("onerror"));
        assert!(!out.contains("onclick"));
    }

    #[test]
    fn strips_dangerous_link_schemes() {
        for scheme in ["javascript:alert(1)", "data:text/html,x", "file:///C:/x", "vbscript:x"] {
            let out = html(&format!("[a]({scheme})"));
            assert!(!out.contains(scheme), "{scheme} survived: {out}");
        }
    }

    #[test]
    fn strips_embedding_elements() {
        let out = html("<iframe src=\"https://x\"></iframe><object data=x></object><style>*{}</style><base href=x><form action=x><input></form>");
        for tag in ["<iframe", "<object", "<style", "<base", "<form", "<input"] {
            assert!(!out.contains(tag), "{tag} survived: {out}");
        }
    }

    #[test]
    fn drops_remote_and_absolute_images() {
        for src in [
            "https://evil.example/p.png",
            "http://evil.example/p.png",
            "//evil.example/p.png",
            "file:///C:/secret.png",
            "C:/secret.png",
            "data:image/png;base64,AAAA",
        ] {
            let out = html(&format!("![a]({src})"));
            assert!(!out.contains("src="), "{src} survived: {out}");
        }
        assert!(!html(r"<img src='\\evil\share\p.png'>").contains("src="));
    }

    #[test]
    fn rewrites_relative_images_to_the_art_origin() {
        let out = html("![a](../img/my%20shot.png)");
        assert!(out.contains("src=\"https://art.localhost/?p=%2E%2E%2Fimg%2Fmy%2520shot%2Epng\""), "{out}");
    }

    #[test]
    fn keeps_table_alignment_as_class_not_style() {
        let out = html("| a | b |\n|:-:|--:|\n| 1 | 2 |\n");
        assert!(out.contains("class=\"ta-c\""), "{out}");
        assert!(out.contains("class=\"ta-r\""), "{out}");
        assert!(!out.contains("style="), "{out}");
    }

    #[test]
    fn renders_task_lists_without_form_controls() {
        let out = html("- [x] done\n- [ ] open\n");
        assert!(out.contains("class=\"task is-done\""), "{out}");
        assert!(out.contains("class=\"task\""), "{out}");
        assert!(!out.contains("<input"));
    }

    #[test]
    fn code_blocks_keep_language_and_escape_content() {
        let out = html("```rust\n<script>x</script>\n```\n");
        assert!(out.contains("data-lang=\"rust\""), "{out}");
        assert!(out.contains("&lt;script&gt;"), "{out}");
    }

    #[test]
    fn language_tag_cannot_break_out_of_attribute() {
        let out = html("```\"><script>x</script>\n```\n");
        assert!(!out.contains("<script"), "{out}");
    }

    #[test]
    fn builds_toc_with_github_anchors() {
        let doc = render("# Cel główny\n\n## Zakres\n\n## Zakres\n\n### `kod` i tekst!\n");
        let ids: Vec<&str> = doc.toc.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["cel-główny", "zakres", "zakres-1", "kod-i-tekst"]);
        assert_eq!(doc.toc[0].level, 1);
        assert!(doc.html.contains("id=\"zakres-1\""));
    }

    #[test]
    fn parses_front_matter_into_pairs() {
        let doc = render("---\nproject: Ostrakon\nstatus: \"draft\"\ntags:\n  - a\n  - b\n# note\n---\n\n# T\n");
        assert_eq!(doc.meta[0], ("project".into(), "Ostrakon".into()));
        assert_eq!(doc.meta[1], ("status".into(), "draft".into()));
        assert_eq!(doc.meta[2], ("tags".into(), "a, b".into()));
        assert_eq!(doc.toc[0].text, "T");
        assert!(!doc.html.contains("project"));
    }

    #[test]
    fn strips_inline_yaml_comments_from_unquoted_values() {
        let doc = render("---\na: other  # note\nb: \"keep # this\"\nc: x#y\n---\n");
        assert_eq!(doc.meta[0].1, "other");
        assert_eq!(doc.meta[1].1, "keep # this");
        assert_eq!(doc.meta[2].1, "x#y");
    }

    #[test]
    fn front_matter_values_are_not_html() {
        let doc = render("---\nx: <script>a</script>\n---\nb\n");
        // Sent as plain data and written with textContent in the frontend.
        assert_eq!(doc.meta[0].1, "<script>a</script>");
        assert!(!doc.html.contains("<script"));
    }

    #[test]
    fn unterminated_front_matter_stays_in_body() {
        let doc = render("---\nnot closed\n");
        assert!(doc.meta.is_empty());
        assert!(doc.html.contains("not closed"));
    }

    #[test]
    fn handles_crlf_front_matter() {
        let doc = render("---\r\na: 1\r\n---\r\n\r\n# T\r\n");
        assert_eq!(doc.meta[0], ("a".into(), "1".into()));
        assert_eq!(doc.toc[0].text, "T");
    }

    #[test]
    fn keeps_common_raw_html_and_footnotes() {
        let out = html("<details><summary>s</summary>t</details>\n\nx[^1]\n\n[^1]: note\n");
        assert!(out.contains("<details>"), "{out}");
        assert!(out.contains("class=\"footnote-definition\""), "{out}");
    }

    #[test]
    fn gfm_alerts_keep_their_class() {
        let out = html("> [!WARNING]\n> uwaga\n");
        assert!(out.contains("markdown-alert-warning"), "{out}");
    }

    #[test]
    fn counts_words() {
        assert_eq!(render("# Dwa słowa\n\nI jeszcze trzy\n").words, 5);
    }
}
