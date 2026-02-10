use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use pulldown_cmark_toc::{GitHubSlugifier, Slugify};
use std::collections::BTreeMap;

use crate::media::{
    ImageVariantIndex, ImageVariantSet, MediaRef, VideoVariantIndex, fallback_format,
    format_extension, format_mime, image_output_formats, parse_media_destination,
};
use crate::macros::{
    IncludeProvider, MacroContext, MarkdownRenderer, MediaRenderer, RenderedMedia, expand_macros,
};
use crate::model::{ImageFormatMode, ImageOutputFormat, Page, Project};
use crate::syntax_highlight::highlight_code_html_classed;

pub struct RenderOptions<'a> {
    pub macro_project: Option<&'a Project>,
    pub macro_page: Option<&'a Page>,
    pub macros_enabled: bool,
    pub include_provider: Option<&'a dyn IncludeProvider>,
    pub rel_prefix: &'a str,
    pub video_heights: &'a [u32],
    pub image_widths: &'a [u32],
    pub max_body_width: &'a str,
    pub desktop_min: &'a str,
    pub wide_min: &'a str,
    pub image_format_mode: ImageFormatMode,
    pub image_alpha: Option<&'a BTreeMap<String, bool>>,
    pub image_variants: Option<&'a ImageVariantIndex>,
    pub video_variants: Option<&'a VideoVariantIndex>,
    pub syntax_highlight: bool,
    pub syntax_theme: &'a str,
    pub syntax_line_numbers: bool,
}

struct CalloutRenderer<'a> {
    options: &'a RenderOptions<'a>,
}

impl MarkdownRenderer for CalloutRenderer<'_> {
    fn render(&self, md: &str) -> String {
        let options = RenderOptions {
            macro_project: self.options.macro_project,
            macro_page: self.options.macro_page,
            macros_enabled: false,
            include_provider: self.options.include_provider,
            rel_prefix: self.options.rel_prefix,
            video_heights: self.options.video_heights,
            image_widths: self.options.image_widths,
            max_body_width: self.options.max_body_width,
            desktop_min: self.options.desktop_min,
            wide_min: self.options.wide_min,
            image_format_mode: self.options.image_format_mode,
            image_alpha: self.options.image_alpha,
            image_variants: self.options.image_variants,
            video_variants: self.options.video_variants,
            syntax_highlight: self.options.syntax_highlight,
            syntax_theme: self.options.syntax_theme,
            syntax_line_numbers: self.options.syntax_line_numbers,
        };
        render_markdown_to_html_with_media(md, &options)
    }
}

struct MacroMediaRenderer<'a> {
    options: &'a RenderOptions<'a>,
}

impl MediaRenderer for MacroMediaRenderer<'_> {
    fn render(&self, dest_url: &str, alt: &str) -> RenderedMedia {
        render_media_element_html(dest_url, alt, self.options)
    }
}

pub fn render_markdown_to_html(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(md, options);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    html_out
}

