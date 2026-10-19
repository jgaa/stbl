//! Semantic landing-page container parsing and fallback rendering.
//!
//! This module is deliberately independent of files, templates, and theme
//! loading. Themes style the stable HTML classes emitted by `render`.

use std::collections::BTreeMap;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

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

/// Images in component metadata participate in the same plan as Markdown images.
pub fn collect_media_refs(markdown: &str) -> Vec<crate::media::MediaRef> {
    fn collect(nodes: &[Node], refs: &mut Vec<crate::media::MediaRef>) {
        for node in nodes {
            let Node::Component(component) = node else {
                continue;
            };
            let key = match component.name.as_str() {
                "card" | "section" => "icon",
                "feature" => "image",
                _ => "",
            };
            if let Some(path) = attr(component, key)
                && let Some(crate::media::MediaRef::Image(image)) =
                    crate::media::parse_media_destination(path, "")
            {
                refs.push(crate::media::MediaRef::Image(image));
            }
            collect(&component.children, refs);
        }
    }
    let mut refs = Vec::new();
    if let Ok(nodes) = parse(markdown) {
        collect(&nodes, &mut refs);
    }
    refs
}

pub fn render(markdown: &str, rel: &str, render_markdown: impl Fn(&str) -> String) -> String {
    render_with_context(markdown, rel, |fragment, _| render_markdown(fragment))
}

