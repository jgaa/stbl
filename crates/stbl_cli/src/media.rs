use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use stbl_core::assets::AssetSourceId;
use stbl_core::media::{ImagePlanInput, MediaRef, VideoPlanInput};
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

    let mut sources = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut lookup = ImageSourceLookup::default();
    for logical in paths {
        let abs_path = project.root.join(&logical);
        if !abs_path.exists() {
            bail!("image not found: {}", abs_path.display());
        }
        let content_hash = hash_file(&abs_path)
            .with_context(|| format!("failed to hash image {}", abs_path.display()))?;
        let source = AssetSourceId(abs_path.to_string_lossy().to_string());
        sources.insert(logical.clone(), source.clone());
        hashes.insert(logical, content_hash);
        lookup.sources.insert(source, abs_path);
    }
    Ok((ImagePlanInput { sources, hashes }, lookup))
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
    for logical in paths {
        let abs_path = project.root.join(&logical);
        if !abs_path.exists() {
            bail!("video not found: {}", abs_path.display());
        }
        let content_hash = hash_file(&abs_path)
            .with_context(|| format!("failed to hash video {}", abs_path.display()))?;
        let source = AssetSourceId(abs_path.to_string_lossy().to_string());
        sources.insert(logical.clone(), source.clone());
        hashes.insert(logical, content_hash);
        lookup.sources.insert(source, abs_path);
    }
    Ok((VideoPlanInput { sources, hashes }, lookup))
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

fn resolve_banner_name(root: &Path, name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("banner name must not be empty");
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
