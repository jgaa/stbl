use anyhow::Result;

use crate::blog_index::{canonical_tag_map, collect_blog_posts, iter_visible_posts, tag_key};
use crate::model::{Page, Project};
use crate::templates::format_timestamp_ymd;
use crate::url::{UrlMapper, logical_key_from_source_path};
use crate::visibility::is_blog_index_excluded;
use pulldown_cmark::{Options as CmarkOptions, Parser};
use pulldown_cmark_toc::{HeadingLevel, Options as TocOptions, TableOfContents};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct MacroContext<'a> {
    pub project: &'a Project,
    pub page: Option<&'a Page>,
    pub include_provider: Option<&'a dyn IncludeProvider>,
    pub render_markdown: Option<&'a dyn MarkdownRenderer>,
    pub render_media: Option<&'a dyn MediaRenderer>,
}

pub trait MarkdownRenderer {
    fn render(&self, md: &str) -> String;
}

pub struct RenderedMedia {
    pub html: String,
    pub maxw: Option<String>,
    pub maxh: Option<String>,
}

pub trait MediaRenderer {
    fn render(&self, dest_url: &str, alt: &str) -> RenderedMedia;
}

pub fn expand_macros(input_md: &str, ctx: &MacroContext<'_>) -> Result<String> {
    expand_macros_with_options(input_md, ctx, MacroExpandOptions::default())
}

#[derive(Debug, Clone, Copy)]
pub struct MacroExpandOptions {
    pub enabled: bool,
    pub max_passes: usize,
}

impl Default for MacroExpandOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_passes: 1,
        }
    }
}

pub struct IncludeRequest<'a> {
    pub path: &'a str,
    pub current_source_path: Option<&'a Path>,
    pub site_root: Option<&'a Path>,
}

pub struct IncludeResponse {
    pub content: String,
    pub resolved_id: String,
}

pub trait IncludeProvider {
    fn include(&self, request: &IncludeRequest<'_>) -> Result<IncludeResponse>;
}

pub fn expand_macros_with_options(
    input_md: &str,
    ctx: &MacroContext<'_>,
    options: MacroExpandOptions,
) -> Result<String> {
    if !options.enabled {
        return Ok(input_md.to_string());
    }
    let mut state = ExpandState::default();
    let mut current = input_md.to_string();
    for _ in 0..options.max_passes.max(1) {
        let expanded = expand_macros_once(&current, ctx, &options, &mut state);
        if !expanded.contains("@[") {
            return Ok(expanded);
        }
        if expanded == current {
            return Ok(expanded);
        }
        current = expanded;
    }
    Ok(current)
}

#[derive(Default)]
struct ExpandState {
    include_stack: Vec<String>,
    include_once: HashSet<String>,
}

const MAX_INCLUDE_DEPTH: usize = 16;

fn expand_macros_once(
    input_md: &str,
    ctx: &MacroContext<'_>,
    options: &MacroExpandOptions,
    state: &mut ExpandState,
) -> String {
    let mut out = String::with_capacity(input_md.len());
    let mut idx = 0;
    while idx < input_md.len() {
        if input_md[idx..].starts_with("@[") {
            if let Some(invocation) = parse_macro_invocation(input_md, idx) {
                if let Some(expanded) = expand_macro(&invocation, ctx, options, state, input_md) {
                    out.push_str(&expanded);
                } else {
                    out.push_str(invocation.raw());
                }
                idx = invocation.end();
                continue;
            }
        }
        let ch = input_md[idx..].chars().next().unwrap();
        out.push(ch);
        idx += ch.len_utf8();
    }
    out
}

enum MacroInvocation<'a> {
    Inline {
        name: &'a str,
        args: Option<&'a str>,
        raw: &'a str,
        end: usize,
    },
    Block {
        name: &'a str,
        args: Option<&'a str>,
        body: &'a str,
        raw: &'a str,
        end: usize,
    },
}

impl<'a> MacroInvocation<'a> {
    fn name(&self) -> &'a str {
        match self {
            MacroInvocation::Inline { name, .. } => name,
            MacroInvocation::Block { name, .. } => name,
        }
    }

    fn args(&self) -> Option<&'a str> {
        match self {
            MacroInvocation::Inline { args, .. } => *args,
            MacroInvocation::Block { args, .. } => *args,
        }
    }

    fn body(&self) -> Option<&'a str> {
        match self {
            MacroInvocation::Inline { .. } => None,
            MacroInvocation::Block { body, .. } => Some(*body),
        }
    }

    fn raw(&self) -> &'a str {
        match self {
            MacroInvocation::Inline { raw, .. } => raw,
            MacroInvocation::Block { raw, .. } => raw,
        }
    }

    fn end(&self) -> usize {
        match self {
            MacroInvocation::Inline { end, .. } => *end,
            MacroInvocation::Block { end, .. } => *end,
        }
    }
}

fn parse_macro_invocation(input: &str, start: usize) -> Option<MacroInvocation<'_>> {
    let name_start = start + 2;
    let name_end = input[name_start..]
        .find(']')
        .map(|offset| name_start + offset)?;
    let name = input[name_start..name_end].trim();
    if name.is_empty() {
        return None;
    }

    let mut end = name_end + 1;
    let mut args = None;
    if input[end..].starts_with('(') {
        let args_start = end + 1;
        let args_end = find_args_end(input, end)?;
        args = Some(&input[args_start..args_end]);
        end = args_end + 1;
    }

    if let Some(body_start) = block_body_start(input, end) {
        if let Some((tag_start, tag_end)) = find_block_end(input, body_start, name) {
            return Some(MacroInvocation::Block {
                name,
                args,
                body: &input[body_start..tag_start],
                raw: &input[start..tag_end],
                end: tag_end,
            });
        }
    }

    Some(MacroInvocation::Inline {
        name,
        args,
        raw: &input[start..end],
        end,
    })
}

fn expand_macro(
    invocation: &MacroInvocation<'_>,
    ctx: &MacroContext<'_>,
    options: &MacroExpandOptions,
    state: &mut ExpandState,
    input_md: &str,
) -> Option<String> {
    let name = invocation.name().trim().to_lowercase();
    let _body = invocation.body();
    match name.as_str() {
        "blogitems" => Some(expand_blogitems(invocation.args(), ctx)),
        "include" => Some(expand_include(invocation.args(), ctx, options, state)),
        "note" | "tip" | "info" | "warning" | "danger" => expand_callout(invocation, &name, ctx),
        "quote" => expand_quote(invocation, ctx),
        "figure" => Some(expand_figure(invocation.args(), ctx)),
        "kbd" => Some(expand_kbd_key(invocation, "kbd")),
        "key" => Some(expand_kbd_key(invocation, "key")),
        "tags" => Some(expand_tags(invocation.args(), ctx)),
        "series" => Some(expand_series(invocation.args(), ctx)),
        "related" => Some(expand_related(invocation.args(), ctx)),
        "toc" => Some(expand_toc(invocation.args(), input_md)),
        _ => None,
    }
}

