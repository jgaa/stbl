use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use std::collections::BTreeMap;

use crate::media::{
    ImageVariantIndex, ImageVariantSet, MediaRef, VideoVariantIndex, fallback_format,
    format_extension, format_mime, image_output_formats, parse_media_destination,
};
use crate::model::{ImageFormatMode, ImageOutputFormat};

pub struct RenderOptions<'a> {
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
}

pub fn render_markdown_to_html(md: &str) -> String {
    let options = Options::empty();
    let parser = Parser::new_ext(md, options);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    html_out
}

pub fn render_markdown_to_html_with_media(md: &str, options: &RenderOptions<'_>) -> String {
    let parser = Parser::new_ext(md, Options::empty());
    let mut events = Vec::new();
    let mut video_pending: Option<VideoPending> = None;
    let mut image_pending: Option<ImagePending> = None;

    for event in parser {
        match event {
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

pub fn render_image_html(dest_url: &str, alt: &str, options: &RenderOptions<'_>) -> String {
    let Some(MediaRef::Image(image)) = parse_media_destination(dest_url, alt) else {
        return render_plain_image(dest_url, alt);
    };
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
            format!("{}artifacts/images/{rel}", options.rel_prefix),
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
    let alt = escape_attr(alt.trim());

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
                "{}artifacts/images/_scale_{width}/{} {width}w",
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
            "{}artifacts/images/_scale_{width}/{}",
            rel_prefix,
            replace_extension(rel, ext),
        ),
        None => format!("{}artifacts/images/{}", rel_prefix, rel),
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
    let (video_path, prefer_p) = match parse_media_destination(dest_url, alt) {
        Some(MediaRef::Video(video)) => (video.path.raw, video.prefer_p),
        _ => (dest_url.to_string(), 720),
    };
    let requested_prefer = prefer_p;
    let heights = match options
        .video_variants
        .and_then(|variants| variants.get(&video_path))
    {
        Some(available) => ordered_heights(available, prefer_p),
        None => ordered_heights(options.video_heights, prefer_p),
    };
    let VideoPaths {
        poster_rel,
        sources,
        download_rel,
    } = video_paths(&video_path, &heights);

    let mut html = String::new();
    html.push_str("<figure class=\"video\" data-stbl-video data-prefer=\"p");
    html.push_str(&requested_prefer.to_string());
    html.push_str("\">");

    html.push_str("<video class=\"video__el\" controls preload=\"metadata\" poster=\"");
    html.push_str(options.rel_prefix);
    html.push_str(&poster_rel);
    html.push('"');
    let alt_trimmed = alt.trim();
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
    html.push_str("</video></figure>");
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
    let download_rel = format!("artifacts/video/{base}.mp4");
    let poster_rel = format!("artifacts/video/_poster_/{base}.jpg");
    let sources = heights
        .iter()
        .map(|height| format!("artifacts/video/_scale_{height}/{base}.mp4"))
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

#[cfg(test)]
mod tests {
    use super::render_markdown_to_html;

    #[test]
    fn renders_basic_markdown() {
        let html = render_markdown_to_html("# Title\n\nHello **world**.\n");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<p>Hello <strong>world</strong>.</p>"));
    }
}
