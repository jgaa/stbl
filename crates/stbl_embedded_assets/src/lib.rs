use flate2::read::GzDecoder;
use std::io::Read;

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

pub fn template_names() -> &'static [&'static str] {
    generated::TEMPLATE_NAMES
}

pub fn template(name: &str) -> Option<&'static Template> {
    generated::TEMPLATES.iter().find(|template| template.name == name)
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

fn find_blob(hash: &[u8; 32]) -> Option<&'static Blob> {
    let index = generated::BLOBS
        .binary_search_by(|blob| blob.hash.cmp(&(*hash)))
        .ok()?;
    Some(&generated::BLOBS[index])
}