fn expand_include(
    args: Option<&str>,
    ctx: &MacroContext<'_>,
    options: &MacroExpandOptions,
    state: &mut ExpandState,
) -> String {
    let mut path = None;
    let mut once = false;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "path" => {
                if !value.is_empty() {
                    path = Some(value);
                }
            }
            "once" => {
                if let Some(parsed) = parse_bool(&value) {
                    once = parsed;
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro include: unknown key '{key}' ignored");
    }

    let Some(path) = path else {
        eprintln!("macro include: missing required path");
        return warning_block("include macro requires a path argument");
    };

    let provider = match ctx.include_provider {
        Some(provider) => provider,
        None => {
            eprintln!("macro include: include_provider missing");
            return warning_block("include macro is not enabled for this build");
        }
    };

    if state.include_stack.len() >= MAX_INCLUDE_DEPTH {
        eprintln!("macro include: max include depth exceeded for '{path}'");
        return warning_block("include macro exceeded maximum include depth");
    }

    let request = IncludeRequest {
        path: &path,
        current_source_path: ctx.page.map(|page| Path::new(&page.source_path)),
        site_root: Some(ctx.project.root.as_path()),
    };

    let response = match provider.include(&request) {
        Ok(response) => response,
        Err(err) => {
            eprintln!("macro include: failed to load '{path}': {err}");
            return warning_block("include macro failed to load the requested file");
        }
    };

    if state.include_stack.contains(&response.resolved_id) {
        eprintln!(
            "macro include: recursion detected for '{}'",
            response.resolved_id
        );
        return warning_block("include macro recursion detected");
    }

    if once && state.include_once.contains(&response.resolved_id) {
        return String::new();
    }
    if once {
        state.include_once.insert(response.resolved_id.clone());
    }

    state.include_stack.push(response.resolved_id);
    let expanded = expand_macros_with_state(&response.content, ctx, options, state);
    state.include_stack.pop();
    expanded
}

fn expand_macros_with_state(
    input_md: &str,
    ctx: &MacroContext<'_>,
    options: &MacroExpandOptions,
    state: &mut ExpandState,
) -> String {
    if !options.enabled {
        return input_md.to_string();
    }
    let mut current = input_md.to_string();
    for _ in 0..options.max_passes.max(1) {
        let expanded = expand_macros_once(&current, ctx, options, state);
        if !expanded.contains("@[") {
            return expanded;
        }
        if expanded == current {
            return expanded;
        }
        current = expanded;
    }
    current
}

fn expand_callout(
    invocation: &MacroInvocation<'_>,
    kind: &str,
    ctx: &MacroContext<'_>,
) -> Option<String> {
    let body = invocation.body()?;
    let mut title = None;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(invocation.args().unwrap_or_default()) {
        match key.as_str() {
            "title" => {
                if !value.is_empty() {
                    title = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro {kind}: unknown key '{key}' ignored");
    }

    let renderer = match ctx.render_markdown {
        Some(renderer) => renderer,
        None => {
            eprintln!("macro {kind}: render_markdown missing");
            return Some(warning_block("callout macro requires a markdown renderer"));
        }
    };

    let body_html = renderer.render(body);
    let mut out = String::new();
    out.push_str("<aside class=\"callout callout-");
    out.push_str(kind);
    out.push_str("\">");
    if let Some(title) = title {
        out.push_str("<div class=\"callout-title\">");
        out.push_str(&escape_html_text(&title));
        out.push_str("</div>");
    }
    out.push_str("<div class=\"callout-body\">");
    out.push_str(&body_html);
    out.push_str("</div></aside>");
    Some(out)
}

fn expand_quote(invocation: &MacroInvocation<'_>, ctx: &MacroContext<'_>) -> Option<String> {
    let body = invocation.body()?;
    let mut author = None;
    let mut source = None;
    let mut href = None;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(invocation.args().unwrap_or_default()) {
        match key.as_str() {
            "author" => {
                if !value.is_empty() {
                    author = Some(value);
                }
            }
            "source" => {
                if !value.is_empty() {
                    source = Some(value);
                }
            }
            "href" => {
                if !value.is_empty() {
                    href = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro quote: unknown key '{key}' ignored");
    }

    let renderer = match ctx.render_markdown {
        Some(renderer) => renderer,
        None => {
            eprintln!("macro quote: render_markdown missing");
            return Some(warning_block("quote macro requires a markdown renderer"));
        }
    };

    let body_html = renderer.render(body);
    let mut out = String::new();
    out.push_str("<figure class=\"quote\">");
    out.push_str("<blockquote class=\"quote-body\">");
    out.push_str(&body_html);
    out.push_str("</blockquote>");

    let author_text = author.as_deref();
    let source_text = source.as_deref();
    let href_text = href.as_deref();
    if author_text.is_some() || source_text.is_some() || href_text.is_some() {
        out.push_str("<figcaption class=\"quote-caption\">&mdash; ");
        if let Some(author) = author_text {
            out.push_str(&escape_html_text(author));
        }
        if source_text.is_some() || href_text.is_some() {
            if author_text.is_some() {
                out.push_str(", ");
            }
            let label = source_text.or(href_text);
            if let Some(label) = label {
                if let Some(href) = href_text {
                    out.push_str("<a href=\"");
                    out.push_str(&escape_attr(href));
                    out.push_str("\">");
                    out.push_str(&escape_html_text(label));
                    out.push_str("</a>");
                } else {
                    out.push_str(&escape_html_text(label));
                }
            }
        }
        out.push_str("</figcaption>");
    }

    out.push_str("</figure>");
    Some(out)
}

fn expand_figure(args: Option<&str>, ctx: &MacroContext<'_>) -> String {
    let mut src = None;
    let mut caption = None;
    let mut alt = None;
    let mut class = None;
    let mut maxw = None;
    let mut maxh = None;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "src" => {
                if !value.is_empty() {
                    src = Some(value);
                }
            }
            "caption" => {
                if !value.is_empty() {
                    caption = Some(value);
                }
            }
            "alt" => {
                if !value.is_empty() {
                    alt = Some(value);
                }
            }
            "class" => {
                if !value.is_empty() {
                    class = Some(value);
                }
            }
            "maxw" => {
                if !value.is_empty() {
                    maxw = Some(value);
                }
            }
            "maxh" => {
                if !value.is_empty() {
                    maxh = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro figure: unknown key '{key}' ignored");
    }

    let Some(src) = src else {
        eprintln!("macro figure: missing required src");
        return warning_block("figure macro requires a src argument");
    };

    let renderer = match ctx.render_media {
        Some(renderer) => renderer,
        None => {
            eprintln!("macro figure: render_media missing");
            return warning_block("figure macro requires a media renderer");
        }
    };

    let mut dest = src.clone();
    if let Some(maxw) = maxw.as_ref() {
        dest.push_str(";maxw=");
        dest.push_str(maxw);
    }
    if let Some(maxh) = maxh.as_ref() {
        dest.push_str(";maxh=");
        dest.push_str(maxh);
    }

    let alt = alt.unwrap_or_default();
    let rendered = renderer.render(&dest, &alt);

    let mut class_parts = Vec::new();
    class_parts.push("figure".to_string());
    if let Some(class) = class.as_deref() {
        for token in class.split_whitespace() {
            if token.is_empty() {
                continue;
            }
            class_parts.push(format!("figure-{token}"));
        }
    }
    let class_attr = escape_attr(&class_parts.join(" "));

    let mut out = String::new();
    out.push_str("<figure class=\"");
    out.push_str(&class_attr);
    out.push('"');

    if rendered.maxw.is_some() || rendered.maxh.is_some() {
        let mut style = String::new();
        if let Some(maxw) = rendered.maxw.as_ref() {
            style.push_str("--media-maxw: ");
            style.push_str(maxw);
            style.push_str("; ");
        }
        if let Some(maxh) = rendered.maxh.as_ref() {
            style.push_str("--media-maxh: ");
            style.push_str(maxh);
            style.push_str("; ");
        }
        if !style.is_empty() {
            out.push_str(" style=\"");
            out.push_str(style.trim());
            out.push('"');
        }
    }
    out.push('>');
    out.push_str(&rendered.html);
    if let Some(caption) = caption {
        let caption = caption.trim();
        if !caption.is_empty() {
            out.push_str("<figcaption>");
            out.push_str(&escape_html_text(caption));
            out.push_str("</figcaption>");
        }
    }
    out.push_str("</figure>");
    out
}

fn expand_kbd_key(invocation: &MacroInvocation<'_>, class: &str) -> String {
    let mut text = None;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(invocation.args().unwrap_or_default()) {
        match key.as_str() {
            "text" => {
                if !value.is_empty() {
                    text = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro {class}: unknown key '{key}' ignored");
    }

    if text.is_none() {
        if let Some(body) = invocation.body() {
            let body = body.trim();
            if !body.is_empty() {
                text = Some(body.to_string());
            }
        }
    }

    let Some(text) = text else {
        return String::new();
    };

    let mut out = String::new();
    out.push_str("<kbd class=\"");
    out.push_str(class);
    out.push_str("\">");
    out.push_str(&escape_html_text(&text));
    out.push_str("</kbd>");
    out
}

fn find_args_end(input: &str, open_paren: usize) -> Option<usize> {
    let mut idx = open_paren + 1;
    let mut in_quotes = false;
    let mut escape = false;
    while idx < input.len() {
        let (ch, len) = next_char(input, idx)?;
        if escape {
            escape = false;
            idx += len;
            continue;
        }
        if in_quotes {
            match ch {
                '\\' => escape = true,
                '"' => in_quotes = false,
                _ => {}
            }
            idx += len;
            continue;
        }

        match ch {
            '"' => {
                in_quotes = true;
                idx += len;
            }
            ')' => return Some(idx),
            _ => idx += len,
        }
    }
    None
}

fn block_body_start(input: &str, after_open: usize) -> Option<usize> {
    let mut idx = after_open;
    let mut saw_whitespace = false;
    while let Some((ch, len)) = next_char(input, idx) {
        if ch == ' ' || ch == '\t' {
            saw_whitespace = true;
            idx += len;
            continue;
        }
        if ch == '\n' || ch == '\r' {
            let mut body_start = idx + len;
            if ch == '\r' {
                if let Some((next_ch, next_len)) = next_char(input, body_start) {
                    if next_ch == '\n' {
                        body_start += next_len;
                    }
                }
            }
            return Some(body_start);
        }
        if saw_whitespace || ch != '\n' {
            return Some(idx);
        }
        break;
    }
    None
}

fn find_block_end(input: &str, search_start: usize, name: &str) -> Option<(usize, usize)> {
    let mut idx = search_start;
    while let Some(offset) = input[idx..].find("@[/") {
        let tag_start = idx + offset;
        let name_start = tag_start + 3;
        let name_end_rel = input[name_start..].find(']')?;
        let name_end = name_start + name_end_rel;
        let close_name = input[name_start..name_end].trim();
        if !close_name.is_empty() && close_name.eq_ignore_ascii_case(name) {
            return Some((tag_start, name_end + 1));
        }
        idx = name_end + 1;
    }
    None
}

fn expand_blogitems(args: Option<&str>, ctx: &MacroContext<'_>) -> String {
    let mut items = 3u32;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "items" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    items = parsed;
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro blogitems: unknown key '{key}' ignored");
    }

    let items = items.clamp(1, 50) as usize;
    let source_page_id = ctx.page.map(|page| page.id);
    let posts = collect_blog_posts(ctx.project, source_page_id);
    let mapper = UrlMapper::new(&ctx.project.config);

    let mut out = String::new();
    out.push_str("<ul class=\"blogitems\">");
    for post in posts.into_iter().take(items) {
        let href = mapper.map(&post.logical_key).href;
        out.push_str("<li class=\"blogitem\"><a href=\"");
        out.push_str(&escape_attr(&href));
        out.push_str("\">");
        out.push_str(&escape_html_text(&post.title));
        out.push_str("</a>");
        if let Some(abstract_text) = post.abstract_text.as_deref() {
            let abstract_text = abstract_text.trim();
            if !abstract_text.is_empty() {
                out.push_str("<p class=\"blogitem-excerpt\">");
                out.push_str(&escape_html_text(abstract_text));
                out.push_str("</p>");
            }
        }
        out.push_str("</li>");
    }
    out.push_str("</ul>");
    out
}

fn expand_toc(args: Option<&str>, input_md: &str) -> String {
    let mut min_level = 2u32;
    let mut max_level = 3u32;
    let mut title = None;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "min" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    min_level = parsed;
                }
            }
            "max" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    max_level = parsed;
                }
            }
            "title" => {
                if !value.is_empty() {
                    title = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro toc: unknown key '{key}' ignored");
    }

    min_level = min_level.clamp(1, 6);
    max_level = max_level.clamp(1, 6);
    if min_level > max_level {
        std::mem::swap(&mut min_level, &mut max_level);
    }

    let levels = heading_level_from_u32(min_level)..=heading_level_from_u32(max_level);
    let options = TocOptions::default().levels(levels);
    let parser = Parser::new_ext(input_md, CmarkOptions::empty());
    let toc = TableOfContents::new_with_events(parser);
    let toc_md = toc.to_cmark_with_options(options);
    if toc_md.trim().is_empty() {
        return String::new();
    }

    let mut out = String::new();
    if let Some(title) = title {
        out.push_str("**");
        out.push_str(&escape_markdown_text(&title));
        out.push_str("**\n\n");
    }
    out.push_str(&toc_md);
    out
}

fn expand_tags(args: Option<&str>, ctx: &MacroContext<'_>) -> String {
    let Some(page) = ctx.page else {
        return String::new();
    };
    if page.header.tags.is_empty() {
        return String::new();
    }
    let canonical_tags = canonical_tag_map(ctx.project);

    let mut style = "inline".to_string();
    let mut sort = "site".to_string();
    let mut prefix = None;
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "style" => {
                if !value.is_empty() {
                    style = value.to_lowercase();
                }
            }
            "sort" => {
                if !value.is_empty() {
                    sort = value.to_lowercase();
                }
            }
            "prefix" => {
                if !value.is_empty() {
                    prefix = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro tags: unknown key '{key}' ignored");
    }

    let style_class = match style.as_str() {
        "pills" => "tags-pills",
        _ => "tags-inline",
    };

    let mut tags = Vec::new();
    let mut seen = HashSet::new();
    for tag in &page.header.tags {
        let key = tag_key(tag);
        if !seen.insert(key.clone()) {
            continue;
        }
        let label = canonical_tags
            .get(&key)
            .cloned()
            .unwrap_or_else(|| tag.clone());
        tags.push(label);
    }
    if sort == "alpha" {
        tags.sort_by(|a, b| tag_key(a).cmp(&tag_key(b)));
    }

    let mapper = UrlMapper::new(&ctx.project.config);
    let mut out = String::new();
    out.push_str("<div class=\"tags ");
    out.push_str(style_class);
    out.push_str("\">");
    if let Some(prefix) = prefix {
        out.push_str("<span class=\"tags-prefix\">");
        out.push_str(&escape_html_text(&prefix));
        out.push_str("</span>");
    }
    for tag in tags {
        let href = mapper.map(&format!("tags/{tag}")).href;
        out.push_str("<a class=\"tag\" href=\"");
        out.push_str(&escape_attr(&href));
        out.push_str("\">");
        out.push_str(&escape_html_text(&tag));
        out.push_str("</a>");
    }
    out.push_str("</div>");
    out
}

fn expand_series(args: Option<&str>, ctx: &MacroContext<'_>) -> String {
    let Some(page) = ctx.page else {
        return String::new();
    };
    let Some(match_info) = find_series_for_page(ctx.project, page) else {
        return String::new();
    };

    let mut nav = true;
    let mut list = false;
    let mut title = "Series".to_string();
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "nav" => {
                if let Some(parsed) = parse_bool(&value) {
                    nav = parsed;
                }
            }
            "list" => {
                if let Some(parsed) = parse_bool(&value) {
                    list = parsed;
                }
            }
            "title" => {
                if !value.is_empty() {
                    title = value;
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro series: unknown key '{key}' ignored");
    }

    let series = match_info.series;
    let series_name = series
        .index
        .header
        .title
        .clone()
        .unwrap_or_else(|| "Series".to_string());
    let total_parts = series.parts.len();
    if total_parts == 0 {
        return String::new();
    }

    let mut title_text = String::new();
    title_text.push_str(&title);
    title_text.push_str(": ");
    title_text.push_str(&series_name);
    if let Some(part_idx) = match_info.part_index {
        let part = &series.parts[part_idx];
        title_text.push_str(" - Part ");
        title_text.push_str(&part.part_no.to_string());
        title_text.push_str(" of ");
        title_text.push_str(&total_parts.to_string());
    } else {
        title_text.push_str(" - ");
        title_text.push_str(&format!("{total_parts} parts"));
    }

    let index_key = logical_key_from_source_path(&series.dir_path);
    let mapper = UrlMapper::new(&ctx.project.config);
    let index_href = crate::url::map_series_index(&index_key).href;

    let mut out = String::new();
    out.push_str("<nav class=\"series-nav\">");
    out.push_str("<div class=\"series-title\">");
    out.push_str(&escape_html_text(&title_text));
    out.push_str("</div>");

    if nav {
        out.push_str("<div class=\"series-links\">");
        if let Some(part_idx) = match_info.part_index {
            if part_idx > 0 {
                let prev = &series.parts[part_idx - 1];
                let prev_href = mapper
                    .map(&logical_key_from_source_path(&prev.page.source_path))
                    .href;
                out.push_str("<a rel=\"prev\" href=\"");
                out.push_str(&escape_attr(&prev_href));
                out.push_str("\">&lt;- Previous</a>");
            }
            out.push_str("<a href=\"");
            out.push_str(&escape_attr(&index_href));
            out.push_str("\">All parts</a>");
            if part_idx + 1 < total_parts {
                let next = &series.parts[part_idx + 1];
                let next_href = mapper
                    .map(&logical_key_from_source_path(&next.page.source_path))
                    .href;
                out.push_str("<a rel=\"next\" href=\"");
                out.push_str(&escape_attr(&next_href));
                out.push_str("\">Next -&gt;</a>");
            }
        } else {
            out.push_str("<a href=\"");
            out.push_str(&escape_attr(&index_href));
            out.push_str("\">All parts</a>");
        }
        out.push_str("</div>");
    }

    if list {
        out.push_str("<ol class=\"series-parts\">");
        for part in &series.parts {
            let href = mapper
                .map(&logical_key_from_source_path(&part.page.source_path))
                .href;
            let label = part
                .page
                .header
                .title
                .clone()
                .unwrap_or_else(|| format!("Part {}", part.part_no));
            out.push_str("<li><a href=\"");
            out.push_str(&escape_attr(&href));
            out.push_str("\">");
            out.push_str(&escape_html_text(&label));
            out.push_str("</a></li>");
        }
        out.push_str("</ol>");
    }

    out.push_str("</nav>");
    out
}

fn expand_related(args: Option<&str>, ctx: &MacroContext<'_>) -> String {
    let Some(page) = ctx.page else {
        return String::new();
    };

    let mut items = 5usize;
    let mut by = "both".to_string();
    let mut title = Some("Related".to_string());
    let mut unknown_keys = Vec::new();
    for (key, value) in parse_args(args.unwrap_or_default()) {
        match key.as_str() {
            "items" => {
                if let Ok(parsed) = value.parse::<usize>() {
                    items = parsed;
                }
            }
            "by" => {
                if !value.is_empty() {
                    by = value.to_lowercase();
                }
            }
            "title" => {
                if value.is_empty() {
                    title = None;
                } else {
                    title = Some(value);
                }
            }
            _ => unknown_keys.push(key),
        }
    }
    for key in unknown_keys {
        eprintln!("macro related: unknown key '{key}' ignored");
    }

    items = items.clamp(1, 50);
    let by_tags = matches!(by.as_str(), "tags" | "both");
    let by_series = matches!(by.as_str(), "series" | "both");

    let series_lookup = build_series_lookup(ctx.project);
    let current_series = series_lookup.get(&page.id).copied();
    let current_tags = page.header.tags.clone();

    let mut scored: Vec<RelatedCandidate<'_>> = Vec::new();
    for candidate in visible_related_pages(ctx.project) {
        if candidate.id == page.id {
            continue;
        }
        let mut score = 0i64;
        if by_series {
            if let (Some(cur), Some(other)) =
                (current_series, series_lookup.get(&candidate.id).copied())
            {
                if cur == other {
                    score += 100;
                }
            }
        }
        if by_tags && !current_tags.is_empty() && !candidate.header.tags.is_empty() {
            let shared = count_shared_tags(&current_tags, &candidate.header.tags);
            if shared > 0 {
                score += i64::from(shared) * 10;
            }
        }
        if score > 0 {
            scored.push(RelatedCandidate {
                page: candidate,
                score,
                published: page_sort_date(candidate),
            });
        }
    }

    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.published.cmp(&a.published))
            .then_with(|| a.page.source_path.cmp(&b.page.source_path))
    });

    let mut selected: Vec<&Page> = scored.iter().take(items).map(|item| item.page).collect();

    if selected.len() < items {
        for candidate in visible_related_pages_sorted(ctx.project) {
            if candidate.id == page.id {
                continue;
            }
            if selected.iter().any(|page| page.id == candidate.id) {
                continue;
            }
            selected.push(candidate);
            if selected.len() >= items {
                break;
            }
        }
    }

    if selected.is_empty() {
        return String::new();
    }

    let mapper = UrlMapper::new(&ctx.project.config);
    let mut out = String::new();
    out.push_str("<section class=\"related\">");
    if let Some(title) = title {
        if !title.trim().is_empty() {
            out.push_str("<h2>");
            out.push_str(&escape_html_text(&title));
            out.push_str("</h2>");
        }
    }
    out.push_str("<ul>");
    for related in selected {
        let href = mapper
            .map(&logical_key_from_source_path(&related.source_path))
            .href;
        let title = related
            .header
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        out.push_str("<li><a href=\"");
        out.push_str(&escape_attr(&href));
        out.push_str("\">");
        out.push_str(&escape_html_text(&title));
        out.push_str("</a>");
        if let Some(published) =
            format_timestamp_ymd(related.header.published.or(related.header.updated))
        {
            out.push_str("<span class=\"meta\">");
            out.push_str(&escape_html_text(&published));
            out.push_str("</span>");
        }
        out.push_str("</li>");
    }
    out.push_str("</ul></section>");
    out
}

struct RelatedCandidate<'a> {
    page: &'a Page,
    score: i64,
    published: i64,
}

fn build_series_lookup(project: &Project) -> HashMap<crate::model::DocId, crate::model::SeriesId> {
    let mut lookup = HashMap::new();
    for series in &project.content.series {
        lookup.insert(series.index.id, series.id);
        for part in &series.parts {
            lookup.insert(part.page.id, series.id);
        }
    }
    lookup
}

fn count_shared_tags(a: &[String], b: &[String]) -> u32 {
    let left = a.iter().map(|tag| tag_key(tag)).collect::<HashSet<_>>();
    b.iter()
        .map(|tag| tag_key(tag))
        .filter(|key| left.contains(key))
        .count() as u32
}

fn page_sort_date(page: &Page) -> i64 {
    page.header.published.or(page.header.updated).unwrap_or(0)
}

fn visible_related_pages<'a>(project: &'a Project) -> Vec<&'a Page> {
    let mut pages: Vec<&Page> = iter_visible_posts(project, None).collect();
    for series in &project.content.series {
        for part in &series.parts {
            if !is_blog_index_excluded(&part.page, None) {
                pages.push(&part.page);
            }
        }
    }
    pages
}

fn visible_related_pages_sorted<'a>(project: &'a Project) -> Vec<&'a Page> {
    let mut pages = visible_related_pages(project);
    pages.sort_by(|a, b| {
        page_sort_date(b)
            .cmp(&page_sort_date(a))
            .then_with(|| a.source_path.cmp(&b.source_path))
    });
    pages
}

struct SeriesMatch<'a> {
    series: &'a crate::model::Series,
    part_index: Option<usize>,
}