pub fn render_markdown_to_html_with_media(md: &str, options: &RenderOptions<'_>) -> String {
    let expanded = match options.macro_project {
        Some(project) => {
            let renderer = CalloutRenderer { options };
            let media_renderer = MacroMediaRenderer { options };
            let ctx = MacroContext {
                project,
                page: options.macro_page,
                include_provider: options.include_provider,
                render_markdown: Some(&renderer),
                render_media: Some(&media_renderer),
            };
            if options.macros_enabled {
                expand_macros(md, &ctx).unwrap_or_else(|_| md.to_string())
            } else {
                md.to_string()
            }
        }
        None => md.to_string(),
    };
    let mut cmark_options = Options::empty();
    cmark_options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(&expanded, cmark_options);
    let mut events = Vec::new();
    let mut video_pending: Option<VideoPending> = None;
    let mut image_pending: Option<ImagePending> = None;
    let mut code_pending: Option<CodeBlockPending> = None;
    let mut heading_pending: Option<HeadingPending> = None;
    let mut slugger = GitHubSlugifier::default();

    for event in parser {
        if let Some(heading) = heading_pending.as_mut() {
            match event {
                Event::End(TagEnd::Heading(_)) => {
                    let level_num = heading_level_value(heading.level);
                    let slug = slugger.slugify(heading.text.trim()).into_owned();
                    let mut inner = String::new();
                    html::push_html(&mut inner, heading.events.drain(..));
                    let html = format!(
                        "<h{level_num} id=\"{}\">{inner}</h{level_num}>",
                        escape_attr(&slug)
                    );
                    events.push(Event::Html(html.into()));
                    heading_pending = None;
                }
                Event::Text(text) => {
                    heading.text.push_str(&text);
                    heading.events.push(Event::Text(text));
                }
                Event::Code(text) => {
                    heading.text.push_str(&text);
                    heading.events.push(Event::Code(text));
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !heading.text.ends_with(' ') {
                        heading.text.push(' ');
                    }
                    heading.events.push(event);
                }
                _ => {
                    heading.events.push(event);
                }
            }
            continue;
        }

        if let Some(code) = code_pending.as_mut() {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let html = render_code_block_html(code, options);
                    events.push(Event::Html(html.into()));
                    code_pending = None;
                }
                Event::Text(text) | Event::Code(text) => {
                    code.code.push_str(&text);
                }
                Event::SoftBreak | Event::HardBreak => {
                    code.code.push('\n');
                }
                _ => {}
            }
            continue;
        }

        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_pending = Some(HeadingPending {
                    level,
                    text: String::new(),
                    events: Vec::new(),
                });
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => extract_language(info.as_ref()),
                    CodeBlockKind::Indented => String::new(),
                };
                code_pending = Some(CodeBlockPending {
                    language,
                    code: String::new(),
                });
            }
            Event::Start(Tag::Image {
                dest_url,
                title,
                id,
                link_type,
            }) => {
                if dest_url.starts_with("video/") {
                    video_pending = Some(VideoPending {
                        dest_url: dest_url.to_string(),
                        alt: String::new(),
                    });
                } else if dest_url.starts_with("images/") {
                    image_pending = Some(ImagePending {
                        dest_url: dest_url.to_string(),
                        alt: String::new(),
                    });
                } else {
                    events.push(Event::Start(Tag::Image {
                        dest_url,
                        title,
                        id,
                        link_type,
                    }));
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some(video) = video_pending.take() {
                    let html = render_video_html(&video.dest_url, &video.alt, options);
                    events.push(Event::Html(html.into()));
                } else if let Some(image) = image_pending.take() {
                    let html = render_image_html(&image.dest_url, &image.alt, options);
                    events.push(Event::Html(html.into()));
                } else {
                    events.push(Event::End(TagEnd::Image));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(video) = video_pending.as_mut() {
                    video.alt.push_str(&text);
                } else if let Some(image) = image_pending.as_mut() {
                    image.alt.push_str(&text);
                } else {
                    events.push(Event::Text(text));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(video) = video_pending.as_mut() {
                    if !video.alt.ends_with(' ') {
                        video.alt.push(' ');
                    }
                } else if let Some(image) = image_pending.as_mut() {
                    if !image.alt.ends_with(' ') {
                        image.alt.push(' ');
                    }
                } else {
                    events.push(event);
                }
            }
            _ => {
                if video_pending.is_none() && image_pending.is_none() {
                    events.push(event);
                }
            }
        }
    }

    let mut html_out = String::new();
    html::push_html(&mut html_out, events.into_iter());
    html_out
}

struct VideoPending {
    dest_url: String,
    alt: String,
}

struct ImagePending {
    dest_url: String,
    alt: String,
}

struct CodeBlockPending {
    language: String,
    code: String,
}

struct HeadingPending<'a> {
    level: HeadingLevel,
    text: String,
    events: Vec<Event<'a>>,
}

pub fn render_media_element_html(
    dest_url: &str,
    alt: &str,
    options: &RenderOptions<'_>,
) -> RenderedMedia {
    match parse_media_destination(dest_url, alt) {
        Some(MediaRef::Image(image)) => RenderedMedia {
            html: render_image_picture_html(&image, options),
            maxw: image.maxw,
            maxh: image.maxh,
        },
        Some(MediaRef::Video(video)) => RenderedMedia {
            html: render_video_element_html(&video, options),
            maxw: video.maxw,
            maxh: video.maxh,
        },
        None => RenderedMedia {
            html: render_plain_image(dest_url, alt),
            maxw: None,
            maxh: None,
        },
    }
}

