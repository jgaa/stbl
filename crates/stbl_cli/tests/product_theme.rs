use std::fs;

use stbl_cli::color_presets::load_color_presets;
use stbl_core::assets::AssetManifest;
use stbl_core::config::load_site_config;
use stbl_core::header::Header;
use stbl_core::model::{DocId, Page, Project, SiteContent};
use stbl_core::templates::render_page;
use stbl_core::theme::resolve_theme_vars;
use tempfile::TempDir;

const CONFIG: &str = "site:\n  id: product-demo\n  title: Product Demo\n  base_url: https://example.com/products/demo/\n  language: en\n  url_style: pretty\ntheme:\n  variant: product\n  header:\n    action:\n      title: Get Started\n      href: download\n";

#[test]
fn product_theme_renders_actions_without_inline_scripts_with_every_color_preset() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("stbl.yaml");
    fs::write(&config_path, CONFIG).expect("write config");
    let config = load_site_config(&config_path).expect("load config");
    let page = Page {
        id: DocId(blake3::hash(b"product")),
        source_path: "articles/index.md".into(),
        header: Header {
            title: Some("Product".into()),
            layout: Some("landing".into()),
            ..Header::default()
        },
        body_markdown: "::: hero\n# Product\n\n@[button](text=\"Get Started\", href=download)\n@[button](text=\"Watch Video\", href=\"#demo\", kind=secondary, icon=play)\n\n![Application](images/demo.svg)\n:::\n\n::: callout\n## Ready?\n\n@[button](text=\"Get Started\", href=download)\n:::\n".into(),
        banner_name: None,
        media_refs: Vec::new(),
        url_path: "index".into(),
        content_hash: blake3::hash(b"product"),
    };
    let mut project = Project {
        root: temp.path().into(),
        config,
        content: SiteContent::default(),
        image_alpha: Default::default(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    };
    let defaults = stbl_embedded_assets::template_colors_yaml("product").expect("product defaults");
    let presets = load_color_presets().expect("presets");
    for (name, preset) in presets {
        project.config.theme.colors = preset.colors;
        project.config.theme.nav = preset.nav;
        let vars = resolve_theme_vars(defaults, &project.config).expect("resolve colors");
        assert!(
            contrast(&vars.c_accent, &vars.c_accent_fg) >= 4.5,
            "{name}: button contrast"
        );
        let html = render_page(
            &project,
            &page,
            &AssetManifest::default(),
            "/products/demo/index/",
            "2026-10-06",
            None,
            None,
        )
        .expect("render product page");
        assert!(!html.contains("<script>"), "{name}: theme needs scripts");
        assert_eq!(
            html.matches("button--primary").count(),
            3,
            "{name}: header, hero, callout actions"
        );
        assert!(html.contains("button--secondary button--play"), "{name}");
        assert!(html.contains("href=\"/products/demo/download/\""), "{name}");
        assert!(html.contains("href=\"#demo\""), "{name}");
        assert!(!html.contains("@[button]"), "{name}");
    }
}

#[test]
fn header_action_requires_both_nonempty_fields() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("stbl.yaml");
    for action in [
        "title: Get Started",
        "href: download",
        "title: ''\n      href: download",
        "title: Get Started\n      href: ''",
    ] {
        let config = format!(
            "site:\n  id: demo\n  title: Demo\n  base_url: https://example.com/\n  language: en\ntheme:\n  header:\n    action:\n      {action}\n"
        );
        fs::write(&path, config).expect("write config");
        let error = load_site_config(&path).expect_err("invalid action");
        assert!(error.to_string().contains("theme.header.action"));
    }
}

fn contrast(first: &str, second: &str) -> f64 {
    fn luminance(hex: &str) -> f64 {
        let values = [1, 3, 5].map(|offset| {
            let value =
                f64::from(u8::from_str_radix(&hex[offset..offset + 2], 16).expect("hex")) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722
    }
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}