fn find_series_for_page<'a>(project: &'a Project, page: &Page) -> Option<SeriesMatch<'a>> {
    for series in &project.content.series {
        if series.index.id == page.id {
            return Some(SeriesMatch {
                series,
                part_index: None,
            });
        }
        if let Some((idx, _)) = series
            .parts
            .iter()
            .enumerate()
            .find(|(_, part)| part.page.id == page.id)
        {
            return Some(SeriesMatch {
                series,
                part_index: Some(idx),
            });
        }
    }
    None
}

fn parse_args(raw: &str) -> Vec<(String, String)> {
    let mut args = Vec::new();
    let mut idx = 0;
    while idx < raw.len() {
        while let Some((ch, len)) = next_char(raw, idx) {
            if ch == ',' || ch.is_whitespace() {
                idx += len;
            } else {
                break;
            }
        }
        if idx >= raw.len() {
            break;
        }

        let key_start = idx;
        while let Some((ch, len)) = next_char(raw, idx) {
            if ch == '=' || ch == ',' {
                break;
            }
            idx += len;
        }
        let key = raw[key_start..idx].trim().to_lowercase();
        if key.is_empty() {
            idx = skip_to_next_comma(raw, idx);
            continue;
        }

        if idx >= raw.len() {
            break;
        }
        let (ch, len) = match next_char(raw, idx) {
            Some(value) => value,
            None => break,
        };
        if ch != '=' {
            idx = skip_to_next_comma(raw, idx + len);
            continue;
        }
        idx += len;

        while let Some((ch, len)) = next_char(raw, idx) {
            if ch.is_whitespace() {
                idx += len;
            } else {
                break;
            }
        }
        if idx >= raw.len() {
            break;
        }

        let mut value = String::new();
        if let Some((ch, len)) = next_char(raw, idx) {
            if ch == '"' {
                idx += len;
                while let Some((next_ch, next_len)) = next_char(raw, idx) {
                    if next_ch == '\\' {
                        idx += next_len;
                        if let Some((escaped, escaped_len)) = next_char(raw, idx) {
                            value.push(escaped);
                            idx += escaped_len;
                        }
                        continue;
                    }
                    if next_ch == '"' {
                        idx += next_len;
                        break;
                    }
                    value.push(next_ch);
                    idx += next_len;
                }
                idx = skip_to_next_comma(raw, idx);
            } else {
                let value_start = idx;
                while let Some((next_ch, next_len)) = next_char(raw, idx) {
                    if next_ch == ',' {
                        break;
                    }
                    idx += next_len;
                }
                value = raw[value_start..idx].trim().to_string();
                idx = skip_to_next_comma(raw, idx);
            }
        }

        args.push((key, value));
    }
    args
}

