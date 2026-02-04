use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

use crate::media::{MediaRef, parse_media_destination};

pub struct RenderOptions<'a> {
    pub rel_prefix: &'a str,
    pub video_heights: &'a [u32],
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
                } else {
                    events.push(Event::End(TagEnd::Image));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(video) = video_pending.as_mut() {
                    video.alt.push_str(&text);
                } else {
                    events.push(Event::Text(text));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(video) = video_pending.as_mut() {
                    if !video.alt.ends_with(' ') {
                        video.alt.push(' ');
                    }
                } else {
                    events.push(event);
                }
            }
            _ => {
                if video_pending.is_none() {
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

fn render_video_html(dest_url: &str, alt: &str, options: &RenderOptions<'_>) -> String {
    let (video_path, prefer_p) = match parse_media_destination(dest_url, alt) {
        Some(MediaRef::Video(video)) => (video.path.raw, video.prefer_p),
        _ => (dest_url.to_string(), 720),
    };
    let requested_prefer = prefer_p;
    let heights = ordered_heights(options.video_heights, prefer_p);
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
