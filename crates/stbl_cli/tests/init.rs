use std::fs;
use std::path::Path;

use serde_yaml::Value;
use stbl_cli::color_presets;
use stbl_cli::init::{InitKind, InitOptions, init_site};
use tempfile::TempDir;

#[test]
fn init_creates_structure() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().to_path_buf();
    run_init(&root, InitKind::Blog, false);

    assert!(root.join("stbl.yaml").exists());
    assert!(root.join("articles").is_dir());
    assert!(root.join("artifacts").is_dir());
    assert!(root.join("images").is_dir());
    assert!(root.join("assets").is_dir());
    assert!(root.join("video").is_dir());

    assert!(root.join("articles/index.md").exists());
    assert!(root.join("articles/about.md").exists());
    assert!(root.join("articles/contact.md").exists());

    assert!(root.join("assets/README.md").exists());
    assert!(!root.join("assets/css/common.css").exists());

    for path in [
        root.join("articles/index.md"),
        root.join("articles/about.md"),
        root.join("articles/contact.md"),
    ] {
        let contents = fs::read_to_string(path).expect("read markdown");
        assert!(!contents.contains("published:"));
    }
}

#[test]
fn init_aborts_if_config_exists() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();
    fs::write(root.join("stbl.yaml"), "site:\n  id: test\n").expect("write config");
    let err = init_site(InitOptions {
        title: "Demo".to_string(),
        base_url: "http://localhost:8080/".to_string(),
        language: "en".to_string(),
        kind: InitKind::Blog,
        color_theme: None,
        copy_all: false,
        target_dir: root.to_path_buf(),
    })
    .expect_err("expected error");
    assert!(err.to_string().contains("stbl.yaml"));
}

#[test]
fn init_aborts_if_required_dir_exists() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();
    fs::create_dir_all(root.join("articles")).expect("create articles");
    let err = init_site(InitOptions {
        title: "Demo".to_string(),
        base_url: "http://localhost:8080/".to_string(),
        language: "en".to_string(),
        kind: InitKind::Blog,
        color_theme: None,
        copy_all: false,
        target_dir: root.to_path_buf(),
    })
    .expect_err("expected error");
    assert!(err.to_string().contains("articles"));
}

#[test]
fn init_kind_changes_index_template() {
    let temp_blog = TempDir::new().expect("tempdir");
    let blog_root = temp_blog.path().to_path_buf();
    run_init(&blog_root, InitKind::Blog, false);
    let blog_index = fs::read_to_string(blog_root.join("articles/index.md")).expect("read index");
    assert!(blog_index.contains("template: blog_index"));

    let temp_landing = TempDir::new().expect("tempdir");
    let landing_root = temp_landing.path().to_path_buf();
    run_init(&landing_root, InitKind::LandingPage, false);
    let landing_index =
        fs::read_to_string(landing_root.join("articles/index.md")).expect("read index");
    assert!(landing_index.contains("template: info"));
}

#[test]
fn init_copy_all_writes_css() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().to_path_buf();
    run_init(&root, InitKind::Blog, true);

    for path in [
        root.join("assets/css/common.css"),
        root.join("assets/css/desktop.css"),
        root.join("assets/css/mobile.css"),
        root.join("assets/css/wide-desktop.css"),
    ] {
        let contents = fs::read_to_string(path).expect("read css");
        assert!(!contents.trim().is_empty());
    }
}

#[test]
fn init_color_theme_matches_apply_colors() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().to_path_buf();
    init_site(InitOptions {
        title: "My Site".to_string(),
        base_url: "http://localhost:8080/".to_string(),
        language: "en".to_string(),
        kind: InitKind::Blog,
        color_theme: Some("slate".to_string()),
        copy_all: false,
        target_dir: root.clone(),
    })
    .expect("init site");

    let presets = color_presets::load_color_presets().expect("load presets");
    let preset = presets.get("slate").expect("preset");
    let raw = fs::read_to_string(root.join("stbl.yaml")).expect("read config");
    let doc: Value = serde_yaml::from_str(&raw).expect("parse config");
    let theme = doc
        .get("theme")
        .and_then(|value| value.as_mapping())
        .expect("theme mapping");
    let colors = theme
        .get(&Value::String("colors".to_string()))
        .and_then(|value| value.as_mapping())
        .expect("colors mapping");
    let nav = theme
        .get(&Value::String("nav".to_string()))
        .and_then(|value| value.as_mapping())
        .expect("nav mapping");
    let scheme = theme
        .get(&Value::String("color_scheme".to_string()))
        .and_then(|value| value.as_mapping())
        .expect("color_scheme mapping");

    let bg = colors
        .get(&Value::String("bg".to_string()))
        .and_then(|value| value.as_str())
        .expect("theme.colors.bg");
    let nav_bg = nav
        .get(&Value::String("bg".to_string()))
        .and_then(|value| value.as_str())
        .expect("theme.nav.bg");
    let scheme_name = scheme
        .get(&Value::String("name".to_string()))
        .and_then(|value| value.as_str())
        .expect("theme.color_scheme.name");
    let scheme_source = scheme
        .get(&Value::String("source".to_string()))
        .and_then(|value| value.as_str())
        .expect("theme.color_scheme.source");

    assert_eq!(bg, preset.colors.bg.as_deref().expect("preset bg"));
    assert_eq!(nav_bg, preset.nav.bg.as_deref().expect("preset nav.bg"));
    assert_eq!(scheme_name, "slate");
    assert_eq!(scheme_source, "preset");
}

fn run_init(root: &Path, kind: InitKind, copy_all: bool) {
    init_site(InitOptions {
        title: "My Site".to_string(),
        base_url: "http://localhost:8080/".to_string(),
        language: "en".to_string(),
        kind,
        color_theme: None,
        copy_all,
        target_dir: root.to_path_buf(),
    })
    .expect("init site");
}