fn heading_level_from_u32(level: u32) -> HeadingLevel {
    match level {
        1 => HeadingLevel::H1,
        2 => HeadingLevel::H2,
        3 => HeadingLevel::H3,
        4 => HeadingLevel::H4,
        5 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

fn escape_markdown_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' | '*' | '_' | '[' | ']' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn skip_to_next_comma(input: &str, mut idx: usize) -> usize {
    while let Some((ch, len)) = next_char(input, idx) {
        idx += len;
        if ch == ',' {
            break;
        }
    }
    idx
}

fn next_char(input: &str, idx: usize) -> Option<(char, usize)> {
    input[idx..].chars().next().map(|ch| (ch, ch.len_utf8()))
}

fn escape_html_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(value: &str) -> String {
    escape_html_text(value)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Some(true),
        "false" | "no" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn warning_block(message: &str) -> String {
    format!("> **Warning:** {message}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Header;
    use crate::model::{
        DocId, ImageFormatMode, MacrosConfig, SecurityConfig, Series, SeriesId, SeriesPart,
        SiteConfig, SiteContent, SiteMeta, SvgSecurityConfig, SvgSecurityMode, ThemeColorOverrides,
        ThemeNavOverrides, ThemeWideBackgroundOverrides, UrlStyle,
    };
    use anyhow::anyhow;
    use std::path::PathBuf;

    fn base_config() -> SiteConfig {
        SiteConfig {
            site: SiteMeta {
                id: "demo".to_string(),
                title: "Demo".to_string(),
                tagline: None,
                logo: None,
                copyright: None,
                base_url: "https://example.com/".to_string(),
                language: "en".to_string(),
                timezone: None,
                url_style: UrlStyle::Html,
                macros: MacrosConfig { enabled: true },
            },
            banner: None,
            menu: Vec::new(),
            nav: Vec::new(),
            theme: crate::model::ThemeConfig {
                variant: "stbl".to_string(),
                max_body_width: "72rem".to_string(),
                breakpoints: crate::model::ThemeBreakpoints {
                    desktop_min: "768px".to_string(),
                    wide_min: "1400px".to_string(),
                },
                colors: ThemeColorOverrides::default(),
                nav: ThemeNavOverrides::default(),
                header: crate::model::ThemeHeaderConfig {
                    layout: Default::default(),
                    menu_align: Default::default(),
                    title_size: "1.3rem".to_string(),
                    tagline_size: "1rem".to_string(),
                },
                wide_background: ThemeWideBackgroundOverrides::default(),
                color_scheme: None,
            },
            syntax: crate::model::SyntaxConfig {
                highlight: true,
                theme: "GitHub".to_string(),
                line_numbers: true,
            },
            assets: crate::model::AssetsConfig {
                cache_busting: false,
            },
            security: SecurityConfig {
                svg: SvgSecurityConfig {
                    mode: SvgSecurityMode::Warn,
                },
            },
            media: crate::model::MediaConfig {
                images: crate::model::ImageConfig {
                    widths: vec![
                        94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560,
                    ],
                    quality: 90,
                    format_mode: ImageFormatMode::Normal,
                },
                video: crate::model::VideoConfig {
                    heights: vec![360, 480, 720, 1080],
                    poster_time_sec: 1,
                },
            },
            footer: crate::model::FooterConfig { show_stbl: true },
            people: None,
            blog: None,
            system: None,
            publish: None,
            rss: None,
            seo: None,
            comments: None,
            chroma: None,
            plyr: None,
        }
    }

    fn make_page(id_seed: &str, source_path: &str, mut header: Header) -> Page {
        if header.title.is_none() {
            header.title = Some(id_seed.to_string());
        }
        Page {
            id: DocId(blake3::hash(id_seed.as_bytes())),
            source_path: source_path.to_string(),
            header,
            body_markdown: "Body".to_string(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: crate::url::logical_key_from_source_path(source_path),
            content_hash: blake3::hash(format!("content:{id_seed}").as_bytes()),
        }
    }

    fn project_with_pages(pages: Vec<Page>) -> Project {
        Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages,
                series: Vec::new(),
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
            image_alpha: std::collections::BTreeMap::new(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        }
    }

    fn project_with_content(pages: Vec<Page>, series: Vec<Series>) -> Project {
        Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages,
                series,
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
            image_alpha: std::collections::BTreeMap::new(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        }
    }

    fn project_with_series(series: Series) -> Project {
        Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
            image_alpha: std::collections::BTreeMap::new(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        }
    }

    fn make_series(dir: &str, title: &str, parts: &[(i32, &str)]) -> Series {
        let mut index_header = Header::default();
        index_header.is_published = true;
        index_header.title = Some(title.to_string());
        let index = make_page(
            "series-index",
            &format!("articles/{dir}/index.md"),
            index_header,
        );

        let mut part_vec = Vec::new();
        for (part_no, part_title) in parts {
            let mut header = Header::default();
            header.is_published = true;
            header.title = Some((*part_title).to_string());
            let seed = format!("{dir}-part-{part_no}");
            let page = make_page(&seed, &format!("articles/{dir}/part{part_no}.md"), header);
            part_vec.push(SeriesPart {
                part_no: *part_no,
                page,
            });
        }

        Series {
            id: SeriesId(blake3::hash(dir.as_bytes())),
            dir_path: dir.to_string(),
            index,
            parts: part_vec,
        }
    }

    struct FakeIncludeProvider {
        map: std::collections::BTreeMap<String, String>,
    }

    impl IncludeProvider for FakeIncludeProvider {
        fn include(&self, request: &IncludeRequest<'_>) -> Result<IncludeResponse> {
            let Some(content) = self.map.get(request.path) else {
                return Err(anyhow!("missing include"));
            };
            Ok(IncludeResponse {
                content: content.clone(),
                resolved_id: request.path.to_string(),
            })
        }
    }

    struct DummyRenderer;

    impl MarkdownRenderer for DummyRenderer {
        fn render(&self, md: &str) -> String {
            format!("<p>{}</p>", md.trim())
        }
    }

    struct DummyMediaRenderer;

    impl MediaRenderer for DummyMediaRenderer {
        fn render(&self, dest_url: &str, alt: &str) -> RenderedMedia {
            let mut maxw = None;
            let mut maxh = None;
            let mut src = dest_url;
            if let Some((path, attrs)) = dest_url.split_once(';') {
                src = path;
                for attr in attrs.split(';') {
                    if let Some(value) = attr.strip_prefix("maxw=") {
                        maxw = Some(value.to_string());
                    } else if let Some(value) = attr.strip_prefix("maxh=") {
                        maxh = Some(value.to_string());
                    }
                }
            }
            RenderedMedia {
                html: format!("<img src=\"{}\" alt=\"{}\">", src, escape_attr(alt.trim())),
                maxw,
                maxh,
            }
        }
    }

    #[test]
    fn expands_known_macro_and_leaves_unknown() {
        let mut header = Header::default();
        header.is_published = true;
        header.published = Some(10);
        let page = make_page("post-1", "articles/post-1.md", header.clone());

        let project = project_with_pages(vec![page]);
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let input = "@[blogitems](items=1)\n\n@[unknown]";
        let output = expand_macros(input, &ctx).expect("expand");
        assert!(output.contains("post-1"));
        assert!(output.contains("@[unknown]"));
    }

    #[test]
    fn blogitems_default_items_is_three() {
        let mut header = Header::default();
        header.is_published = true;
        header.published = Some(1);
        let p1 = make_page("post-1", "articles/post-1.md", header.clone());
        let p2 = make_page("post-2", "articles/post-2.md", header.clone());
        let p3 = make_page("post-3", "articles/post-3.md", header.clone());
        let p4 = make_page("post-4", "articles/post-4.md", header);

        let project = project_with_pages(vec![p1, p2, p3, p4]);
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[blogitems]", &ctx).expect("expand");
        assert!(output.contains("<ul class=\"blogitems\">"));
        assert!(output.contains("post-1"));
        assert!(output.contains("post-2"));
        assert!(output.contains("post-3"));
        assert!(!output.contains("post-4"));
    }

    #[test]
    fn blogitems_respects_items_argument() {
        let mut header = Header::default();
        header.is_published = true;
        header.published = Some(1);
        let p1 = make_page("post-1", "articles/post-1.md", header.clone());
        let p2 = make_page("post-2", "articles/post-2.md", header.clone());
        let p3 = make_page("post-3", "articles/post-3.md", header.clone());
        let p4 = make_page("post-4", "articles/post-4.md", header.clone());
        let p5 = make_page("post-5", "articles/post-5.md", header);

        let project = project_with_pages(vec![p1, p2, p3, p4, p5]);
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[blogitems](items=5)", &ctx).expect("expand");
        assert!(output.contains("post-5"));
    }

    #[test]
    fn tags_macro_renders_links_and_classes() {
        let mut header = Header::default();
        header.is_published = true;
        header.tags = vec!["rust".to_string(), "cli".to_string()];
        let page = make_page("post-1", "articles/post-1.md", header);

        let project = project_with_pages(vec![page.clone()]);
        let ctx = MacroContext {
            project: &project,
            page: Some(&page),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[tags]", &ctx).expect("expand");
        assert!(output.contains("<div class=\"tags tags-inline\">"));
        assert!(output.contains("href=\"tags/rust.html\""));
        assert!(output.contains("href=\"tags/cli.html\""));
        let rust_pos = output.find(">rust</a>").expect("rust tag");
        let cli_pos = output.find(">cli</a>").expect("cli tag");
        assert!(rust_pos < cli_pos);
    }

    #[test]
    fn tags_macro_empty_when_no_tags() {
        let mut header = Header::default();
        header.is_published = true;
        let page = make_page("post-1", "articles/post-1.md", header);

        let project = project_with_pages(vec![page.clone()]);
        let ctx = MacroContext {
            project: &project,
            page: Some(&page),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[tags]", &ctx).expect("expand");
        assert!(output.trim().is_empty());
    }

    #[test]
    fn series_macro_middle_shows_prev_and_next() {
        let series = make_series(
            "series",
            "Series Title",
            &[(1, "Part One"), (2, "Part Two"), (3, "Part Three")],
        );
        let current = series.parts[1].page.clone();
        let project = project_with_series(series);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[series]", &ctx).expect("expand");
        assert!(output.contains("Part 2 of 3"));
        assert!(output.contains("rel=\"prev\""));
        assert!(output.contains("rel=\"next\""));
    }

    #[test]
    fn series_macro_first_shows_only_next() {
        let series = make_series(
            "series",
            "Series Title",
            &[(1, "Part One"), (2, "Part Two"), (3, "Part Three")],
        );
        let current = series.parts[0].page.clone();
        let project = project_with_series(series);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[series]", &ctx).expect("expand");
        assert!(!output.contains("rel=\"prev\""));
        assert!(output.contains("rel=\"next\""));
    }

    #[test]
    fn series_macro_last_shows_only_prev() {
        let series = make_series(
            "series",
            "Series Title",
            &[(1, "Part One"), (2, "Part Two"), (3, "Part Three")],
        );
        let current = series.parts[2].page.clone();
        let project = project_with_series(series);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[series]", &ctx).expect("expand");
        assert!(output.contains("rel=\"prev\""));
        assert!(!output.contains("rel=\"next\""));
    }

    #[test]
    fn series_macro_list_shows_all_parts_in_order() {
        let series = make_series(
            "series",
            "Series Title",
            &[(1, "Part One"), (2, "Part Two"), (3, "Part Three")],
        );
        let current = series.parts[1].page.clone();
        let project = project_with_series(series);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[series](list=true)", &ctx).expect("expand");
        assert!(output.contains("<ol class=\"series-parts\">"));
        let first = output.find("Part One").expect("part one");
        let second = output.find("Part Two").expect("part two");
        let third = output.find("Part Three").expect("part three");
        assert!(first < second);
        assert!(second < third);
    }

    #[test]
    fn related_macro_prefers_shared_tags() {
        let mut current_header = Header::default();
        current_header.is_published = true;
        current_header.tags = vec!["rust".to_string()];
        let current = make_page("current", "articles/current.md", current_header);

        let mut shared_header = Header::default();
        shared_header.is_published = true;
        shared_header.tags = vec!["rust".to_string()];
        shared_header.published = Some(10);
        let shared = make_page("shared", "articles/shared.md", shared_header);

        let mut other_header = Header::default();
        other_header.is_published = true;
        other_header.published = Some(100);
        let other = make_page("other", "articles/other.md", other_header);

        let project = project_with_pages(vec![current.clone(), shared, other]);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[related](items=1, by=tags)", &ctx).expect("expand");
        assert!(output.contains("shared"));
        assert!(!output.contains("other"));
    }

    #[test]
    fn related_macro_prefers_series_over_tags() {
        let mut series = make_series(
            "series",
            "Series Title",
            &[(1, "Part One"), (2, "Part Two")],
        );
        series.parts[0].page.header.tags = vec!["rust".to_string()];
        let current = series.parts[0].page.clone();

        let mut tag_header = Header::default();
        tag_header.is_published = true;
        tag_header.tags = vec!["rust".to_string()];
        tag_header.published = Some(100);
        let tag_page = make_page("tagged", "articles/tagged.md", tag_header);

        let project = project_with_content(vec![tag_page], vec![series]);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[related](items=1)", &ctx).expect("expand");
        assert!(output.contains("Part Two"));
    }

    #[test]
    fn related_macro_order_is_stable() {
        let mut current_header = Header::default();
        current_header.is_published = true;
        current_header.tags = vec!["rust".to_string()];
        let current = make_page("current", "articles/current.md", current_header);

        let mut a_header = Header::default();
        a_header.is_published = true;
        a_header.tags = vec!["rust".to_string()];
        a_header.published = Some(10);
        let a = make_page("a", "articles/a.md", a_header);

        let mut b_header = Header::default();
        b_header.is_published = true;
        b_header.tags = vec!["rust".to_string()];
        b_header.published = Some(10);
        let b = make_page("b", "articles/b.md", b_header);

        let project = project_with_pages(vec![current.clone(), b, a]);
        let ctx = MacroContext {
            project: &project,
            page: Some(&current),
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };

        let output = expand_macros("@[related](items=2, by=tags)", &ctx).expect("expand");
        let first = output.find("a.html").expect("a link");
        let second = output.find("b.html").expect("b link");
        assert!(first < second);
    }

    #[test]
    fn malformed_macro_is_left_intact() {
        let project = project_with_pages(Vec::new());
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };
        let input = "Text @[blogitems(items=2]\nMore";
        let output = expand_macros(input, &ctx).expect("expand");
        assert_eq!(output, input);
    }

    #[test]
    fn render_pipeline_expands_blogitems_in_order() {
        let mut header = Header::default();
        header.is_published = true;

        let mut posts = Vec::new();
        for idx in 1..=6 {
            let mut post_header = header.clone();
            post_header.title = Some(format!("Post {idx}"));
            post_header.published = Some(idx as i64);
            posts.push(make_page(
                &format!("post-{idx}"),
                &format!("articles/post-{idx}.md"),
                post_header,
            ));
        }

        let mut macro_header = Header::default();
        macro_header.is_published = true;
        macro_header.title = Some("Macro Page".to_string());
        let macro_page = make_page("macro-page", "articles/macro.md", macro_header);

        let mut pages = posts.clone();
        pages.push(macro_page.clone());
        let project = project_with_pages(pages);

        let options = crate::render::RenderOptions {
            macro_project: Some(&project),
            macro_page: Some(&macro_page),
            macros_enabled: true,
            include_provider: None,
            rel_prefix: "",
            video_heights: &project.config.media.video.heights,
            image_widths: &project.config.media.images.widths,
            max_body_width: &project.config.theme.max_body_width,
            desktop_min: &project.config.theme.breakpoints.desktop_min,
            wide_min: &project.config.theme.breakpoints.wide_min,
            image_format_mode: project.config.media.images.format_mode,
            image_alpha: None,
            image_variants: None,
            video_variants: None,
            syntax_highlight: true,
            syntax_theme: "GitHub",
            syntax_line_numbers: true,
        };

        let html =
            crate::render::render_markdown_to_html_with_media("@[blogitems](items=3)", &options);

        let idx6 = html.find("Post 6").expect("post 6");
        let idx5 = html.find("Post 5").expect("post 5");
        let idx4 = html.find("Post 4").expect("post 4");
        assert!(idx6 < idx5 && idx5 < idx4);
        assert!(html.contains("post-6.html"));
        assert!(html.contains("post-5.html"));
        assert!(html.contains("post-4.html"));
        assert!(!html.contains("post-3.html"));
    }

    #[test]
    fn render_pipeline_expands_kbd_quote_figure() {
        let mut header = Header::default();
        header.is_published = true;
        header.title = Some("Macro Page".to_string());
        let page = make_page("macro-page", "articles/macro.md", header);

        let project = project_with_pages(vec![page.clone()]);

        let options = crate::render::RenderOptions {
            macro_project: Some(&project),
            macro_page: Some(&page),
            macros_enabled: true,
            include_provider: None,
            rel_prefix: "",
            video_heights: &project.config.media.video.heights,
            image_widths: &project.config.media.images.widths,
            max_body_width: &project.config.theme.max_body_width,
            desktop_min: &project.config.theme.breakpoints.desktop_min,
            wide_min: &project.config.theme.breakpoints.wide_min,
            image_format_mode: project.config.media.images.format_mode,
            image_alpha: None,
            image_variants: None,
            video_variants: None,
            syntax_highlight: true,
            syntax_theme: "GitHub",
            syntax_line_numbers: true,
        };

        let md = r#"@[kbd]Ctrl@[/kbd]

@[quote](author="Alan Kay", source="Talk")
Hello *world*.
@[/quote]

@[figure](src="images/diagram.png", caption="System overview", alt="Diagram", class="wide", maxw="900px")
"#;

        let html = crate::render::render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("<kbd class=\"kbd\">Ctrl</kbd>"));
        assert!(html.contains("<figure class=\"quote\">"));
        assert!(html.contains("<blockquote class=\"quote-body\">"));
        assert!(html.contains("<em>world</em>"));
        assert!(html.contains("<figure class=\"figure figure-wide\""));
        assert!(html.contains("<picture>"));
        assert!(html.contains("<figcaption>System overview</figcaption>"));
        assert!(!html.contains("@["));
    }

    #[test]
    fn parse_args_supports_quoted_values() {
        let args = parse_args("items=5, title=\"Heads up, friend\", note=\"a, b\", flag=on");
        let mut map = std::collections::BTreeMap::new();
        for (key, value) in args {
            map.insert(key, value);
        }
        assert_eq!(map.get("items").map(String::as_str), Some("5"));
        assert_eq!(
            map.get("title").map(String::as_str),
            Some("Heads up, friend")
        );
        assert_eq!(map.get("note").map(String::as_str), Some("a, b"));
        assert_eq!(map.get("flag").map(String::as_str), Some("on"));
    }

    #[test]
    fn parse_args_handles_escaped_quotes() {
        let args = parse_args("title=\"Heads up, \\\"friend\\\"\"");
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].0, "title");
        assert_eq!(args[0].1, "Heads up, \"friend\"");
    }

    #[test]
    fn block_macro_parses_body_and_closer() {
        let input = "@[blogitems](items=2)\nBody line\n@[/blogitems]";
        let invocation = parse_macro_invocation(input, 0).expect("parse");
        match invocation {
            MacroInvocation::Block {
                name,
                args,
                body,
                raw,
                end,
            } => {
                assert_eq!(name, "blogitems");
                assert_eq!(args, Some("items=2"));
                assert_eq!(body, "Body line\n");
                assert_eq!(raw, input);
                assert_eq!(end, input.len());
            }
            _ => panic!("expected block macro"),
        }
    }

    #[test]
    fn inline_macro_remains_inline_when_no_close_tag() {
        let input = "@[blogitems](items=2)\nBody line\nMore";
        let invocation = parse_macro_invocation(input, 0).expect("parse");
        match invocation {
            MacroInvocation::Inline {
                name, args, end, ..
            } => {
                assert_eq!(name, "blogitems");
                assert_eq!(args, Some("items=2"));
                assert!(end < input.len());
            }
            _ => panic!("expected inline macro"),
        }
    }

    #[test]
    fn callout_macro_renders_block_with_title() {
        let project = project_with_pages(Vec::new());
        let renderer = DummyRenderer;
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: Some(&renderer),
            render_media: None,
        };
        let input = "@[note](title=\"Heads up\")\nBody **markdown**.\n@[/note]";
        let output = expand_macros(input, &ctx).expect("expand");
        assert!(output.contains("<aside class=\"callout callout-note\">"));
        assert!(output.contains("<div class=\"callout-title\">Heads up</div>"));
        assert!(output.contains("<div class=\"callout-body\"><p>Body **markdown**.</p></div>"));
    }

    #[test]
    fn callout_inline_is_left_untouched() {
        let project = project_with_pages(Vec::new());
        let renderer = DummyRenderer;
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: Some(&renderer),
            render_media: None,
        };
        let input = "@[note](title=\"Heads up\")";
        let output = expand_macros(input, &ctx).expect("expand");
        assert_eq!(output, input);
    }

    #[test]
    fn kbd_macro_renders_with_text_arg_and_escapes() {
        let project = project_with_pages(Vec::new());
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };
        let output = expand_macros("@[kbd](text=\"Ctrl\")", &ctx).expect("expand");
        assert_eq!(output, "<kbd class=\"kbd\">Ctrl</kbd>");

        let output = expand_macros("@[kbd](text=\"<b>\")", &ctx).expect("expand");
        assert!(output.contains("&lt;b&gt;"));
        assert!(!output.contains("<b>"));
    }

    #[test]
    fn key_macro_supports_inline_body() {
        let project = project_with_pages(Vec::new());
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };
        let output = expand_macros("@[key]Enter@[/key]", &ctx).expect("expand");
        assert_eq!(output, "<kbd class=\"key\">Enter</kbd>");
    }

    #[test]
    fn quote_macro_renders_body_and_caption_variants() {
        let project = project_with_pages(Vec::new());
        let renderer = DummyRenderer;
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: Some(&renderer),
            render_media: None,
        };

        let output = expand_macros(
            "@[quote](author=\"Alan Kay\")\nBest **idea**.\n@[/quote]",
            &ctx,
        )
        .expect("expand");
        assert!(
            output.contains("<blockquote class=\"quote-body\"><p>Best **idea**.</p></blockquote>")
        );
        assert!(
            output.contains("<figcaption class=\"quote-caption\">&mdash; Alan Kay</figcaption>")
        );

        let output = expand_macros("@[quote](source=\"Talk\")\nBest **idea**.\n@[/quote]", &ctx)
            .expect("expand");
        assert!(output.contains("<figcaption class=\"quote-caption\">&mdash; Talk</figcaption>"));

        let output = expand_macros(
            "@[quote](author=\"Alan Kay\", source=\"Talk\", href=\"https://example.com\")\nBest **idea**.\n@[/quote]",
            &ctx,
        )
        .expect("expand");
        assert!(output.contains("&mdash; Alan Kay, <a href=\"https://example.com\">Talk</a>"));
    }

    #[test]
    fn figure_macro_renders_media_and_caption() {
        let project = project_with_pages(Vec::new());
        let media_renderer = DummyMediaRenderer;
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: Some(&media_renderer),
        };

        let output = expand_macros(
            "@[figure](src=\"images/foo.jpg\", caption=\"System overview\", alt=\"Diagram\", class=\"wide\")",
            &ctx,
        )
        .expect("expand");
        assert!(output.contains("<figure class=\"figure figure-wide\">"));
        assert!(output.contains("<img src=\"images/foo.jpg\" alt=\"Diagram\">"));
        assert!(output.contains("<figcaption>System overview</figcaption>"));
    }

    #[test]
    fn figure_macro_escapes_caption_and_applies_constraints() {
        let project = project_with_pages(Vec::new());
        let media_renderer = DummyMediaRenderer;
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: Some(&media_renderer),
        };

        let output = expand_macros(
            "@[figure](src=\"images/foo.jpg\", caption=\"<b>Hi</b>\", maxw=\"900px\")",
            &ctx,
        )
        .expect("expand");
        assert!(output.contains("&lt;b&gt;Hi&lt;/b&gt;"));
        assert!(output.contains("--media-maxw: 900px;"));
    }

    #[test]
    fn toc_macro_uses_default_range_and_slugs() {
        let project = project_with_pages(Vec::new());
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };
        let input = "# Title\n\n@[toc]\n\n## Section\n\n### Sub\n";
        let output = expand_macros(input, &ctx).expect("expand");
        assert!(output.contains("- [Section](#section)"));
        assert!(output.contains("- [Sub](#sub)"));
        assert!(!output.contains("- [Title](#title)"));
    }

    #[test]
    fn include_macro_requires_provider() {
        let project = project_with_pages(Vec::new());
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: None,
            render_markdown: None,
            render_media: None,
        };
        let output = expand_macros("@[include](path=\"partials/a.md\")", &ctx).expect("expand");
        assert!(output.contains("include macro is not enabled"));
    }

    #[test]
    fn include_macro_expands_content() {
        let project = project_with_pages(Vec::new());
        let mut map = std::collections::BTreeMap::new();
        map.insert("partials/a.md".to_string(), "Hello include".to_string());
        let provider = FakeIncludeProvider { map };
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: Some(&provider),
            render_markdown: None,
            render_media: None,
        };
        let output = expand_macros("@[include](path=\"partials/a.md\")", &ctx).expect("expand");
        assert!(output.contains("Hello include"));
    }

    #[test]
    fn include_macro_detects_recursion() {
        let project = project_with_pages(Vec::new());
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            "loop.md".to_string(),
            "@[include](path=\"loop.md\")".to_string(),
        );
        let provider = FakeIncludeProvider { map };
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: Some(&provider),
            render_markdown: None,
            render_media: None,
        };
        let output = expand_macros("@[include](path=\"loop.md\")", &ctx).expect("expand");
        assert!(output.contains("include macro recursion detected"));
    }

    #[test]
    fn include_macro_once_only_includes_first_time() {
        let project = project_with_pages(Vec::new());
        let mut map = std::collections::BTreeMap::new();
        map.insert("once.md".to_string(), "Once".to_string());
        let provider = FakeIncludeProvider { map };
        let ctx = MacroContext {
            project: &project,
            page: None,
            include_provider: Some(&provider),
            render_markdown: None,
            render_media: None,
        };
        let input =
            "@[include](path=\"once.md\", once=true)\n@[include](path=\"once.md\", once=true)";
        let output = expand_macros(input, &ctx).expect("expand");
        assert_eq!(output.matches("Once").count(), 1);
    }
}
