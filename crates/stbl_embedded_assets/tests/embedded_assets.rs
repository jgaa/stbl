use std::collections::BTreeSet;

use stbl_embedded_assets as embedded;

#[test]
fn default_template_assets_roundtrip() {
    let template = embedded::template("stbl").expect("stbl template");
    let mut paths = BTreeSet::new();
    for entry in template.assets {
        assert!(paths.insert(entry.path), "duplicate path: {}", entry.path);
        let gzip = embedded::asset_bytes_gzip(&entry.hash).expect("gzip bytes");
        assert!(!gzip.is_empty());
        let raw_len = embedded::asset_raw_len(&entry.hash).expect("raw len");
        let bytes = embedded::decompress_to_vec(&entry.hash).expect("decompress");
        assert_eq!(
            bytes.len(),
            raw_len as usize,
            "len mismatch for {}",
            entry.path
        );
        let hash = blake3::hash(&bytes);
        assert_eq!(
            hash.as_bytes(),
            &entry.hash,
            "hash mismatch for {}",
            entry.path
        );
    }
}

#[test]
fn default_template_colors_yaml_is_embedded() {
    let bytes = embedded::template_colors_yaml("stbl").expect("stbl colors yaml");
    let text = std::str::from_utf8(bytes).expect("utf8 colors yaml");
    assert!(text.contains("base:"), "missing base section");
    assert!(text.contains("nav:"), "missing nav section");
    assert!(
        text.contains("wide_background:"),
        "missing wide_background section"
    );
}

#[test]
fn minimal_template_is_embedded_with_theme_overrides() {
    let template = embedded::template("minimal").expect("minimal template");
    let paths = template
        .assets
        .iter()
        .map(|entry| entry.path)
        .collect::<BTreeSet<_>>();

    for required in [
        "minimal.colors.yaml",
        "templates/base.html",
        "templates/partials/header.html",
        "templates/partials/blog_index.html",
        "css/common.css",
        "css/mobile.css",
    ] {
        assert!(paths.contains(required), "missing minimal asset: {required}");
    }

    let colors = embedded::template_colors_yaml("minimal").expect("minimal colors yaml");
    assert!(std::str::from_utf8(colors)
        .expect("minimal colors utf8")
        .contains("#315f8c"));
}

#[test]
fn liberty_theme_is_embedded_with_dark_defaults() {
    let template = embedded::template("liberty").expect("liberty template");
    let paths = template
        .assets
        .iter()
        .map(|entry| entry.path)
        .collect::<BTreeSet<_>>();

    for required in [
        "liberty.colors.yaml",
        "css/common.css",
        "css/desktop.css",
        "css/mobile.css",
        "css/syntax.css",
    ] {
        assert!(paths.contains(required), "missing liberty asset: {required}");
    }

    let colors = embedded::template_colors_yaml("liberty").expect("liberty colors yaml");
    let text = std::str::from_utf8(colors).expect("liberty colors utf8");
    assert!(text.contains("#111317"));
    assert!(text.contains("#68d6ff"));
}

#[test]
fn paper_theme_is_embedded_with_warm_defaults() {
    let template = embedded::template("paper").expect("paper template");
    let paths = template
        .assets
        .iter()
        .map(|entry| entry.path)
        .collect::<BTreeSet<_>>();

    for required in [
        "paper.colors.yaml",
        "css/common.css",
        "css/desktop.css",
        "css/mobile.css",
        "css/syntax.css",
    ] {
        assert!(paths.contains(required), "missing paper asset: {required}");
    }

    let colors = embedded::template_colors_yaml("paper").expect("paper colors yaml");
    let text = std::str::from_utf8(colors).expect("paper colors utf8");
    assert!(text.contains("#faf8f4"));
    assert!(text.contains("#8a5b2a"));
}

#[test]
fn mono_theme_is_embedded_with_monochrome_defaults() {
    let template = embedded::template("mono").expect("mono template");
    let paths = template
        .assets
        .iter()
        .map(|entry| entry.path)
        .collect::<BTreeSet<_>>();

    for required in [
        "mono.colors.yaml",
        "css/common.css",
        "css/desktop.css",
        "css/mobile.css",
        "css/syntax.css",
    ] {
        assert!(paths.contains(required), "missing mono asset: {required}");
    }

    let colors = embedded::template_colors_yaml("mono").expect("mono colors yaml");
    let text = std::str::from_utf8(colors).expect("mono colors utf8");
    assert!(text.contains("#ffffff"));
    assert!(text.contains("#111111"));
}
