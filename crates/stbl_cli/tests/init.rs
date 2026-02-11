use std::fs;
use std::path::Path;

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