/// The callback receives the image column percentage for a split hero.
pub fn render_with_context(
    markdown: &str,
    rel: &str,
    render_markdown: impl Fn(&str, Option<f32>) -> String,
) -> String {
    render_with_media(markdown, rel, render_markdown, |path, _| {
        format!(
            "<img src=\"{}\" alt=\"\">",
            escape_attr(&resolve_asset(path, rel))
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentImage {
    CardIcon,
    CardImage,
    SectionIcon,
    SectionImage,
    Feature,
}

/// Themes render component images using the same planned variants as Markdown.
pub fn render_with_media(
    markdown: &str,
    rel: &str,
    render_markdown: impl Fn(&str, Option<f32>) -> String,
    render_image: impl Fn(&str, ComponentImage) -> String,
) -> String {
    match parse(markdown) {
        Ok(nodes) => render_nodes(&nodes, rel, &render_markdown, &render_image),
        // Assembly validation prevents this in normal builds. Keep content usable
        // for direct library callers as well.
        Err(_) => render_markdown(&strip_containers(markdown), None),
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
        let folding = matches!(component.name.as_str(), "card" | "features" | "section")
            && name == "folded"
            && matches!(value.as_deref(), Some("yes" | "no" | "auto" | "off"));
        let supported = folding
            || match component.name.as_str() {
                "hero" => {
                    (name == "align" && matches!(value.as_deref(), Some("left" | "center")))
                        || (name == "reverse" && value.is_none())
                        || (name == "image-align"
                            && matches!(value.as_deref(), Some("left" | "right")))
                }
                "section" => {
                    (name == "icon" && value.is_some())
                        || (name == "tone"
                            && matches!(
                                value.as_deref(),
                                Some("default" | "subtle" | "accent" | "contrast")
                            ))
                        || (name == "image-align"
                            && matches!(value.as_deref(), Some("left" | "right" | "center")))
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

fn render_nodes(
    nodes: &[Node],
    rel: &str,
    render_markdown: &impl Fn(&str, Option<f32>) -> String,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    nodes
        .iter()
        .map(|node| match node {
            Node::Markdown(markdown) => render_markdown(markdown, None),
            Node::Component(component) => {
                render_component(component, rel, render_markdown, render_image)
            }
        })
        .collect()
}

fn render_component(
    component: &Component,
    rel: &str,
    render_markdown: &impl Fn(&str, Option<f32>) -> String,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    if component.name == "hero" {
        return render_hero(component, rel, render_markdown, render_image);
    }
    if component.name == "features" {
        return render_features(component, rel, render_markdown, render_image);
    }
    if component.name == "section" {
        return render_section(component, rel, render_markdown, render_image);
    }
    let content = render_nodes(&component.children, rel, render_markdown, render_image);
    match component.name.as_str() {
        "card" => render_card(component, &content, render_image),
        "feature" => {
            let reverse = matches!(component.attrs.get("reverse"), Some(None));
            let image = attr(component, "image")
                .map(|path| {
                    format!(
                        "<div class=\"landing-feature__image\">{}</div>",
                        render_image(path, ComponentImage::Feature)
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

fn render_section(
    component: &Component,
    rel: &str,
    render_markdown: &impl Fn(&str, Option<f32>) -> String,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    let side = allowed_attr(component, "image-align", &["left", "right"]);
    let mut children = component.children.as_slice();
    let intro = if let Some(side) = side
        && let Some(Node::Markdown(markdown)) = children.first()
        && let Some((text, media, percent)) = split_hero_image(markdown)
    {
        children = &children[1..];
        format!(
            "<div class=\"landing-section__intro landing-hero--split{}\" style=\"--hero-image-weight: {percent}fr; --hero-text-weight: {}fr\"><div class=\"landing-hero__content\">{}</div><div class=\"landing-hero__image\">{}</div></div>",
            if side == "left" {
                " landing-hero--reverse"
            } else {
                ""
            },
            100.0 - percent,
            render_markdown(&text, None),
            render_markdown(&media, Some(percent)),
        )
    } else {
        String::new()
    };
    let content = render_nodes(children, rel, render_markdown, render_image);
    let tone = allowed_attr(
        component,
        "tone",
        &["default", "subtle", "accent", "contrast"],
    )
    .unwrap_or("default");
    let centered = if attr(component, "image-align") == Some("center") {
        " landing-section--image-center"
    } else {
        ""
    };
    let icon = render_icon(component, render_image);
    let folded = folding_attribute(component);
    format!(
        "<section class=\"landing-section landing-section--{tone}{centered}\"{folded}>{icon}{intro}{content}</section>"
    )
}

fn render_hero(
    component: &Component,
    rel: &str,
    render_markdown: &impl Fn(&str, Option<f32>) -> String,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    let align = allowed_attr(component, "align", &["left", "center"]).unwrap_or("left");
    let reverse = match allowed_attr(component, "image-align", &["left", "right"]) {
        Some(side) => side == "left",
        None => matches!(component.attrs.get("reverse"), Some(None)),
    };
    let mut content = String::new();
    let mut image = None;
    let mut percent = 50.0;
    for child in &component.children {
        let split = match child {
            Node::Markdown(markdown) if align != "center" && image.is_none() => {
                split_hero_image(markdown)
            }
            _ => None,
        };
        if let Some((text, media, width)) = split {
            percent = width;
            content.push_str(&render_markdown(&text, None));
            image = Some(render_markdown(&media, Some(percent)));
        } else {
            content.push_str(&render_nodes(
                std::slice::from_ref(child),
                rel,
                render_markdown,
                render_image,
            ));
        }
    }
    let body = if let Some(image) = image {
        format!(
            "<div class=\"landing-hero__content\">{content}</div><div class=\"landing-hero__image\">{image}</div>"
        )
    } else {
        return format!(
            "<section class=\"landing-hero landing-hero--{align}\">{content}</section>"
        );
    };
    format!(
        "<section class=\"landing-hero landing-hero--{align} landing-hero--split{}\" style=\"--hero-image-weight: {percent}fr; --hero-text-weight: {}fr\">{body}</section>",
        if reverse {
            " landing-hero--reverse"
        } else {
            ""
        },
        100.0 - percent
    )
}

/// Select only a top-level paragraph containing a single image. Inline images,
/// code examples, lists and block quotes remain part of the ordinary content.
fn split_hero_image(markdown: &str) -> Option<(String, String, f32)> {
    let parser = Parser::new(markdown);
    let definitions = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| &markdown[definition.span.clone()])
        .collect::<Vec<_>>()
        .join("\n");
    let mut depth = 0;
    let mut paragraph = None;
    let mut image_count = 0;
    let mut image_depth = 0;
    let mut image_only = false;
    let mut percent = 50.0;
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 && tag == Tag::Paragraph {
                    paragraph = Some(range.clone());
                    image_count = 0;
                    image_only = true;
                    percent = 50.0;
                } else if paragraph.is_some() && image_depth == 0 {
                    if let Tag::Image { dest_url, .. } = &tag {
                        if let Some(crate::media::MediaRef::Image(image)) =
                            crate::media::parse_media_destination(dest_url, "")
                        {
                            percent = image
                                .maxw
                                .as_deref()
                                .and_then(|width| width.strip_suffix('%'))
                                .and_then(|width| width.parse::<f32>().ok())
                                .filter(|width| *width > 0.0 && *width < 100.0)
                                .unwrap_or(50.0);
                        }
                        image_count += 1;
                        image_depth = depth + 1;
                    } else {
                        image_only = false;
                    }
                }
                depth += 1;
            }
            Event::End(tag) => {
                if depth == image_depth {
                    image_depth = 0;
                }
                depth -= 1;
                if tag == TagEnd::Paragraph && depth == 0 {
                    if let Some(paragraph) = paragraph.take() {
                        if image_only && image_count == 1 {
                            let span = paragraph.start..range.end;
                            let mut text = markdown.to_string();
                            text.replace_range(span.clone(), "");
                            let media = format!("{}\n\n{definitions}", &markdown[span]);
                            return Some((text, media, percent));
                        }
                    }
                }
            }
            Event::Text(text) if image_depth == 0 && !text.trim().is_empty() => image_only = false,
            Event::SoftBreak | Event::HardBreak => {}
            _ if image_depth == 0 => image_only = false,
            _ => {}
        }
    }
    None
}

fn render_features(
    component: &Component,
    rel: &str,
    render_markdown: &impl Fn(&str, Option<f32>) -> String,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    let mut out = format!(
        "<section class=\"landing-features\"{}>",
        folding_attribute(component)
    );
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
            cards.push_str(&render_component(card, rel, render_markdown, render_image));
        } else {
            flush_cards(&mut out, &mut cards);
            out.push_str(&render_nodes(
                std::slice::from_ref(child),
                rel,
                render_markdown,
                render_image,
            ));
        }
    }
    flush_cards(&mut out, &mut cards);
    out.push_str("</section>");
    out
}

fn render_card(
    component: &Component,
    content: &str,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    let icon = render_icon(component, render_image);
    let folded = folding_attribute(component);
    format!(
        "<article class=\"landing-card\"{folded}>{icon}<div class=\"landing-card__content\">{content}</div></article>"
    )
}

fn folding_attribute(component: &Component) -> String {
    allowed_attr(component, "folded", &["yes", "no", "auto", "off"])
        .map(|value| format!(" data-folded=\"{value}\""))
        .unwrap_or_default()
}

fn render_icon(
    component: &Component,
    render_image: &impl Fn(&str, ComponentImage) -> String,
) -> String {
    attr(component, "icon")
        .map(|path| {
            let is_svg = path.to_ascii_lowercase().ends_with(".svg");
            let kind = match (component.name.as_str(), is_svg) {
                ("section", true) => ComponentImage::SectionIcon,
                ("section", false) => ComponentImage::SectionImage,
                (_, true) => ComponentImage::CardIcon,
                (_, false) => ComponentImage::CardImage,
            };
            let suffix = if is_svg { "icon" } else { "image" };
            format!(
                "<div class=\"landing-{}__{suffix}\">{}</div>",
                component.name,
                render_image(path, kind)
            )
        })
        .unwrap_or_default()
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
    fn section_and_features_folding_validate_and_keep_content_visible() {
        for component in ["section", "features"] {
            for mode in ["yes", "no", "auto", "off"] {
                let source = format!(
                    "::: {component} folded={mode}\n## Group\n\nIntroduction.\n\n::: features\n::: card folded=no\n### Card\n\nCard body.\n:::\n:::\n:::\n"
                );
                assert!(validate(&source, true).is_empty(), "{component}: {mode}");
                let html = render(&source, "", crate::render::render_markdown_to_html);
                assert!(html.contains(&format!("data-folded=\"{mode}\"")));
                assert!(html.contains("<h2>Group</h2>"));
                assert!(html.contains("<h3>Card</h3>"));
                assert!(html.contains("<p>Card body.</p>"));
                assert!(!html.contains("hidden"));
                assert!(!html.contains("<button"));
            }
            for value in ["true", "false", "invalid"] {
                let source = format!("::: {component} folded={value}\n## Group\n:::\n");
                assert_eq!(validate(&source, true).len(), 1);
                assert!(!render(&source, "", str::to_string).contains("data-folded"));
            }
            let source = format!("::: {component}\n## Group\n:::\n");
            assert!(!render(&source, "", str::to_string).contains("data-folded"));
        }
        let source = "::: section folded=yes image-align=left icon=images/icon.svg\n## Split heading\n\nIntro text.\n\n![Image](images/photo.png)\n:::\n";
        assert!(validate(source, true).is_empty());
        let html = render(source, "", crate::render::render_markdown_to_html);
        assert!(html.contains("data-folded=\"yes\""));
        assert!(html.contains("landing-section__intro"));
        assert!(html.contains("<h2>Split heading</h2>"));
    }

    #[test]
    fn card_folding_accepts_only_supported_values() {
        for value in ["yes", "no", "auto", "off", "\"auto\""] {
            let source =
                format!("::: features\n::: card folded={value}\n### Card\n\nBody\n:::\n:::\n");
            assert!(validate(&source, true).is_empty(), "{value}");
        }
        for attribute in [
            "folded",
            "folded=true",
            "folded=false",
            "folded=",
            "folded=YES",
        ] {
            let source = format!("::: features\n::: card {attribute}\n### Card\n:::\n:::\n");
            assert!(!validate(&source, true).is_empty(), "{attribute}");
            let html = render(&source, "", crate::render::render_markdown_to_html);
            assert!(!html.contains("data-folded"));
        }
    }

    #[test]
    fn card_folding_preserves_visible_content_without_javascript() {
        for value in ["yes", "no", "auto", "off"] {
            let source = format!(
                "::: features\n::: card folded={value} icon=images/test.svg\n### Card *title*\n\n[Read more](details)\n:::\n:::\n"
            );
            let html = render(&source, "", crate::render::render_markdown_to_html);
            assert!(html.contains(&format!("data-folded=\"{value}\"")));
            assert!(html.contains("<h3>Card <em>title</em></h3>"));
            assert!(html.contains("<a href=\"details\">Read more</a>"));
            assert!(html.contains("src=\"images/test.svg\""));
            assert!(!html.contains("hidden"));
            assert!(!html.contains("<button"));
        }
        let html = render(
            "::: features\n::: card\n### Card\n:::\n:::\n",
            "",
            crate::render::render_markdown_to_html,
        );
        assert!(html.contains("<article class=\"landing-card\">"));
        assert!(!html.contains("data-folded"));
    }

    #[test]
    fn section_icons_validate_render_and_resolve_like_card_icons() {
        for path in ["images/icons/a & b.svg", "images/photo.png"] {
            let source =
                format!("::: section tone=subtle icon=\"{path}\"\n## Heading\n\nContent.\n:::\n");
            assert!(validate(&source, true).is_empty());
            let html = render(&source, "../", crate::render::render_markdown_to_html);
            let suffix = if path.ends_with(".svg") {
                "icon"
            } else {
                "image"
            };
            assert!(html.starts_with(&format!(
                "<section class=\"landing-section landing-section--subtle\"><div class=\"landing-section__{suffix}\">"
            )));
            assert!(html.contains(&format!(
                "src=\"../{}\" alt=\"\"",
                path.replace('&', "&amp;")
            )));
            assert!(html.find("</div>").unwrap() < html.find("<h2>").unwrap());
            assert_eq!(collect_media_refs(&source).len(), 1);
        }
        assert_eq!(validate("::: section icon\nContent\n:::", true).len(), 1);
        assert!(
            !render("::: section\nContent\n:::", "", str::to_string)
                .contains("landing-section__icon")
        );
    }

    #[test]
    fn section_icon_callback_distinguishes_svg_and_raster_media() {
        for (path, kind) in [
            ("images/blocks.SVG", ComponentImage::SectionIcon),
            ("images/photo.png", ComponentImage::SectionImage),
        ] {
            let html = render_with_media(
                &format!("::: section icon={path}\nContent\n:::\n"),
                "../",
                |text, _| text.to_string(),
                |actual_path, actual_kind| {
                    assert_eq!(actual_path, path);
                    assert_eq!(actual_kind, kind);
                    "<planned-image>".to_string()
                },
            );
            assert!(html.contains("<planned-image>"));
        }
    }

    #[test]
    fn centered_section_preserves_image_position_and_normal_width_constraints() {
        let source = "::: section image-align=center\n## Heading\n\nText above.\n\n![Illustration](images/example.png;maxw=50%)\n\nText below.\n\n::: features\n::: card\n### Card\nCard text.\n:::\n:::\n:::\n";
        assert!(validate(source, true).is_empty());
        let html = render_with_context(source, "", |markdown, width| {
            assert_eq!(width, None);
            crate::render::render_markdown_to_html(markdown)
        });
        assert!(html.contains("landing-section--image-center"));
        assert!(!html.contains("landing-hero--split"));
        assert!(html.find("Text above.").unwrap() < html.find("<img ").unwrap());
        assert!(html.find("<img ").unwrap() < html.find("Text below.").unwrap());
        assert!(html.find("Text below.").unwrap() < html.find("landing-features__cards").unwrap());
        assert!(html.contains("images/example.png;maxw=50%"));
        assert!(html.contains("alt=\"Illustration\""));
    }

    #[test]
    fn section_image_alignment_splits_intro_and_keeps_cards_below() {
        for side in ["left", "right"] {
            let source = format!(
                "::: section tone=subtle image-align={side}\n## Privacy\n\n![Private](images/privacy.png;maxw=60%)\n\nSome **text**.\n\n::: features\n::: card\n### Card\nText\n:::\n:::\n:::\n"
            );
            assert!(validate(&source, true).is_empty());
            let html = render_with_context(&source, "", |markdown, width| {
                if markdown.contains("![Private]") {
                    assert_eq!(width, Some(60.0));
                } else {
                    assert_eq!(width, None);
                }
                crate::render::render_markdown_to_html(markdown)
            });
            assert!(html.contains("landing-section--subtle"));
            assert!(html.contains("landing-section__intro landing-hero--split"));
            assert_eq!(html.contains("landing-hero--reverse"), side == "left");
            assert!(html.contains("--hero-image-weight: 60fr; --hero-text-weight: 40fr"));
            assert!(html.contains("<strong>text</strong>"));
            assert!(html.contains("alt=\"Private\""));
            assert_eq!(html.matches("<img ").count(), 1);
            assert!(
                html.find("landing-hero__content").unwrap()
                    < html.find("landing-hero__image").unwrap()
            );
            assert!(html.contains("</div></div><section class=\"landing-features\">"));
        }
    }

    #[test]
    fn section_images_split_only_when_requested_and_standalone_in_intro() {
        for (attribute, markdown) in [
            ("", "![Private](images/privacy.png)"),
            (" image-align=left", "Text ![Private](images/privacy.png)"),
            (" image-align=right", "> ![Private](images/privacy.png)"),
            (" image-align=left", "No image"),
            (
                " image-align=right",
                "::: features\n![Private](images/privacy.png)\n:::",
            ),
        ] {
            let html = render(
                &format!("::: section{attribute}\n{markdown}\n:::\n"),
                "",
                crate::render::render_markdown_to_html,
            );
            assert!(!html.contains("landing-section__intro"), "{markdown}");
        }
        for component in ["section", "hero"] {
            for side in ["left", "right"] {
                assert!(
                    validate(
                        &format!("::: {component} image-align={side}\nText\n:::\n"),
                        true
                    )
                    .is_empty()
                );
            }
            assert_eq!(
                validate(
                    &format!("::: {component} image-align=center\nText\n:::\n"),
                    true
                )
                .len(),
                usize::from(component == "hero")
            );
        }
    }

    #[test]
    fn hero_image_alignment_overrides_reverse_and_preserves_centered_layout() {
        for (attributes, reverse, split) in [
            ("image-align=left", true, true),
            ("image-align=right", false, true),
            ("reverse image-align=right", false, true),
            ("align=center image-align=left", false, false),
        ] {
            let html = render(
                &format!("::: hero {attributes}\n# Hero\n\n![Image](images/hero.png)\n:::\n"),
                "",
                crate::render::render_markdown_to_html,
            );
            assert_eq!(
                html.contains("landing-hero--reverse"),
                reverse,
                "{attributes}"
            );
            assert_eq!(html.contains("landing-hero--split"), split, "{attributes}");
        }
    }

    #[test]
    fn discovers_nested_component_images_but_ignores_code_and_remote_assets() {
        let refs = collect_media_refs(
            "::: section\n::: features\n::: card icon=\"images/cards/a b.png\"\nCard\n:::\n::: card icon=images/icons/check.svg\nIcon\n:::\n::: card icon=https://example.com/remote.png\nRemote\n:::\n:::\n:::\n::: feature image=images/features/demo.jpg\nFeature\n:::\n```\n::: card icon=images/code.png\n:::\n```\n",
        );
        let paths = refs
            .iter()
            .map(|media| match media {
                crate::media::MediaRef::Image(image) => image.path.raw.as_str(),
                _ => panic!("expected image"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                "images/cards/a b.png",
                "images/icons/check.svg",
                "images/features/demo.jpg"
            ]
        );
    }

    #[test]
    fn component_image_callback_distinguishes_photos_icons_and_features() {
        let html = render_with_media(
            "::: features\n::: card icon=images/cards/photo.png\nPhoto\n:::\n::: card icon=images/icons/check.svg\nIcon\n:::\n:::\n::: feature image=images/demo.jpg\nFeature\n:::\n",
            "../",
            |text, _| text.to_string(),
            |path, kind| format!("<media data-kind=\"{kind:?}\">{path}</media>"),
        );
        assert!(html.contains("class=\"landing-card__image\"><media data-kind=\"CardImage\""));
        assert!(html.contains("class=\"landing-card__icon\"><media data-kind=\"CardIcon\""));
        assert!(html.contains("class=\"landing-feature__image\"><media data-kind=\"Feature\""));
    }

    #[test]
    fn fallback_component_images_escape_attributes() {
        let html = render(
            "::: card icon=images/a&b.png\nCard\n:::\n",
            "../",
            str::to_string,
        );
        assert!(html.contains("src=\"../images/a&amp;b.png\" alt=\"\""));
    }

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
    #[test]
    fn hero_splits_standalone_image_and_preserves_markdown() {
        let html = render(
            "::: hero reverse\n# Product\n\nSome **text** and [Download](download/).\n\n![Screen](images/screen.png)\n:::\n",
            "",
            crate::render::render_markdown_to_html,
        );
        assert!(html.contains("landing-hero--split landing-hero--reverse"));
        assert!(html.contains("<strong>text</strong>"));
        assert!(html.contains("href=\"download/\""));
        assert!(html.contains("alt=\"Screen\""));
        assert!(
            html.find("landing-hero__content").unwrap() < html.find("landing-hero__image").unwrap()
        );
        assert_eq!(html.matches("<img ").count(), 1);
        assert!(validate("::: hero reverse\nText\n:::\n", true).is_empty());
        assert_eq!(validate("::: hero reverse=yes\nText\n:::\n", true).len(), 1);
    }

    #[test]
    fn hero_preserves_reference_images_and_links() {
        let html = render(
            "::: hero\n# Product\n\n[Download][download]\n\n![Screen][screen]\n\n[screen]: images/screen.png\n[download]: download/\n:::\n",
            "",
            crate::render::render_markdown_to_html,
        );
        assert!(html.contains("landing-hero--split"));
        assert!(html.contains("src=\"images/screen.png\""));
        assert!(html.contains("href=\"download/\""));
    }

    #[test]
    fn hero_leaves_inline_nested_and_code_images_in_content() {
        for markdown in [
            "Text ![Screen](images/screen.png)",
            "> ![Screen](images/screen.png)",
            "- ![Screen](images/screen.png)",
            "```\n![Screen](images/screen.png)\n```",
            "No image",
            "![A](images/a.png) ![B](images/b.png)",
        ] {
            let html = render(
                &format!("::: hero\n{markdown}\n:::\n"),
                "",
                crate::render::render_markdown_to_html,
            );
            assert!(!html.contains("landing-hero--split"), "{markdown}");
        }
    }

    #[test]
    fn centered_hero_keeps_stacked_presentation() {
        let html = render(
            "::: hero align=center\n# Product\n\n![Screen](images/screen.png)\n:::\n",
            "",
            crate::render::render_markdown_to_html,
        );
        assert!(html.contains("landing-hero--center"));
        assert!(!html.contains("landing-hero--split"));
    }
    #[test]
    fn hero_percentage_sizes_column_and_reaches_media_renderer() {
        for reverse in ["", " reverse"] {
            let html = render_with_context(
                &format!(
                    "::: hero{reverse}\n# Product\n\n![Screen](images/screen.png;maxw=60%)\n:::\n"
                ),
                "",
                |markdown, width| {
                    if markdown.contains("![Screen]") {
                        assert_eq!(width, Some(60.0));
                    } else {
                        assert_eq!(width, None);
                    }
                    crate::render::render_markdown_to_html(markdown)
                },
            );
            assert!(html.contains("--hero-image-weight: 60fr; --hero-text-weight: 40fr"));
        }
    }
}
