//! Semantic landing-page container parsing and fallback rendering.
//!
//! This module is deliberately independent of files, templates, and theme
//! loading. Themes style the stable HTML classes emitted by `render`.

use std::collections::BTreeMap;

use crate::header::{Header, TemplateId};
use crate::model::DiagnosticLevel;

const COMPONENTS: &[&str] = &["hero", "section", "features", "card", "feature", "callout"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandingDiagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Markdown(String),
    Component(Component),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Component {
    name: String,
    attrs: BTreeMap<String, Option<String>>,
    children: Vec<Node>,
    line: usize,
}

/// An explicit layout always wins. `landingpage.html` retains its historic
/// template meaning and supplies the landing default only when layout is absent.
pub fn uses_landing_layout(header: &Header) -> bool {
    match header.layout.as_deref().map(str::trim) {
        Some(value) => value.eq_ignore_ascii_case("landing"),
        None => header.template == Some(TemplateId::Landing),
    }
}

pub fn validate(markdown: &str, enabled: bool) -> Vec<LandingDiagnostic> {
    if !enabled {
        return Vec::new();
    }
    let nodes = match parse(markdown) {
        Ok(nodes) => nodes,
        Err(message) => {
            return vec![LandingDiagnostic {
                level: DiagnosticLevel::Error,
                message,
            }];
        }
    };
    let mut diagnostics = Vec::new();
    validate_nodes(&nodes, false, &mut diagnostics);
    diagnostics
}

pub fn render(markdown: &str, rel: &str, render_markdown: impl Fn(&str) -> String) -> String {
    match parse(markdown) {
        Ok(nodes) => render_nodes(&nodes, rel, &render_markdown),
        // Assembly validation prevents this in normal builds. Keep content usable
        // for direct library callers as well.
        Err(_) => render_markdown(&strip_containers(markdown)),
    }
}

/// Graceful degradation for non-landing pages: only the recognized fences are
/// removed; all contained Markdown remains ordinary Markdown.
pub fn strip_containers(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut code_fence = None;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(marker) = code_fence {
            out.push(line);
            if is_fence_closer(trimmed, marker) {
                code_fence = None;
            }
            continue;
        }
        if let Some(marker) = code_fence_marker(trimmed) {
            out.push(line);
            code_fence = Some(marker);
            continue;
        }
        if !(trimmed == ":::"
            || matches!(opening_name(trimmed), Some(name) if COMPONENTS.contains(&name)))
        {
            out.push(line);
        }
    }
    out.join("\n")
}

fn validate_nodes(nodes: &[Node], inside_features: bool, diagnostics: &mut Vec<LandingDiagnostic>) {
    for node in nodes {
        let Node::Component(component) = node else {
            continue;
        };
        if component.name == "card" && !inside_features {
            diagnostics.push(LandingDiagnostic {
                level: DiagnosticLevel::Warning,
                message: format!(
                    "landing page line {}: card should be inside features",
                    component.line
                ),
            });
        }
        validate_attributes(component, diagnostics);
        validate_nodes(
            &component.children,
            inside_features || component.name == "features",
            diagnostics,
        );
    }
}

fn validate_attributes(component: &Component, diagnostics: &mut Vec<LandingDiagnostic>) {
    for (name, value) in &component.attrs {
        let supported = match component.name.as_str() {
            "hero" => name == "align" && matches!(value.as_deref(), Some("left" | "center")),
            "section" => {
                name == "tone"
                    && matches!(
                        value.as_deref(),
                        Some("default" | "subtle" | "accent" | "contrast")
                    )
            }
            "card" => name == "icon" && value.is_some(),
            "feature" => {
                (name == "image" && value.is_some()) || (name == "reverse" && value.is_none())
            }
            _ => false,
        };
        if !supported {
            diagnostics.push(LandingDiagnostic {
                level: DiagnosticLevel::Warning,
                message: format!(
                    "landing page line {}: unsupported attribute '{}' on {}",
                    component.line, name, component.name
                ),
            });
        }
    }
}

fn parse(markdown: &str) -> Result<Vec<Node>, String> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let (nodes, next) = parse_nodes(&lines, 0, None)?;
    debug_assert_eq!(next, lines.len());
    Ok(nodes)
}

