use std::fs;
use std::path::Path;

use stbl_cli::config_loader::load_config_for_build;
use stbl_cli::upgrade::upgrade_site;
use tempfile::TempDir;

#[test]
fn upgrade_generates_yaml() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();
    write_legacy_config(root, "Demo Site", "https://example.com/");

    let output = upgrade_site(root, false).expect("upgrade");
    assert!(!output.yaml.is_empty());

    let yaml_path = root.join("stbl.yaml");
    assert!(yaml_path.exists());
    let yaml = fs::read_to_string(yaml_path).expect("read yaml");
    assert!(yaml.contains("site:"));
    assert!(yaml.contains("id:"));
    assert!(yaml.contains("title:"));
    assert!(yaml.contains("Demo Site"));
    assert!(yaml.contains("base_url:"));
    assert!(yaml.contains("https://example.com/"));
    assert!(yaml.contains("language:"));
    assert!(yaml.contains("en"));
}

#[test]
fn build_requires_yaml() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();
    write_legacy_config(root, "Demo Site", "https://example.com/");

    let err = load_config_for_build(root).expect_err("expected missing yaml error");
    let message = err.to_string();
    assert!(message.contains("Missing stbl.yaml"));
    assert!(message.contains("stbl_cli upgrade"));
}

#[test]
fn upgrade_refuses_overwrite() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();
    write_legacy_config(root, "Demo Site", "https://example.com/");
    fs::write(root.join("stbl.yaml"), "site:\n  id: demo\n").expect("write yaml");

    let err = upgrade_site(root, false).expect_err("expected overwrite error");
    assert!(err.to_string().contains("stbl.yaml already exists"));
}

fn write_legacy_config(root: &Path, title: &str, url: &str) {
    let contents = format!("name \"{}\"\nurl \"{}\"\nlanguage en\n", title, url);
    fs::write(root.join("stbl.conf"), contents).expect("write legacy config");
}
