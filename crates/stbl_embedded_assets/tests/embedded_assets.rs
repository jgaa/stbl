use std::collections::BTreeSet;

use stbl_embedded_assets as embedded;

#[test]
fn default_template_assets_roundtrip() {
    let template = embedded::template("default").expect("default template");
    let mut paths = BTreeSet::new();
    for entry in template.assets {
        assert!(paths.insert(entry.path), "duplicate path: {}", entry.path);
        let gzip = embedded::asset_bytes_gzip(&entry.hash).expect("gzip bytes");
        assert!(!gzip.is_empty());
        let raw_len = embedded::asset_raw_len(&entry.hash).expect("raw len");
        let bytes = embedded::decompress_to_vec(&entry.hash).expect("decompress");
        assert_eq!(bytes.len(), raw_len as usize, "len mismatch for {}", entry.path);
        let hash = blake3::hash(&bytes);
        assert_eq!(hash.as_bytes(), &entry.hash, "hash mismatch for {}", entry.path);
    }
}
