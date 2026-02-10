use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::collections::BTreeMap;

use crate::assets::AssetSourceId;
use crate::model::{
    BuildTask, ContentId, ImageFormatMode, ImageOutputFormat, InputFingerprint, OutputArtifact,
    TaskId, TaskKind,
};
use blake3::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaPath {
    /// Raw path as written in markdown, e.g. "images/foo.jpg" or "video/intro.mp4"
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageAttr {
    Banner,
    WidthPercent(u8),
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoAttr {
    PreferP(u16),
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub path: MediaPath,
    pub alt: String,
    pub attrs: Vec<ImageAttr>,
    pub maxw: Option<String>,
    pub maxh: Option<String>,
    pub has_args: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoRef {
    pub path: MediaPath,
    pub alt: String,
    pub prefer_p: u16,
    pub attrs: Vec<VideoAttr>,
    pub maxw: Option<String>,
    pub maxh: Option<String>,
    pub has_args: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaRef {
    Image(ImageRef),
    Video(VideoRef),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImagePlanInput {
    pub sources: BTreeMap<String, AssetSourceId>,
    pub hashes: BTreeMap<String, Hash>,
    pub alpha: BTreeMap<String, bool>,
    pub dimensions: BTreeMap<String, MediaDimensions>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageVariantFallback {
    pub path: String,
    pub format: ImageOutputFormat,
    pub mime: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageVariantSet {
    pub avif: Option<String>,
    pub webp: Option<String>,
    pub fallback: ImageVariantFallback,
}

pub type ImageVariantIndex = BTreeMap<String, BTreeMap<u32, ImageVariantSet>>;
pub type VideoVariantIndex = BTreeMap<String, Vec<u32>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MediaDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VideoPlanInput {
    pub sources: BTreeMap<String, AssetSourceId>,
    pub hashes: BTreeMap<String, Hash>,
    pub dimensions: BTreeMap<String, MediaDimensions>,
}

pub fn parse_media_destination(dest: &str, alt: &str) -> Option<MediaRef> {
    parse_media_destination_internal(dest, alt, None)
}

pub fn collect_media_refs_with_errors(markdown: &str) -> (Vec<MediaRef>, Vec<String>) {
    let parser = Parser::new_ext(markdown, Options::empty());
    let mut refs = Vec::new();
    let mut errors = Vec::new();
    let mut stack: Vec<(String, String)> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Image {
                dest_url,
                title: _,
                id: _,
                link_type: _,
            }) => {
                stack.push((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Image) => {
                if let Some((dest, alt)) = stack.pop() {
                    if let Some(media_ref) =
                        parse_media_destination_internal(&dest, &alt, Some(&mut errors))
                    {
                        refs.push(media_ref);
                    }
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, alt)) = stack.last_mut() {
                    alt.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, alt)) = stack.last_mut() {
                    if !alt.ends_with(' ') {
                        alt.push(' ');
                    }
                }
            }
            _ => {}
        }
    }
    (refs, errors)
}

pub fn collect_media_refs(markdown: &str) -> Vec<MediaRef> {
    collect_media_refs_with_errors(markdown).0
}

fn parse_media_destination_internal(
    dest: &str,
    alt: &str,
    mut errors: Option<&mut Vec<String>>,
) -> Option<MediaRef> {
    let has_args = dest.contains(';');
    let mut parts = dest.split(';');
    let path = parts.next()?.trim();
    if path.starts_with("images/") {
        let mut attrs = Vec::new();
        let mut maxw = None;
        let mut maxh = None;
        for attr in parts {
            let attr = attr.trim();
            if attr.eq_ignore_ascii_case("banner") {
                attrs.push(ImageAttr::Banner);
                continue;
            }
            if let Some(value) = attr.strip_prefix("maxw=") {
                match parse_media_length(value) {
                    Some(value) => maxw = Some(value),
                    None => {
                        if let Some(errors) = errors.as_deref_mut() {
                            errors.push(format!(
                                "invalid maxw value '{value}' in '{dest}'; expected <number><unit> with unit in px, rem, em, %, vw, vh"
                            ));
                        }
                        return None;
                    }
                }
                continue;
            }
            if let Some(value) = attr.strip_prefix("maxh=") {
                match parse_media_length(value) {
                    Some(value) => maxh = Some(value),
                    None => {
                        if let Some(errors) = errors.as_deref_mut() {
                            errors.push(format!(
                                "invalid maxh value '{value}' in '{dest}'; expected <number><unit> with unit in px, rem, em, %, vw, vh"
                            ));
                        }
                        return None;
                    }
                }
                continue;
            }
            if let Some(percent) = attr.strip_suffix('%') {
                if let Ok(value) = percent.parse::<u8>() {
                    if (1..=100).contains(&value) {
                        attrs.push(ImageAttr::WidthPercent(value));
                        continue;
                    }
                }
            }
            if !attr.is_empty() {
                attrs.push(ImageAttr::Unknown(attr.to_string()));
            }
        }
        return Some(MediaRef::Image(ImageRef {
            path: MediaPath {
                raw: path.to_string(),
            },
            alt: alt.to_string(),
            attrs,
            maxw,
            maxh,
            has_args,
        }));
    }
    if path.starts_with("video/") {
        let mut attrs = Vec::new();
        let mut prefer_p: u16 = 720;
        let mut maxw = None;
        let mut maxh = None;
        for attr in parts {
            let attr = attr.trim();
            if let Some(value) = parse_video_prefer(attr) {
                prefer_p = value;
                attrs.push(VideoAttr::PreferP(value));
                continue;
            }
            if let Some(value) = attr.strip_prefix("maxw=") {
                match parse_media_length(value) {
                    Some(value) => maxw = Some(value),
                    None => {
                        if let Some(errors) = errors.as_deref_mut() {
                            errors.push(format!(
                                "invalid maxw value '{value}' in '{dest}'; expected <number><unit> with unit in px, rem, em, %, vw, vh"
                            ));
                        }
                        return None;
                    }
                }
                continue;
            }
            if let Some(value) = attr.strip_prefix("maxh=") {
                match parse_media_length(value) {
                    Some(value) => maxh = Some(value),
                    None => {
                        if let Some(errors) = errors.as_deref_mut() {
                            errors.push(format!(
                                "invalid maxh value '{value}' in '{dest}'; expected <number><unit> with unit in px, rem, em, %, vw, vh"
                            ));
                        }
                        return None;
                    }
                }
                continue;
            }
            if !attr.is_empty() {
                attrs.push(VideoAttr::Unknown(attr.to_string()));
            }
        }
        return Some(MediaRef::Video(VideoRef {
            path: MediaPath {
                raw: path.to_string(),
            },
            alt: alt.to_string(),
            prefer_p,
            attrs,
            maxw,
            maxh,
            has_args,
        }));
    }
    None
}

fn parse_media_length(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let units = ["px", "rem", "em", "%", "vw", "vh"];
    let unit = units.iter().find(|unit| value.ends_with(*unit))?;
    let number = value[..value.len() - unit.len()].trim();
    if number.is_empty() {
        return None;
    }
    let mut chars = number.chars();
    let first = chars.next()?;
    if !first.is_ascii_digit() {
        return None;
    }
    let mut seen_dot = false;
    let mut seen_digit_after_dot = false;
    for ch in chars {
        if ch == '.' {
            if seen_dot {
                return None;
            }
            seen_dot = true;
            continue;
        }
        if !ch.is_ascii_digit() {
            return None;
        }
        if seen_dot {
            seen_digit_after_dot = true;
        }
    }
    if seen_dot && !seen_digit_after_dot {
        return None;
    }
    Some(value.to_string())
}

// (collect_media_refs defined earlier with error support)

pub fn plan_image_tasks(
    images: &ImagePlanInput,
    widths: &[u32],
    quality: u8,
    format_mode: ImageFormatMode,
) -> Vec<BuildTask> {
    let mut tasks = Vec::new();
    let mut paths = images.sources.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    let mut widths = widths.to_vec();
    widths.sort_unstable();
    for path in paths {
        let source = images.sources.get(&path).expect("source exists");
        let input_hash = images
            .hashes
            .get(&path)
            .copied()
            .expect("image hash exists");
        let has_alpha = images.alpha.get(&path).copied().unwrap_or(false);
        let rel = path.strip_prefix("images/").unwrap_or(path.as_str());
        let original_out = format!("images/{rel}");
        let copy_kind = TaskKind::CopyImageOriginal {
            source: source.clone(),
            out_rel: original_out.clone(),
        };
        let copy_id = TaskId::new("img_copy", &[path.as_str()]);
        let copy_fingerprint =
            fingerprint_image_task(&copy_id, "CopyImageOriginal", input_hash);
        tasks.push(BuildTask {
            id: copy_id,
            kind: copy_kind,
            inputs_fingerprint: copy_fingerprint,
            inputs: vec![ContentId::Image(path.clone())],
            outputs: vec![OutputArtifact {
                path: std::path::PathBuf::from(original_out),
            }],
        });

        let is_svg = path.to_lowercase().ends_with(".svg");
        if is_svg {
            continue;
        }
        let formats = image_output_formats(format_mode, has_alpha);
        let max_width = images
            .dimensions
            .get(&path)
            .map(|dimensions| dimensions.width);
        for width in &widths {
            if *width == 0 {
                continue;
            }
            if let Some(max_width) = max_width {
                if *width > max_width {
                    continue;
                }
            }
            for format in &formats {
                let ext = format_extension(*format);
                let out_rel =
                    format!("images/_scale_{width}/{}", replace_extension(rel, ext));
                let width_label = format!("w={width}");
                let quality_label = format!("q={quality}");
                let format_label = format!("f={}", ext);
                let kind = TaskKind::ResizeImage {
                    source: source.clone(),
                    width: *width,
                    quality,
                    format: *format,
                    out_rel: out_rel.clone(),
                };
                let id = TaskId::new(
                    "img_scale",
                    &[path.as_str(), &width_label, &quality_label, &format_label],
                );
                let fingerprint =
                    fingerprint_image_task(&id, "ResizeImage", input_hash);
                tasks.push(BuildTask {
                    id,
                    kind,
                    inputs_fingerprint: fingerprint,
                    inputs: vec![ContentId::Image(path.clone())],
                    outputs: vec![OutputArtifact {
                        path: std::path::PathBuf::from(out_rel),
                    }],
                });
            }
        }
    }
    tasks
}

pub fn build_image_variant_index(
    images: &ImagePlanInput,
    widths: &[u32],
    format_mode: ImageFormatMode,
) -> ImageVariantIndex {
    let mut index = BTreeMap::new();
    let mut widths = widths.to_vec();
    widths.sort_unstable();
    widths.dedup();
    let mut paths = images.sources.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let is_svg = path.to_ascii_lowercase().ends_with(".svg");
        if is_svg {
            continue;
        }
        let has_alpha = images.alpha.get(&path).copied().unwrap_or(false);
        let rel = path.strip_prefix("images/").unwrap_or(path.as_str());
        let formats = image_output_formats(format_mode, has_alpha);
        let fallback = fallback_format(has_alpha);
        let mut per_width = BTreeMap::new();
        let max_width = images
            .dimensions
            .get(&path)
            .map(|dimensions| dimensions.width);
        for width in widths.iter().copied().filter(|width| *width > 0) {
            if let Some(max_width) = max_width {
                if width > max_width {
                    continue;
                }
            }
            let fallback_path = scaled_image_path(rel, width, fallback);
            let mut set = ImageVariantSet {
                avif: None,
                webp: None,
                fallback: ImageVariantFallback {
                    path: fallback_path,
                    format: fallback,
                    mime: format_mime(fallback),
                },
            };
            for format in &formats {
                let path = scaled_image_path(rel, width, *format);
                match format {
                    ImageOutputFormat::Avif => set.avif = Some(path),
                    ImageOutputFormat::Webp => set.webp = Some(path),
                    ImageOutputFormat::Jpeg | ImageOutputFormat::Png => {}
                }
            }
            per_width.insert(width, set);
        }
        if !per_width.is_empty() {
            index.insert(path, per_width);
        }
    }
    index
}

pub fn build_video_variant_index(
    videos: &VideoPlanInput,
    heights: &[u32],
) -> VideoVariantIndex {
    let mut index = BTreeMap::new();
    let mut heights = heights.to_vec();
    heights.sort_unstable();
    heights.dedup();
    let mut paths = videos.sources.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let max_height = videos
            .dimensions
            .get(&path)
            .map(|dimensions| dimensions.height);
        let filtered = heights
            .iter()
            .copied()
            .filter(|height| *height > 0)
            .filter(|height| match max_height {
                Some(max_height) => *height <= max_height,
                None => true,
            })
            .collect::<Vec<_>>();
        index.insert(path, filtered);
    }
    index
}

pub fn image_output_formats(
    mode: ImageFormatMode,
    has_alpha: bool,
) -> Vec<ImageOutputFormat> {
    let mut formats = Vec::new();
    if mode == ImageFormatMode::Normal && cfg!(feature = "avif") {
        formats.push(ImageOutputFormat::Avif);
    }
    formats.push(ImageOutputFormat::Webp);
    formats.push(if has_alpha {
        ImageOutputFormat::Png
    } else {
        ImageOutputFormat::Jpeg
    });
    formats
}

pub fn fallback_format(has_alpha: bool) -> ImageOutputFormat {
    if has_alpha {
        ImageOutputFormat::Png
    } else {
        ImageOutputFormat::Jpeg
    }
}

pub fn format_extension(format: ImageOutputFormat) -> &'static str {
    match format {
        ImageOutputFormat::Avif => "avif",
        ImageOutputFormat::Webp => "webp",
        ImageOutputFormat::Jpeg => "jpg",
        ImageOutputFormat::Png => "png",
    }
}

pub fn format_mime(format: ImageOutputFormat) -> &'static str {
    match format {
        ImageOutputFormat::Avif => "image/avif",
        ImageOutputFormat::Webp => "image/webp",
        ImageOutputFormat::Jpeg => "image/jpeg",
        ImageOutputFormat::Png => "image/png",
    }
}

