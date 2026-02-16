use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use image::{ColorType, GenericImageView};
use stbl_core::assets::AssetSourceId;
use stbl_core::media::{ImagePlanInput, MediaDimensions, MediaRef, VideoPlanInput};
use stbl_core::model::{Page, Project};

#[derive(Debug, Default, Clone)]
pub struct ImageSourceLookup {
    sources: BTreeMap<AssetSourceId, PathBuf>,
}

impl ImageSourceLookup {
    pub fn resolve(&self, source: &AssetSourceId) -> Option<&PathBuf> {
        self.sources.get(source)
    }
}

#[derive(Debug, Default, Clone)]
pub struct VideoSourceLookup {
    sources: BTreeMap<AssetSourceId, PathBuf>,
}

impl VideoSourceLookup {
    pub fn resolve(&self, source: &AssetSourceId) -> Option<&PathBuf> {
        self.sources.get(source)
    }
}

pub fn discover_images(project: &Project) -> Result<(ImagePlanInput, ImageSourceLookup)> {
    let mut paths = BTreeSet::new();
    for page in all_pages(project) {
        for media_ref in &page.media_refs {
            if let MediaRef::Image(image_ref) = media_ref {
                paths.insert(image_ref.path.raw.clone());
            }
        }
        if let Some(name) = page.banner_name.as_ref() {
            let resolved = resolve_banner_name(&project.root, name)
                .with_context(|| format!("failed to resolve banner '{}'", name))?;
            paths.insert(resolved);
        }
    }
    if let Some(path) = resolve_wide_background_image_path(project)? {
        paths.insert(path);
    }

    let mut sources = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut lookup = ImageSourceLookup::default();
    let mut alpha = BTreeMap::new();
    let mut dimensions = BTreeMap::new();
    for logical in paths {
        let abs_path = project.root.join(&logical);
        if !abs_path.exists() {
            bail!("image not found: {}", abs_path.display());
        }
        let (has_alpha, detected) = detect_alpha_and_dimensions(&abs_path)
            .with_context(|| format!("failed to inspect image {}", abs_path.display()))?;
        if let Some(detected) = detected {
            dimensions.insert(logical.clone(), detected);
        }
        let content_hash = hash_file(&abs_path)
            .with_context(|| format!("failed to hash image {}", abs_path.display()))?;
        let source = AssetSourceId(abs_path.to_string_lossy().to_string());
        sources.insert(logical.clone(), source.clone());
        hashes.insert(logical.clone(), content_hash);
        alpha.insert(logical, has_alpha);
        lookup.sources.insert(source, abs_path);
    }
    Ok((
        ImagePlanInput {
            sources,
            hashes,
            alpha,
            dimensions,
        },
        lookup,
    ))
}

fn resolve_wide_background_image_path(project: &Project) -> Result<Option<String>> {
    let Some(raw) = project.config.theme.wide_background.image.as_ref() else {
        return Ok(None);
    };
    let value = raw.trim();
    if value.is_empty()
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("data:")
        || value.starts_with("url(")
    {
        return Ok(None);
    }

    let logical = value
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if logical.is_empty() {
        return Ok(None);
    }
    let candidate = project.root.join(&logical);
    if candidate.exists() {
        return Ok(Some(logical));
    }

    bail!(
        "wide background image not found: {}",
        project.root.join(value).display()
    );
}

pub fn discover_videos(project: &Project) -> Result<(VideoPlanInput, VideoSourceLookup)> {
    let mut paths = BTreeSet::new();
    for page in all_pages(project) {
        for media_ref in &page.media_refs {
            if let MediaRef::Video(video_ref) = media_ref {
                paths.insert(video_ref.path.raw.clone());
            }
        }
    }

    let mut sources = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut lookup = VideoSourceLookup::default();
    let mut dimensions = BTreeMap::new();
    for logical in paths {
        let abs_path = project.root.join(&logical);
        if !abs_path.exists() {
            bail!("video not found: {}", abs_path.display());
        }
        let detected = probe_video_dimensions(&abs_path)
            .with_context(|| format!("failed to inspect video {}", abs_path.display()))?;
        let content_hash = hash_file(&abs_path)
            .with_context(|| format!("failed to hash video {}", abs_path.display()))?;
        let source = AssetSourceId(abs_path.to_string_lossy().to_string());
        sources.insert(logical.clone(), source.clone());
        hashes.insert(logical.clone(), content_hash);
        dimensions.insert(logical, detected);
        lookup.sources.insert(source, abs_path);
    }
    Ok((
        VideoPlanInput {
            sources,
            hashes,
            dimensions,
        },
        lookup,
    ))
}