fn parse_nodes(
    lines: &[&str],
    mut index: usize,
    closing: Option<(&str, usize)>,
) -> Result<(Vec<Node>, usize), String> {
    let mut nodes = Vec::new();
    let mut markdown = String::new();
    let mut code_fence: Option<char> = None;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        if let Some(marker) = code_fence {
            markdown.push_str(line);
            markdown.push('\n');
            if is_fence_closer(trimmed, marker) {
                code_fence = None;
            }
            index += 1;
            continue;
        }
        if let Some(marker) = code_fence_marker(trimmed) {
            markdown.push_str(line);
            markdown.push('\n');
            code_fence = Some(marker);
            index += 1;
            continue;
        }
        if trimmed == ":::" {
            let Some((_expected, _start_line)) = closing else {
                return Err(format!(
                    "landing page line {}: unexpected closing container",
                    index + 1
                ));
            };
            push_markdown(&mut nodes, &mut markdown);
            return Ok((nodes, index + 1));
        }
        if trimmed.starts_with(":::") {
            let (name, attrs) = parse_opening(trimmed, index + 1)?;
            push_markdown(&mut nodes, &mut markdown);
            let (children, next) = parse_nodes(lines, index + 1, Some((&name, index + 1)))?;
            nodes.push(Node::Component(Component {
                name,
                attrs,
                children,
                line: index + 1,
            }));
            index = next;
            continue;
        }
        markdown.push_str(line);
        markdown.push('\n');
        index += 1;
    }
    if let Some((name, line)) = closing {
        return Err(format!(
            "landing page line {}: unclosed {} container",
            line, name
        ));
    }
    push_markdown(&mut nodes, &mut markdown);
    Ok((nodes, index))
}

fn push_markdown(nodes: &mut Vec<Node>, markdown: &mut String) {
    if !markdown.is_empty() {
        nodes.push(Node::Markdown(std::mem::take(markdown)));
    }
}

fn parse_opening(
    line: &str,
    line_no: usize,
) -> Result<(String, BTreeMap<String, Option<String>>), String> {
    let rest = line[3..].trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("");
    if !COMPONENTS.contains(&name) {
        return Err(format!(
            "landing page line {}: unknown component '{}'",
            line_no, name
        ));
    }
    let attrs = parse_attributes(parts.next().unwrap_or(""), line_no)?;
    Ok((name.to_string(), attrs))
}

fn parse_attributes(input: &str, line: usize) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut attrs = BTreeMap::new();
    let mut rest = input.trim();
    while !rest.is_empty() {
        let name_end = rest
            .find(|ch: char| ch.is_whitespace() || ch == '=')
            .unwrap_or(rest.len());
        let name = &rest[..name_end];
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(format!(
                "landing page line {}: invalid attribute syntax",
                line
            ));
        }
        rest = rest[name_end..].trim_start();
        let value = if let Some(after_equals) = rest.strip_prefix('=') {
            rest = after_equals.trim_start();
            if let Some(after_quote) = rest.strip_prefix('"') {
                let Some(end) = after_quote.find('"') else {
                    return Err(format!(
                        "landing page line {}: unterminated quoted attribute",
                        line
                    ));
                };
                let value = after_quote[..end].to_string();
                rest = after_quote[end + 1..].trim_start();
                Some(value)
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                if end == 0 {
                    return Err(format!(
                        "landing page line {}: missing attribute value",
                        line
                    ));
                }
                let value = rest[..end].to_string();
                rest = rest[end..].trim_start();
                Some(value)
            }
        } else {
            None
        };
        if attrs.insert(name.to_string(), value).is_some() {
            return Err(format!(
                "landing page line {}: duplicate attribute '{}'",
                line, name
            ));
        }
    }
    Ok(attrs)
}

fn opening_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(":::")?.trim();
    rest.split_whitespace().next()
}

fn render_nodes(nodes: &[Node], rel: &str, render_markdown: &impl Fn(&str) -> String) -> String {
    nodes
        .iter()
        .map(|node| match node {
            Node::Markdown(markdown) => render_markdown(markdown),
            Node::Component(component) => render_component(component, rel, render_markdown),
        })
        .collect()
}

fn render_component(
    component: &Component,
    rel: &str,
    render_markdown: &impl Fn(&str) -> String,
) -> String {
    let content = render_nodes(&component.children, rel, render_markdown);
    match component.name.as_str() {
        "hero" => format!(
            "<section class=\"landing-hero landing-hero--{}\">{content}</section>",
            allowed_attr(component, "align", &["left", "center"]).unwrap_or("left")
        ),
        "section" => format!(
            "<section class=\"landing-section landing-section--{}\">{content}</section>",
            allowed_attr(
                component,
                "tone",
                &["default", "subtle", "accent", "contrast"]
            )
            .unwrap_or("default")
        ),
        "features" => render_features(component, rel, render_markdown),
        "card" => render_card(component, rel, &content),
        "feature" => {
            let reverse = matches!(component.attrs.get("reverse"), Some(None));
            let image = attr(component, "image")
                .map(|path| {
                    format!(
                        "<img class=\"landing-feature__image\" src=\"{}\" alt=\"\">",
                        escape_attr(&resolve_asset(path, rel))
                    )
                })
                .unwrap_or_default();
            format!(
                "<section class=\"landing-feature{}\">{image}<div class=\"landing-feature__content\">{content}</div></section>",
                if reverse {
                    " landing-feature--reverse"
                } else {
                    ""
                }
            )
        }
        "callout" => format!("<aside class=\"landing-callout\">{content}</aside>"),
        _ => content,
    }
}