fn replace_extension(path: &str, ext: &str) -> String {
    match path.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.{ext}"),
        None => format!("{path}.{ext}"),
    }
}

fn scaled_image_path(rel: &str, width: u32, format: ImageOutputFormat) -> String {
    let ext = format_extension(format);
    format!("images/_scale_{width}/{}", replace_extension(rel, ext))
}

pub fn plan_video_tasks(
    videos: &VideoPlanInput,
    heights: &[u32],
    poster_time_sec: u32,
) -> Vec<BuildTask> {
    let mut tasks = Vec::new();
    let mut paths = videos.sources.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    let mut heights = heights.to_vec();
    heights.sort_unstable();
    for path in paths {
        let source = videos.sources.get(&path).expect("source exists");
        let input_hash = videos
            .hashes
            .get(&path)
            .copied()
            .expect("video hash exists");
        let rel = path.strip_prefix("video/").unwrap_or(path.as_str());
        let original_out = format!("video/{rel}");
        let copy_kind = TaskKind::CopyVideoOriginal {
            source: source.clone(),
            out_rel: original_out.clone(),
        };
        let copy_id = TaskId::new("vid_copy", &[path.as_str()]);
        let copy_fingerprint =
            fingerprint_video_task(&copy_id, "CopyVideoOriginal", input_hash);
        tasks.push(BuildTask {
            id: copy_id,
            kind: copy_kind,
            inputs_fingerprint: copy_fingerprint,
            inputs: vec![ContentId::Video(path.clone())],
            outputs: vec![OutputArtifact {
                path: std::path::PathBuf::from(original_out),
            }],
        });

        let poster_rel = poster_output_rel(rel);
        let poster_kind = TaskKind::ExtractVideoPoster {
            source: source.clone(),
            poster_time_sec,
            out_rel: poster_rel.clone(),
        };
        let poster_label = format!("t={poster_time_sec}");
        let poster_id = TaskId::new("vid_poster", &[path.as_str(), &poster_label]);
        let poster_fingerprint = fingerprint_video_task(
            &poster_id,
            "ExtractVideoPoster",
            input_hash,
        );
        tasks.push(BuildTask {
            id: poster_id,
            kind: poster_kind,
            inputs_fingerprint: poster_fingerprint,
            inputs: vec![ContentId::Video(path.clone())],
            outputs: vec![OutputArtifact {
                path: std::path::PathBuf::from(poster_rel),
            }],
        });

        let max_height = videos
            .dimensions
            .get(&path)
            .map(|dimensions| dimensions.height);
        for height in &heights {
            if *height == 0 {
                continue;
            }
            if let Some(max_height) = max_height {
                if *height > max_height {
                    continue;
                }
            }
            let out_rel = format!("video/_scale_{height}/{rel}");
            let height_label = format!("h={height}");
            let kind = TaskKind::TranscodeVideoMp4 {
                source: source.clone(),
                height: *height,
                out_rel: out_rel.clone(),
            };
            let id = TaskId::new("vid_scale", &[path.as_str(), &height_label]);
            let fingerprint =
                fingerprint_video_task(&id, "TranscodeVideoMp4", input_hash);
            tasks.push(BuildTask {
                id,
                kind,
                inputs_fingerprint: fingerprint,
                inputs: vec![ContentId::Video(path.clone())],
                outputs: vec![OutputArtifact {
                    path: std::path::PathBuf::from(out_rel),
                }],
            });
        }
    }
    tasks
}