pub fn resolve_banner_paths(project: &mut Project) -> Result<()> {
    for page in project.content.pages.iter_mut() {
        if let Some(name) = page.banner_name.clone() {
            page.banner_name = Some(resolve_banner_name(&project.root, &name)?);
        }
    }
    for series in project.content.series.iter_mut() {
        if let Some(name) = series.index.banner_name.clone() {
            series.index.banner_name = Some(resolve_banner_name(&project.root, &name)?);
        }
        for part in series.parts.iter_mut() {
            if let Some(name) = part.page.banner_name.clone() {
                part.page.banner_name = Some(resolve_banner_name(&project.root, &name)?);
            }
        }
    }
    Ok(())
}

fn all_pages(project: &Project) -> Vec<&Page> {
    let mut pages = Vec::new();
    pages.extend(project.content.pages.iter());
    pages.extend(project.content.series.iter().map(|series| &series.index));
    for series in &project.content.series {
        for part in &series.parts {
            pages.push(&part.page);
        }
    }
    pages
}

fn detect_alpha_and_dimensions(path: &Path) -> Result<(bool, Option<MediaDimensions>)> {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
    {
        return Ok((false, None));
    }
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let image = reader.decode()?;
    let (width, height) = image.dimensions();
    let has_alpha = matches!(
        image.color(),
        ColorType::La8
            | ColorType::La16
            | ColorType::Rgba8
            | ColorType::Rgba16
            | ColorType::Rgba32F
    );
    Ok((has_alpha, Some(MediaDimensions { width, height })))
}

fn probe_video_dimensions(path: &Path) -> Result<MediaDimensions> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(path)
        .output()
        .with_context(|| "failed to run ffmpeg for video probe")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(dimensions) = parse_video_dimensions(&stderr) {
        return Ok(dimensions);
    }
    bail!(
        "failed to parse video dimensions from ffmpeg output for {}",
        path.display()
    )
}

fn parse_video_dimensions(output: &str) -> Option<MediaDimensions> {
    for line in output.lines() {
        if !line.contains("Video:") {
            continue;
        }
        for token in line.split_whitespace() {
            if let Some((width, height)) = parse_dimension_token(token) {
                return Some(MediaDimensions { width, height });
            }
        }
    }
    None
}

fn parse_dimension_token(token: &str) -> Option<(u32, u32)> {
    let token = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != 'x');
    let (width, height) = token.split_once('x')?;
    let width = width.trim_matches(|ch: char| !ch.is_ascii_digit());
    let height = height.trim_matches(|ch: char| !ch.is_ascii_digit());
    if width.is_empty() || height.is_empty() {
        return None;
    }
    let width = width.parse::<u32>().ok()?;
    let height = height.parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

pub(crate) fn resolve_banner_name(root: &Path, name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("banner name must not be empty");
    }
    if name.starts_with("images/") {
        let candidate = root.join(name);
        if candidate.exists() {
            return Ok(name.to_string());
        }
        bail!("banner image not found: {}", candidate.display());
    }
    if name.contains('/') || name.contains('\\') {
        bail!("banner name must not be a path: {}", name);
    }
    let images_dir = root.join("images");
    if name.contains('.') {
        let candidate = images_dir.join(name);
        if candidate.exists() {
            return Ok(format!("images/{}", name));
        }
        bail!("banner image not found: {}", candidate.display());
    }
    let candidates = ["avif", "webp", "jpg", "png"];
    for ext in candidates {
        let file_name = format!("{name}.{ext}");
        let candidate = images_dir.join(&file_name);
        if candidate.exists() {
            return Ok(format!("images/{file_name}"));
        }
    }
    bail!("banner image not found: {}", name)
}

fn hash_file(path: &Path) -> Result<blake3::Hash> {
    let data = std::fs::read(path)?;
    Ok(blake3::hash(&data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stbl_core::config::load_site_config;
    use stbl_core::model::{Project, SiteContent};
    use tempfile::TempDir;

    #[test]
    fn resolve_wide_background_image_path_accepts_relative_images_path() {
        let temp = TempDir::new().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("images")).expect("mkdir images");
        std::fs::write(root.join("images/bg.jpg"), b"fake").expect("write image");
        std::fs::write(
            root.join("stbl.yaml"),
            "site:\n  id: demo\n  title: Demo\n  base_url: https://example.com\n  language: en\ntheme:\n  wide_background:\n    image: images/bg.jpg\n",
        )
        .expect("write config");

        let config = load_site_config(&root.join("stbl.yaml")).expect("load config");
        let project = Project {
            root: root.to_path_buf(),
            config,
            content: SiteContent::default(),
            image_alpha: BTreeMap::new(),
            image_variants: BTreeMap::new(),
            video_variants: BTreeMap::new(),
        };

        let path = resolve_wide_background_image_path(&project).expect("resolve path");
        assert_eq!(path.as_deref(), Some("images/bg.jpg"));
    }
}
