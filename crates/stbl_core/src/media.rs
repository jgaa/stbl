use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::BTreeMap;

use crate::assets::AssetSourceId;
use crate::model::{BuildTask, ContentId, InputFingerprint, OutputArtifact, TaskId, TaskKind};
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoRef {
    pub path: MediaPath,
    pub alt: String,
    pub prefer_p: u16,
    pub attrs: Vec<VideoAttr>,
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VideoPlanInput {
    pub sources: BTreeMap<String, AssetSourceId>,
    pub hashes: BTreeMap<String, Hash>,
}

pub fn parse_media_destination(dest: &str, alt: &str) -> Option<MediaRef> {
    let mut parts = dest.split(';');
    let path = parts.next()?.trim();
    if path.starts_with("images/") {
        let mut attrs = Vec::new();
        for attr in parts {
            let attr = attr.trim();
            if attr.eq_ignore_ascii_case("banner") {
                attrs.push(ImageAttr::Banner);
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
        }));
    }
    if path.starts_with("video/") {
        let mut attrs = Vec::new();
        let mut prefer_p: u16 = 720;
        for attr in parts {
            let attr = attr.trim();
            if let Some(value) = parse_video_prefer(attr) {
                prefer_p = value;
                attrs.push(VideoAttr::PreferP(value));
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
        }));
    }
    None
}

pub fn collect_media_refs(markdown: &str) -> Vec<MediaRef> {
    let parser = Parser::new(markdown);
    let mut refs = Vec::new();
    let mut stack: Vec<(String, String)> = Vec::new();
    for event in parser {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                stack.push((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Image) => {
                if let Some((dest, alt)) = stack.pop() {
                    if let Some(media_ref) = parse_media_destination(&dest, &alt) {
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
    refs
}

pub fn plan_image_tasks(
    images: &ImagePlanInput,
    widths: &[u32],
    quality: u8,
    render_config_hash: [u8; 32],
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
        let rel = path.strip_prefix("images/").unwrap_or(path.as_str());
        let original_out = format!("artifacts/images/{rel}");
        let copy_kind = TaskKind::CopyImageOriginal {
            source: source.clone(),
            out_rel: original_out.clone(),
        };
        let copy_id = TaskId::new("img_copy", &[path.as_str()]);
        let copy_fingerprint =
            fingerprint_image_task(&copy_id, "CopyImageOriginal", render_config_hash, input_hash);
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
        for width in &widths {
            if *width == 0 {
                continue;
            }
            let out_rel = format!("artifacts/images/_scale_{width}/{rel}");
            let width_label = format!("w={width}");
            let quality_label = format!("q={quality}");
            let kind = TaskKind::ResizeImage {
                source: source.clone(),
                width: *width,
                quality,
                out_rel: out_rel.clone(),
            };
            let id = TaskId::new(
                "img_scale",
                &[path.as_str(), &width_label, &quality_label],
            );
            let fingerprint =
                fingerprint_image_task(&id, "ResizeImage", render_config_hash, input_hash);
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
    tasks
}

pub fn plan_video_tasks(
    videos: &VideoPlanInput,
    heights: &[u32],
    poster_time_sec: u32,
    render_config_hash: [u8; 32],
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
        let original_out = format!("artifacts/video/{rel}");
        let copy_kind = TaskKind::CopyVideoOriginal {
            source: source.clone(),
            out_rel: original_out.clone(),
        };
        let copy_id = TaskId::new("vid_copy", &[path.as_str()]);
        let copy_fingerprint =
            fingerprint_video_task(&copy_id, "CopyVideoOriginal", render_config_hash, input_hash);
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
            render_config_hash,
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

        for height in &heights {
            if *height == 0 {
                continue;
            }
            let out_rel = format!("artifacts/video/_scale_{height}/{rel}");
            let height_label = format!("h={height}");
            let kind = TaskKind::TranscodeVideoMp4 {
                source: source.clone(),
                height: *height,
                out_rel: out_rel.clone(),
            };
            let id = TaskId::new("vid_scale", &[path.as_str(), &height_label]);
            let fingerprint =
                fingerprint_video_task(&id, "TranscodeVideoMp4", render_config_hash, input_hash);
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
    render_config_hash: [u8; 32],
    source_hash: Hash,
) -> InputFingerprint {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.task.v1");
    add_str(&mut hasher, &task_id.0);
    add_str(&mut hasher, kind_label);
    hasher.update(&render_config_hash);
    hasher.update(source_hash.as_bytes());
    InputFingerprint(*hasher.finalize().as_bytes())
}

fn fingerprint_video_task(
    task_id: &TaskId,
    kind_label: &str,
    render_config_hash: [u8; 32],
    source_hash: Hash,
) -> InputFingerprint {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.task.v1");
    add_str(&mut hasher, &task_id.0);
    add_str(&mut hasher, kind_label);
    hasher.update(&render_config_hash);
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
        format!("artifacts/video/_poster_/{stem}.jpg")
    } else {
        format!("artifacts/video/_poster_/{parent}/{stem}.jpg")
    }
}
