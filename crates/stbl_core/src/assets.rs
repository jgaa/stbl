use crate::model::{BuildTask, ContentId, InputFingerprint, OutputArtifact, TaskId, TaskKind};
use blake3::{Hasher, hash};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetRelPath(pub String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetSourceId(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAsset {
    pub rel: AssetRelPath,
    pub source: AssetSourceId,
    pub content_hash: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetIndex {
    pub assets: Vec<ResolvedAsset>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetManifest {
    pub entries: BTreeMap<String, String>,
}

pub fn build_asset_manifest(asset_index: &AssetIndex, cache_busting: bool) -> AssetManifest {
    let mut entries = BTreeMap::new();
    for asset in &asset_index.assets {
        let out_rel = out_rel_for_asset(&asset.rel, &asset.content_hash, cache_busting);
        entries.insert(asset.rel.0.clone(), out_rel);
    }
    entries.insert(
        "css/vars.css".to_string(),
        "artifacts/css/vars.css".to_string(),
    );
    AssetManifest { entries }
}

pub fn plan_assets(
    asset_index: &AssetIndex,
    cache_busting: bool,
    render_config_hash: [u8; 32],
) -> (Vec<BuildTask>, AssetManifest) {
    let mut items = asset_index
        .assets
        .iter()
        .filter(|asset| asset.rel.0 != "css/vars.css")
        .cloned()
        .collect::<Vec<_>>();
    items.sort_by(|a, b| a.rel.cmp(&b.rel));

    for window in items.windows(2) {
        if window[0].rel == window[1].rel {
            panic!("duplicate asset rel path: {}", window[0].rel.0);
        }
    }

    let manifest = build_asset_manifest(asset_index, cache_busting);
    let tasks = items
        .into_iter()
        .map(|asset| {
            let rel = asset.rel.clone();
            let out_rel = out_rel_for_asset(&rel, &asset.content_hash, cache_busting);
            let kind = TaskKind::CopyAsset {
                rel: rel.clone(),
                source: asset.source.clone(),
                out_rel: out_rel.clone(),
            };
            let id = TaskId::new("copy_asset", &[rel.0.as_str()]);
            let inputs_fingerprint =
                fingerprint_copy_asset(&id, render_config_hash, &asset.content_hash);
            BuildTask {
                id,
                kind,
                inputs_fingerprint,
                inputs: vec![ContentId::Asset(rel)],
                outputs: vec![OutputArtifact {
                    path: std::path::PathBuf::from(out_rel),
                }],
            }
        })
        .collect();

    (tasks, manifest)
}

fn fingerprint_copy_asset(
    task_id: &TaskId,
    render_config_hash: [u8; 32],
    content_hash: &str,
) -> InputFingerprint {
    let mut hasher = Hasher::new();
    hasher.update(b"stbl2.task.v1");
    add_str(&mut hasher, &task_id.0);
    add_str(&mut hasher, "CopyAsset");
    hasher.update(&render_config_hash);
    hasher.update(hash(content_hash.as_bytes()).as_bytes());
    InputFingerprint(*hasher.finalize().as_bytes())
}

fn add_str(hasher: &mut Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn out_rel_for_asset(rel: &AssetRelPath, content_hash: &str, cache_busting: bool) -> String {
    let rel_path = Path::new(&rel.0);
    let file_name = rel_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((stem, ext)) => (stem, Some(ext)),
        None => (file_name, None),
    };
    let hashed_name = if cache_busting {
        let short = content_hash.get(0..8).unwrap_or(content_hash);
        match ext {
            Some(ext) => format!("{stem}.{short}.{ext}"),
            None => format!("{stem}.{short}"),
        }
    } else {
        file_name.to_string()
    };
    let parent = rel_path.parent().and_then(|parent| {
        let text = parent.to_string_lossy();
        if text.is_empty() {
            None
        } else {
            Some(text.replace('\\', "/"))
        }
    });
    match parent {
        Some(parent) => format!("artifacts/{parent}/{hashed_name}"),
        None => format!("artifacts/{hashed_name}"),
    }
}
