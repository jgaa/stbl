use std::fs;

use stbl_cli::assets::discover_assets_for_theme;
use stbl_core::assets::build_asset_manifest;
use stbl_core::config::load_site_config;
use stbl_core::header::Header;
use stbl_core::model::{DocId, Page, Project, SiteContent};
use stbl_core::templates::render_page;
use tempfile::TempDir;

#[test]
fn built_in_themes_load_foldable_cards_with_hashed_assets() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("stbl.yaml");
    for theme in ["stbl", "minimal", "product", "liberty", "paper", "mono"] {
        fs::write(
            &config_path,
            format!(
                "site:\n  id: demo\n  title: Demo\n  base_url: https://example.com/blog/\n  language: en\ntheme:\n  variant: {theme}\n"
            ),
        )
        .expect("write config");
        let project = Project {
            root: temp.path().into(),
            config: load_site_config(&config_path).expect("load config"),
            content: SiteContent::default(),
            image_alpha: Default::default(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        };
        let page = Page {
            id: DocId(blake3::hash(b"folded")),
            source_path: "articles/index.md".into(),
            header: Header {
                title: Some("Cards".into()),
                layout: Some("landing".into()),
                ..Header::default()
            },
            body_markdown: "::: features\n::: card folded=auto\n### Card\n\nBody.\n:::\n::: card folded=yes\n### Folded\n\nFolded body.\n:::\n::: card folded=no\n### Expanded\n\nExpanded body.\n:::\n::: card folded=off\n### Static\n\nStatic body.\n:::\n:::\n"
                .into(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: "index".into(),
            content_hash: blake3::hash(b"folded"),
        };
        let (index, _) = discover_assets_for_theme(temp.path(), theme).expect("discover assets");
        let manifest = build_asset_manifest(&index, true);
        let script = &manifest.entries["js/landing-cards.js"];
        assert!(script.starts_with("artifacts/js/landing-cards."));
        let html = render_page(
            &project,
            &page,
            &manifest,
            "/blog/index/",
            "2026-10-19",
            None,
            None,
        )
        .expect("render cards");
        assert!(
            html.contains(&format!(
                "{}\" defer></script>",
                script.replace('/', "&#x2f;")
            )),
            "{theme}"
        );
        for mode in ["yes", "no", "auto", "off"] {
            assert!(
                html.contains(&format!("data-folded=\"{mode}\"")),
                "{theme}: {mode}"
            );
        }
        assert!(html.contains("<h3 id=\"card\">Card</h3>"), "{theme}");
        assert!(html.contains("<p>Body.</p>"), "{theme}");
        assert!(
            !html.contains("<button"),
            "{theme}: enhancement needs JavaScript"
        );
    }
}