pub fn render_image_html(dest_url: &str, alt: &str, options: &RenderOptions<'_>) -> String {
    let Some(MediaRef::Image(image)) = parse_media_destination(dest_url, alt) else {
        return render_plain_image(dest_url, alt);
    };
    let mut html = render_image_picture_html(&image, options);
    let is_banner = image
        .attrs
        .iter()
        .any(|attr| matches!(attr, crate::media::ImageAttr::Banner));
    let apply_constraints = !is_banner && (image.maxw.is_some() || image.maxh.is_some());
    if apply_constraints {
        let mut wrapped = String::new();
        wrapped.push_str("<figure class=\"media-frame\"");
        let mut style = String::new();
        if let Some(maxw) = image.maxw.as_ref() {
            style.push_str("--media-maxw: ");
            style.push_str(maxw);
            style.push_str("; ");
        }
        if let Some(maxh) = image.maxh.as_ref() {
            style.push_str("--media-maxh: ");
            style.push_str(maxh);
            style.push_str("; ");
        }
        if !style.is_empty() {
            wrapped.push_str(" style=\"");
            wrapped.push_str(style.trim());
            wrapped.push('"');
        }
        wrapped.push('>');
        wrapped.push_str(&html);
        wrapped.push_str("</figure>");
        html = wrapped;
    }
    html
}

fn render_image_picture_html(image: &crate::media::ImageRef, options: &RenderOptions<'_>) -> String {
    if !image.has_args {
        return render_plain_media_image(image, options);
    }
    let path = image.path.raw.as_str();
    let rel = path.strip_prefix("images/").unwrap_or(path);
    let is_svg = rel.to_lowercase().ends_with(".svg");
    let has_alpha = image_has_alpha(path, options);
    let fallback = fallback_format(has_alpha);
    let variants = options
        .image_variants
        .and_then(|index| index.get(path));
    let use_original =
        !is_svg && options.image_variants.is_some() && variants.map_or(true, |v| v.is_empty());
    let (src, srcset) = if is_svg || use_original {
        (
            format!("{}images/{rel}", options.rel_prefix),
            None,
        )
    } else if let Some(variants) = variants {
        let src = fallback_src_from_variants(variants, options.rel_prefix);
        let src = if src.is_empty() {
            src_for_format(
                rel,
                options.image_widths,
                format_extension(fallback),
                options.rel_prefix,
            )
        } else {
            src
        };
        (
            src,
            srcset_for_variant_format(variants, fallback, options.rel_prefix),
        )
    } else {
        (
            src_for_format(
                rel,
                options.image_widths,
                format_extension(fallback),
                options.rel_prefix,
            ),
            srcset_for_format(
                rel,
                options.image_widths,
                format_extension(fallback),
                options.rel_prefix,
            ),
        )
    };
    let sizes = image_sizes(&image.attrs, options);
    let (class_attr, style_attr) = image_class_style(&image.attrs);
    let alt = escape_attr(image.alt.trim());

    let mut html = String::new();
    html.push_str("<picture");
    if let Some(class_attr) = class_attr.as_ref() {
        html.push_str(" class=\"");
        html.push_str(class_attr);
        html.push('"');
    }
    html.push('>');
    if !is_svg && !use_original {
        let formats = image_output_formats(options.image_format_mode, has_alpha);
        for format in formats
            .iter()
            .copied()
            .filter(|format| matches!(format, ImageOutputFormat::Avif | ImageOutputFormat::Webp))
        {
            let srcset = if let Some(variants) = variants {
                srcset_for_variant_format(variants, format, options.rel_prefix)
            } else {
                srcset_for_format(
                    rel,
                    options.image_widths,
                    format_extension(format),
                    options.rel_prefix,
                )
            };
            if let Some(srcset) = srcset {
                html.push_str("<source type=\"");
                html.push_str(format_mime(format));
                html.push_str("\" srcset=\"");
                html.push_str(&srcset);
                html.push_str("\" sizes=\"");
                html.push_str(&sizes);
                html.push_str("\">");
            }
        }
    }
    html.push_str("<img src=\"");
    html.push_str(&src);
    html.push('"');
    html.push_str(" alt=\"");
    html.push_str(&alt);
    html.push('"');
    if let Some(srcset) = srcset {
        html.push_str(" srcset=\"");
        html.push_str(&srcset);
        html.push('"');
        html.push_str(" sizes=\"");
        html.push_str(&sizes);
        html.push('"');
    }
    if let Some(style_attr) = style_attr.as_ref() {
        html.push_str(" style=\"");
        html.push_str(style_attr);
        html.push('"');
    }
    html.push_str(" loading=\"lazy\" decoding=\"async\">");
    html.push_str("</picture>");
    html
}