fn fingerprint_image_task(
    task_id: &TaskId,
    kind_label: &str,
    source_hash: Hash,
) -> InputFingerprint {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.task.v1");
    add_str(&mut hasher, &task_id.0);
    add_str(&mut hasher, kind_label);
    hasher.update(source_hash.as_bytes());
    InputFingerprint(*hasher.finalize().as_bytes())
}

fn fingerprint_video_task(
    task_id: &TaskId,
    kind_label: &str,
    source_hash: Hash,
) -> InputFingerprint {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.task.v1");
    add_str(&mut hasher, &task_id.0);
    add_str(&mut hasher, kind_label);
    hasher.update(source_hash.as_bytes());
    InputFingerprint(*hasher.finalize().as_bytes())
}

fn add_str(hasher: &mut Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn parse_video_prefer(attr: &str) -> Option<u16> {
    let attr = attr.trim();
    if let Some(value) = attr.strip_prefix('p') {
        if let Ok(value) = value.parse::<u16>() {
            if matches!(value, 360 | 480 | 720 | 1080 | 1440 | 2160) {
                return Some(value);
            }
        }
    }
    None
}

fn poster_output_rel(rel: &str) -> String {
    let rel_path = std::path::Path::new(rel);
    let stem = rel_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("poster");
    let parent = rel_path
        .parent()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .replace('\\', "/");
    if parent.is_empty() || parent == "." {
        format!("video/_poster_/{stem}.jpg")
    } else {
        format!("video/_poster_/{parent}/{stem}.jpg")
    }
}

#[cfg(test)]
mod tests {
    use super::{MediaRef, parse_media_destination};

    #[test]
    fn image_ref_tracks_presence_of_args() {
        let plain = parse_media_destination("images/foo.jpg", "Alt").expect("plain");
        match plain {
            MediaRef::Image(image) => assert!(!image.has_args),
            _ => panic!("expected image"),
        }

        let optioned =
            parse_media_destination("images/foo.jpg;maxw=200px", "Alt").expect("optioned");
        match optioned {
            MediaRef::Image(image) => assert!(image.has_args),
            _ => panic!("expected image"),
        }
    }

    #[test]
    fn video_ref_tracks_presence_of_args() {
        let plain = parse_media_destination("video/foo.mp4", "Alt").expect("plain");
        match plain {
            MediaRef::Video(video) => assert!(!video.has_args),
            _ => panic!("expected video"),
        }

        let optioned = parse_media_destination("video/foo.mp4;p720", "Alt").expect("optioned");
        match optioned {
            MediaRef::Video(video) => assert!(video.has_args),
            _ => panic!("expected video"),
        }
    }
}
