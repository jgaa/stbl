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

#[test]
fn default_template_colors_yaml_is_embedded() {
    let bytes = embedded::template_colors_yaml("default").expect("default colors yaml");
    let text = std::str::from_utf8(bytes).expect("utf8 colors yaml");
    assert!(text.contains("base:"), "missing base section");
    assert!(text.contains("nav:"), "missing nav section");
    assert!(text.contains("wide_background:"), "missing wide_background section");
}