fn render_features(
    component: &Component,
    rel: &str,
    render_markdown: &impl Fn(&str) -> String,
) -> String {
    let mut out = String::from("<section class=\"landing-features\">");
    let mut cards = String::new();
    let flush_cards = |out: &mut String, cards: &mut String| {
        if !cards.is_empty() {
            out.push_str("<div class=\"landing-features__cards\">");
            out.push_str(cards);
            out.push_str("</div>");
            cards.clear();
        }
    };
    for child in &component.children {
        if matches!(child, Node::Markdown(markdown) if markdown.trim().is_empty()) {
            continue;
        }
        if let Node::Component(card) = child
            && card.name == "card"
        {
            cards.push_str(&render_component(card, rel, render_markdown));
        } else {
            flush_cards(&mut out, &mut cards);
            out.push_str(&render_nodes(
                std::slice::from_ref(child),
                rel,
                render_markdown,
            ));
        }
    }
    flush_cards(&mut out, &mut cards);
    out.push_str("</section>");
    out
}

fn render_card(component: &Component, rel: &str, content: &str) -> String {
    let icon = attr(component, "icon")
        .map(|path| {
            format!(
                "<img class=\"landing-card__icon\" src=\"{}\" alt=\"\">",
                escape_attr(&resolve_asset(path, rel))
            )
        })
        .unwrap_or_default();
    format!(
        "<article class=\"landing-card\">{icon}<div class=\"landing-card__content\">{content}</div></article>"
    )
}

fn attr<'a>(component: &'a Component, name: &str) -> Option<&'a str> {
    component.attrs.get(name).and_then(|value| value.as_deref())
}

fn allowed_attr<'a>(component: &'a Component, name: &str, allowed: &[&str]) -> Option<&'a str> {
    attr(component, name).filter(|value| allowed.contains(value))
}

fn code_fence_marker(line: &str) -> Option<char> {
    let marker = line.chars().next()?;
    (matches!(marker, '`' | '~') && line.starts_with(&marker.to_string().repeat(3)))
        .then_some(marker)
}

fn is_fence_closer(line: &str, marker: char) -> bool {
    line.starts_with(&marker.to_string().repeat(3))
}

fn resolve_asset(path: &str, rel: &str) -> String {
    if path.contains("://") || path.starts_with('/') || path.starts_with('#') {
        path.to_string()
    } else {
        format!("{rel}{path}")
    }
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Header;

    #[test]
    fn layout_detection_honors_explicit_layout() {
        let mut header = Header {
            template: Some(TemplateId::Landing),
            ..Header::default()
        };
        assert!(uses_landing_layout(&header));
        header.layout = Some("standard".to_string());
        assert!(!uses_landing_layout(&header));
        header.layout = Some("landing".to_string());
        assert!(uses_landing_layout(&header));
        let explicit_landing = Header {
            layout: Some("landing".to_string()),
            ..Header::default()
        };
        assert!(uses_landing_layout(&explicit_landing));
        assert!(!uses_landing_layout(&Header::default()));
    }

    #[test]
    fn renders_components_and_quoted_attributes() {
        let html = render(
            "::: hero align=center\nHero\n:::\n\n::: section tone=subtle\nSection\n:::\n\n::: features\nIntro\n::: card icon=\"images/a b.svg\"\n# Card\n:::\n\n::: card\n# Other card\n:::\n:::\n\n::: feature image=images/x.webp reverse\nText\n:::\n\n::: callout\nCallout\n:::\n",
            "../",
            |value| format!("<md>{}</md>", value.trim()),
        );
        assert!(html.contains("landing-hero--center"));
        assert!(html.contains("landing-section--subtle"));
        assert!(html.contains("landing-features__cards"));
        assert_eq!(html.matches("landing-features__cards").count(), 1);
        assert!(html.contains("../images/a b.svg"));
        assert!(html.contains("landing-feature--reverse"));
        assert!(html.contains("landing-callout"));
    }

    #[test]
    fn validation_handles_cards_and_bad_nesting() {
        let warnings = validate("::: card\nText\n:::\n", true);
        assert!(
            warnings
                .iter()
                .any(|item| item.message.contains("inside features"))
        );
        let errors = validate("::: hero\nText", true);
        assert_eq!(errors[0].level, DiagnosticLevel::Error);
    }

    #[test]
    fn non_landing_degradation_keeps_markdown() {
        assert_eq!(strip_containers("::: hero\n# Hello\n:::\n"), "# Hello");
    }

    #[test]
    fn fences_inside_code_blocks_are_not_components() {
        let html = render("```text\n::: hero\n::: \n```\n", "", |value| {
            value.to_string()
        });
        assert!(html.contains("::: hero"));
    }
}
