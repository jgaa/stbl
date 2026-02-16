use flate2::read::GzDecoder;
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Mutex, OnceLock};

mod generated;

pub struct Blob {
    pub hash: [u8; 32],
    pub gzip: &'static [u8],
    pub raw_len: u32,
}

pub struct AssetEntry {
    pub path: &'static str,
    pub hash: [u8; 32],
}

pub struct Template {
    pub name: &'static str,
    pub assets: &'static [AssetEntry],
}

#[derive(Debug)]
pub enum AssetError {
    MissingTemplate { name: String },
    MissingTemplateAsset { template: String, path: String },
    DecompressFailed { template: String, path: String },
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::MissingTemplate { name } => write!(f, "missing template: {}", name),
            AssetError::MissingTemplateAsset { template, path } => {
                write!(f, "missing asset {} in template {}", path, template)
            }
            AssetError::DecompressFailed { template, path } => {
                write!(
                    f,
                    "failed to decompress asset {} in template {}",
                    path, template
                )
            }
        }
    }
}

impl std::error::Error for AssetError {}

static COLORS_CACHE: OnceLock<Mutex<BTreeMap<String, &'static [u8]>>> = OnceLock::new();

pub fn template_names() -> &'static [&'static str] {
    generated::TEMPLATE_NAMES
}

pub fn template(name: &str) -> Option<&'static Template> {
    generated::TEMPLATES
        .iter()
        .find(|template| template.name == name)
}

pub fn asset_bytes_gzip(hash: &[u8; 32]) -> Option<&'static [u8]> {
    find_blob(hash).map(|blob| blob.gzip)
}

pub fn asset_raw_len(hash: &[u8; 32]) -> Option<u32> {
    find_blob(hash).map(|blob| blob.raw_len)
}

pub fn decompress_to_vec(hash: &[u8; 32]) -> Option<Vec<u8>> {
    let blob = find_blob(hash)?;
    let mut decoder = GzDecoder::new(blob.gzip);
    let mut out = Vec::with_capacity(blob.raw_len as usize);
    if decoder.read_to_end(&mut out).is_err() {
        return None;
    }
    if out.len() != blob.raw_len as usize {
        return None;
    }
    Some(out)
}

pub fn template_colors_yaml(variant: &str) -> Result<&'static [u8], AssetError> {
    let template = template(variant).ok_or_else(|| AssetError::MissingTemplate {
        name: variant.to_string(),
    })?;
    let path = format!("{}.colors.yaml", variant);

    if let Some(cache) = COLORS_CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .ok()
    {
        if let Some(bytes) = cache.get(variant) {
            return Ok(bytes);
        }
    }

    let entry = template
        .assets
        .iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| AssetError::MissingTemplateAsset {
            template: variant.to_string(),
            path: path.clone(),
        })?;

    let bytes = decompress_to_vec(&entry.hash).ok_or_else(|| AssetError::DecompressFailed {
        template: variant.to_string(),
        path: path.clone(),
    })?;

    let leaked = Box::leak(bytes.into_boxed_slice());
    let mut cache = COLORS_CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .expect("colors cache lock");
    cache.insert(variant.to_string(), leaked);
    Ok(leaked)
}

fn find_blob(hash: &[u8; 32]) -> Option<&'static Blob> {
    let index = generated::BLOBS
        .binary_search_by(|blob| blob.hash.cmp(&(*hash)))
        .ok()?;
    Some(&generated::BLOBS[index])
}