fn render_plain_media_image(image: &crate::media::ImageRef, options: &RenderOptions<'_>) -> String {
    let mut html = String::new();
    html.push_str("<img src=\"");
    html.push_str(options.rel_prefix);
    html.push_str(&image.path.raw);
    html.push_str("\" alt=\"");
    html.push_str(&escape_attr(image.alt.trim()));
    html.push_str("\" loading=\"lazy\" decoding=\"async\">");
    html
}

fn srcset_for_variant_format(
    variants: &BTreeMap<u32, ImageVariantSet>,
    format: ImageOutputFormat,
    rel_prefix: &str,
) -> Option<String> {
    let mut parts = Vec::new();
    for (width, set) in variants {
        let path = match format {
            ImageOutputFormat::Avif => set.avif.as_ref(),
            ImageOutputFormat::Webp => set.webp.as_ref(),
            ImageOutputFormat::Jpeg | ImageOutputFormat::Png => {
                if set.fallback.format == format {
                    Some(&set.fallback.path)
                } else {
                    None
                }
            }
        };
        if let Some(path) = path {
            parts.push(format!("{rel_prefix}{path} {width}w"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn fallback_src_from_variants(
    variants: &BTreeMap<u32, ImageVariantSet>,
    rel_prefix: &str,
) -> String {
    if let Some((_, set)) = variants.iter().next_back() {
        return format!("{rel_prefix}{}", set.fallback.path);
    }
    String::new()
}

fn srcset_for_format(
    rel: &str,
    widths: &[u32],
    ext: &str,
    rel_prefix: &str,
) -> Option<String> {
    let mut widths = widths.to_vec();
    widths.sort_unstable();
    widths.dedup();
    let parts = widths
        .into_iter()
        .filter(|width| *width > 0)
        .map(|width| {
            format!(
                "{}images/_scale_{width}/{} {width}w",
                rel_prefix,
                replace_extension(rel, ext),
            )
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn src_for_format(rel: &str, widths: &[u32], ext: &str, rel_prefix: &str) -> String {
    let max_width = widths.iter().copied().filter(|width| *width > 0).max();
    match max_width {
        Some(width) => format!(
            "{}images/_scale_{width}/{}",
            rel_prefix,
            replace_extension(rel, ext),
        ),
        None => format!("{}images/{}", rel_prefix, rel),
    }
}

fn image_has_alpha(path: &str, options: &RenderOptions<'_>) -> bool {
    if let Some(map) = options.image_alpha {
        if let Some(value) = map.get(path) {
            return *value;
        }
    }
    match path.rsplit_once('.') {
        Some((_, ext)) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "png" | "apng" | "gif"
        ),
        None => false,
    }
}

fn replace_extension(path: &str, ext: &str) -> String {
    match path.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.{ext}"),
        None => format!("{path}.{ext}"),
    }
}

fn render_plain_image(dest_url: &str, alt: &str) -> String {
    let mut html = String::new();
    html.push_str("<img src=\"");
    html.push_str(dest_url);
    html.push_str("\" alt=\"");
    html.push_str(&escape_attr(alt.trim()));
    html.push_str("\">");
    html
}

fn render_video_html(dest_url: &str, alt: &str, options: &RenderOptions<'_>) -> String {
    let (video, maxw, maxh, prefer_p) = match parse_media_destination(dest_url, alt) {
        Some(MediaRef::Video(video)) => {
            let prefer = video.prefer_p;
            let maxw = video.maxw.clone();
            let maxh = video.maxh.clone();
            (Some(video), maxw, maxh, prefer)
        }
        _ => (None, None, None, 720),
    };

    let (html, requested_prefer) = match video.as_ref() {
        Some(video) => (render_video_element_html(video, options), video.prefer_p),
        None => {
            let mut fallback = String::new();
            fallback.push_str("<video class=\"video__el\" controls preload=\"metadata\">");
            fallback.push_str("<source src=\"");
            fallback.push_str(options.rel_prefix);
            fallback.push_str(dest_url);
            fallback.push_str("\" type=\"video/mp4\">");
            fallback.push_str("Your browser doesn't support HTML5 video.");
            fallback.push_str("</video>");
            (fallback, prefer_p)
        }
    };

    let mut wrapped = String::new();
    wrapped.push_str("<figure");
    if maxw.is_some() || maxh.is_some() {
        wrapped.push_str(" class=\"media-frame video\"");
    } else {
        wrapped.push_str(" class=\"video\"");
    }
    if maxw.is_some() || maxh.is_some() {
        let mut style = String::new();
        if let Some(maxw) = maxw.as_ref() {
            style.push_str("--media-maxw: ");
            style.push_str(maxw);
            style.push_str("; ");
        }
        if let Some(maxh) = maxh.as_ref() {
            style.push_str("--media-maxh: ");
            style.push_str(maxh);
            style.push_str("; ");
        }
        if !style.is_empty() {
            wrapped.push_str(" style=\"");
            wrapped.push_str(style.trim());
            wrapped.push('"');
        }
    }
    wrapped.push_str(" data-stbl-video data-prefer=\"p");
    wrapped.push_str(&requested_prefer.to_string());
    wrapped.push_str("\">");
    wrapped.push_str(&html);
    wrapped.push_str("</figure>");
    wrapped
}

fn render_video_element_html(video: &crate::media::VideoRef, options: &RenderOptions<'_>) -> String {
    let video_path = video.path.raw.as_str();
    let heights = match options
        .video_variants
        .and_then(|variants| variants.get(video_path))
    {
        Some(available) => ordered_heights(available, video.prefer_p),
        None => ordered_heights(options.video_heights, video.prefer_p),
    };
    let VideoPaths {
        poster_rel,
        sources,
        download_rel,
    } = video_paths(video_path, &heights);

    let mut html = String::new();
    html.push_str("<video");
    if video.has_args {
        html.push_str(" class=\"video__el\"");
    }
    html.push_str(" controls preload=\"metadata\" poster=\"");
    html.push_str(options.rel_prefix);
    html.push_str(&poster_rel);
    html.push('"');
    let alt_trimmed = video.alt.trim();
    if !alt_trimmed.is_empty() {
        html.push_str(" aria-label=\"");
        html.push_str(&escape_attr(alt_trimmed));
        html.push('"');
    }
    html.push_str(">");

    let sources = if sources.is_empty() {
        vec![download_rel.clone()]
    } else {
        sources
    };
    for src in sources {
        html.push_str("<source src=\"");
        html.push_str(options.rel_prefix);
        html.push_str(&src);
        html.push_str("\" type=\"video/mp4\">");
    }

    html.push_str("Your browser doesn't support HTML5 video - <a href=\"");
    html.push_str(options.rel_prefix);
    html.push_str(&download_rel);
    html.push_str("\">download it</a>.");
    html.push_str("</video>");
    html
}

struct VideoPaths {
    poster_rel: String,
    sources: Vec<String>,
    download_rel: String,
}

fn video_paths(dest_url: &str, heights: &[u32]) -> VideoPaths {
    let rel = dest_url.strip_prefix("video/").unwrap_or(dest_url);
    let rel_path = std::path::Path::new(rel);
    let stem = rel_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("video");
    let parent = rel_path
        .parent()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .replace('\\', "/");
    let base = if parent.is_empty() || parent == "." {
        stem.to_string()
    } else {
        format!("{parent}/{stem}")
    };
    let download_rel = format!("video/{base}.mp4");
    let poster_rel = format!("video/_poster_/{base}.jpg");
    let sources = heights
        .iter()
        .map(|height| format!("video/_scale_{height}/{base}.mp4"))
        .collect();
    VideoPaths {
        poster_rel,
        sources,
        download_rel,
    }
}

fn ordered_heights(heights: &[u32], prefer_p: u16) -> Vec<u32> {
    let mut sorted = heights.to_vec();
    sorted.sort_unstable();
    if sorted.is_empty() {
        return sorted;
    }
    let prefer_u32 = prefer_p as u32;
    let preferred = if sorted.contains(&prefer_u32) {
        Some(prefer_u32)
    } else {
        let mut below = sorted.iter().cloned().filter(|value| *value <= prefer_u32);
        if let Some(value) = below.next_back() {
            Some(value)
        } else {
            sorted.first().cloned()
        }
    };
    if let Some(preferred) = preferred {
        let mut ordered = Vec::with_capacity(sorted.len());
        ordered.push(preferred);
        for value in sorted {
            if value != preferred {
                ordered.push(value);
            }
        }
        ordered
    } else {
        sorted
    }
}

fn image_sizes(attrs: &[crate::media::ImageAttr], options: &RenderOptions<'_>) -> String {
    for attr in attrs {
        if let crate::media::ImageAttr::WidthPercent(percent) = attr {
            return format!("{percent}vw");
        }
    }
    if attrs
        .iter()
        .any(|attr| matches!(attr, crate::media::ImageAttr::Banner))
    {
        return "100vw".to_string();
    }
    format!(
        "(min-width: {}) {}, (min-width: {}) {}, 100vw",
        options.wide_min,
        options.max_body_width,
        options.desktop_min,
        options.max_body_width
    )
}

fn image_class_style(attrs: &[crate::media::ImageAttr]) -> (Option<String>, Option<String>) {
    let mut class_attr = None;
    let mut style_attr = None;
    let mut width_percent = None;
    for attr in attrs {
        match attr {
            crate::media::ImageAttr::Banner => {
                class_attr = Some("banner-image".to_string());
            }
            crate::media::ImageAttr::WidthPercent(percent) => {
                width_percent = Some(*percent);
            }
            _ => {}
        }
    }
    if let Some(percent) = width_percent {
        style_attr = Some(format!("width: {percent}%; height: auto;"));
    }
    (class_attr, style_attr)
}

fn escape_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_html_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn extract_language(info: &str) -> String {
    info.split_whitespace().next().unwrap_or("").to_string()
}

fn heading_level_value(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn sanitize_language(language: &str) -> String {
    language
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+'))
        .collect()
}

fn render_code_block_html(pending: &CodeBlockPending, options: &RenderOptions<'_>) -> String {
    let language_raw = pending.language.trim();
    let language = sanitize_language(language_raw);
    let mut classes = Vec::new();
    if !language.is_empty() {
        classes.push(format!("language-{language}"));
    }
    if options.syntax_line_numbers {
        classes.push("has-line-numbers".to_string());
    }

    if options.syntax_highlight {
        if let Some(html) =
            highlight_code_html_classed(&pending.code, language_raw, options.syntax_theme)
        {
            classes.push("syntect".to_string());
            let class_attr = if classes.is_empty() {
                String::new()
            } else {
                format!(" class=\"{}\"", classes.join(" "))
            };
            let html = if options.syntax_line_numbers {
                wrap_code_lines(&html)
            } else {
                html
            };
            let pre = format!("<pre class=\"code-block\"><code{class_attr}>{html}</code></pre>");
            return wrap_code_block_container(&pre);
        }
    }

    let class_attr = if classes.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", classes.join(" "))
    };
    let escaped = escape_html_text(&pending.code);
    let escaped = if options.syntax_line_numbers {
        wrap_code_lines(&escaped)
    } else {
        escaped
    };
    let pre = format!("<pre class=\"code-block\"><code{class_attr}>{escaped}</code></pre>");
    wrap_code_block_container(&pre)
}

fn wrap_code_lines(text: &str) -> String {
    let mut out = String::new();
    let mut lines: Vec<&str> = text.split('\n').collect();
    if matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }
    let mut iter = lines.into_iter().peekable();
    while let Some(line) = iter.next() {
        out.push_str("<span class=\"code-line\">");
        out.push_str(line);
        out.push_str("</span>");
    }
    out
}

fn wrap_code_block_container(pre_html: &str) -> String {
    format!(
        "<div class=\"codeblock\" data-codeblock>\
<button type=\"button\" class=\"codeblock__copy\" data-copy-button aria-label=\"Copy code\">Copy</button>\
{pre_html}</div>"
    )
}

#[cfg(test)]
mod tests {
    use super::{RenderOptions, render_markdown_to_html, render_markdown_to_html_with_media};
    use crate::model::ImageFormatMode;

    #[test]
    fn renders_basic_markdown() {
        let html = render_markdown_to_html("# Title\n\nHello **world**.\n");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<p>Hello <strong>world</strong>.</p>"));
    }

    fn base_render_options<'a>(video_heights: &'a [u32], image_widths: &'a [u32]) -> RenderOptions<'a> {
        RenderOptions {
            macro_project: None,
            macro_page: None,
            macros_enabled: false,
            include_provider: None,
            rel_prefix: "",
            video_heights,
            image_widths,
            max_body_width: "72rem",
            desktop_min: "768px",
            wide_min: "1400px",
            image_format_mode: ImageFormatMode::Normal,
            image_alpha: None,
            image_variants: None,
            video_variants: None,
            syntax_highlight: true,
            syntax_theme: "GitHub",
            syntax_line_numbers: true,
        }
    }

    #[test]
    fn headings_receive_ids() {
        let md = "# Title\n\n## Section\n";
        let heights = [360];
        let widths = [360];
        let options = base_render_options(&heights, &widths);
        let html = render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("<h1 id=\"title\">Title</h1>"));
        assert!(html.contains("<h2 id=\"section\">Section</h2>"));
    }

    #[test]
    fn fenced_code_highlighted_when_enabled() {
        let md = "```rust\nfn main() {}\n```\n";
        let heights = [360];
        let widths = [360];
        let options = base_render_options(&heights, &widths);
        let html = render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("<div class=\"codeblock\" data-codeblock>"));
        assert!(html.contains(
            "<button type=\"button\" class=\"codeblock__copy\" data-copy-button aria-label=\"Copy code\">Copy</button>"
        ));
        assert!(html.contains(
            "<pre class=\"code-block\"><code class=\"language-rust has-line-numbers syntect\">"
        ));
        assert!(html.contains("<span class=\""));
    }

    #[test]
    fn fenced_code_falls_back_when_disabled() {
        let md = "```rust\nfn main() { println!(\"<tag>&\"); }\n```\n";
        let heights = [360];
        let widths = [360];
        let mut options = base_render_options(&heights, &widths);
        options.syntax_highlight = false;
        let html = render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("<div class=\"codeblock\" data-codeblock>"));
        assert!(html.contains(
            "<button type=\"button\" class=\"codeblock__copy\" data-copy-button aria-label=\"Copy code\">Copy</button>"
        ));
        assert!(html.contains(
            "<pre class=\"code-block\"><code class=\"language-rust has-line-numbers\">"
        ));
        assert!(html.contains("&lt;tag&gt;"));
        assert!(html.contains("&quot;"));
        assert!(html.contains("&amp;"));
        assert!(!html.contains("syntect"));
    }

    #[test]
    fn fenced_code_renders_line_numbers_when_enabled() {
        let md = "```rust\nfn main() {}\n```\n";
        let heights = [360];
        let widths = [360];
        let options = base_render_options(&heights, &widths);
        let html = render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("has-line-numbers"));
        assert!(html.contains("<span class=\"code-line\">"));
    }

    #[test]
    fn plain_images_render_without_srcset() {
        let md = "![Alt](images/foo.jpg)\n";
        let heights = [360];
        let widths = [360];
        let options = base_render_options(&heights, &widths);
        let html = render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("<img src=\"images/foo.jpg\""));
        assert!(html.contains("loading=\"lazy\""));
        assert!(html.contains("decoding=\"async\""));
        assert!(!html.contains("<picture"));
        assert!(!html.contains("srcset="));
    }

    #[test]
    fn optioned_images_keep_responsive_markup() {
        let md = "![Alt](images/foo.jpg;maxw=200px)\n";
        let heights = [360];
        let widths = [360];
        let options = base_render_options(&heights, &widths);
        let html = render_markdown_to_html_with_media(md, &options);
        assert!(html.contains("<picture"));
        assert!(html.contains("srcset="));
    }
}
