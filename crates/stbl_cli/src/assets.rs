use anyhow::{Context, Result, anyhow, bail};
use blake3;
use stbl_core::assets::{AssetIndex, AssetRelPath, AssetSourceId, ResolvedAsset};
use stbl_core::model::{BuildTask, TaskKind};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use stbl_embedded_assets as embedded;

#[derive(Debug, Default, Clone)]
pub struct AssetSourceLookup {
    sources: BTreeMap<AssetSourceId, AssetSource>,
}

impl AssetSourceLookup {
    pub fn resolve(&self, source: &AssetSourceId) -> Option<&AssetSource> {
        self.sources.get(source)
    }
}

#[derive(Debug, Clone)]
pub enum AssetSource {
    File(PathBuf),
    Embedded(Vec<u8>),
}

pub fn embedded_default_template() -> Result<&'static embedded::Template> {
    embedded::template("default")
        .ok_or_else(|| anyhow!("embedded assets template 'default' not found"))
}

pub fn iter_embedded_assets(
    template: &embedded::Template,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut assets = Vec::new();
    for entry in template.assets {
        let bytes = embedded::decompress_to_vec(&entry.hash)
            .ok_or_else(|| anyhow!("failed to decompress embedded asset {}", entry.path))?;
        assets.push((entry.path.to_string(), bytes));
    }
    Ok(assets)
}

pub fn discover_assets(site_root: &Path) -> Result<(AssetIndex, AssetSourceLookup)> {
    let mut resolved: BTreeMap<AssetRelPath, (AssetSourceId, AssetSource, String)> =
        BTreeMap::new();
    let template = embedded_default_template()?;
    for (rel_path, bytes) in iter_embedded_assets(template)? {
        let rel = normalize_rel_path(Path::new(&rel_path))?;
        if rel.0 == "css/vars.css" {
            continue;
        }
        let content_hash = blake3::hash(&bytes).to_hex().to_string();
        let source_id = AssetSourceId(format!("embedded:{content_hash}"));
        resolved.insert(
            rel,
            (source_id, AssetSource::Embedded(bytes), content_hash),
        );
    }

    if site_root.exists() {
        collect_site_assets(site_root, &mut resolved)?;
    }

    let mut sources = BTreeMap::new();
    let assets = resolved
        .into_iter()
        .map(|(rel, (source, asset_source, content_hash))| {
            sources.insert(source.clone(), asset_source);
            ResolvedAsset {
                rel,
                source,
                content_hash,
            }
        })
        .collect::<Vec<_>>();

    Ok((AssetIndex { assets }, AssetSourceLookup { sources }))
}

#[allow(dead_code)]
pub fn execute_copy_tasks(
    tasks: &[BuildTask],
    out_dir: &Path,
    lookup: &AssetSourceLookup,
) -> Result<()> {
    for task in tasks {
        if let TaskKind::CopyAsset {
            source, out_rel, ..
        } = &task.kind
        {
            copy_asset_to_out(out_dir, out_rel, source, lookup)?;
        }
    }
    Ok(())
}

pub fn copy_asset_to_out(
    out_dir: &Path,
    out_rel: &str,
    source: &AssetSourceId,
    lookup: &AssetSourceLookup,
) -> Result<()> {
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    match lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown asset source {}", source.0))?
    {
        AssetSource::File(src_path) => {
            std::fs::copy(src_path, &out_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    src_path.display(),
                    out_path.display()
                )
            })?;
        }
        AssetSource::Embedded(bytes) => {
            std::fs::write(&out_path, bytes)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
        }
    }
    Ok(())
}

fn collect_site_assets(
    root: &Path,
    out: &mut BTreeMap<AssetRelPath, (AssetSourceId, AssetSource, String)>,
) -> Result<()> {
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(root).with_context(|| {
            format!(
                "failed to read asset relative path for {}",
                entry.path().display()
            )
        })?;
        let rel = normalize_rel_path(rel)?;
        if rel.0 == "README.md" {
            continue;
        }
        if rel.0 == "css/vars.css" {
            continue;
        }
        let source = AssetSourceId(entry.path().to_string_lossy().to_string());
        let bytes = std::fs::read(entry.path())
            .with_context(|| format!("failed to read {}", entry.path().display()))?;
        let content_hash = blake3::hash(&bytes).to_hex().to_string();
        out.insert(
            rel,
            (
                source,
                AssetSource::File(entry.path().to_path_buf()),
                content_hash,
            ),
        );
    }
    Ok(())
}

fn normalize_rel_path(path: &Path) -> Result<AssetRelPath> {
    let raw = path.to_string_lossy().replace('\\', "/");
    let rel = Path::new(&raw);
    if rel.is_absolute() {
        bail!("asset rel path must be relative: {}", raw);
    }
    for comp in rel.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("asset rel path must not contain parent/root: {}", raw);
            }
            _ => {}
        }
    }
    if raw.is_empty() {
        bail!("asset rel path must not be empty");
    }
    Ok(AssetRelPath(raw))
}
