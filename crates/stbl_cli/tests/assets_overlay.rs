use std::fs;
use std::path::Path;

use blake3::hash;
use stbl_cli::assets::{discover_assets, discover_assets_for_theme, execute_copy_tasks};
use stbl_core::assets::plan_assets;
use stbl_core::model::{SecurityConfig, SvgSecurityConfig, SvgSecurityMode};
use tempfile::TempDir;

#[test]
fn site_assets_override_theme_assets() {
    let temp = TempDir::new().expect("tempdir");
    let site_root = temp.path().join("site");
    let out_dir = temp.path().join("out");

    write_file(&site_root.join("css/common.css"), "site").expect("write site css");
    write_file(&site_root.join("img/x.svg"), "x").expect("write site img");

    let (asset_index, lookup) = discover_assets(&site_root).expect("discover assets");
    let (tasks, _manifest) = plan_assets(&asset_index, false, blake3::hash(b"config").into());
    let security = SecurityConfig {
        svg: SvgSecurityConfig {
            mode: SvgSecurityMode::Off,
        },
    };
    execute_copy_tasks(&tasks, &out_dir, &lookup, &security).expect("copy assets");

    let css = fs::read_to_string(out_dir.join("artifacts/css/common.css")).expect("read css");
    assert_eq!(css, "site");
    let img = fs::read_to_string(out_dir.join("artifacts/img/x.svg")).expect("read img");
    assert_eq!(img, "x");
}

#[test]
fn cache_busting_emits_hashed_filenames() {
    let temp = TempDir::new().expect("tempdir");
    let site_root = temp.path().join("site");
    let out_dir = temp.path().join("out");

    write_file(&site_root.join("css/common.css"), "site").expect("write site css");

    let (asset_index, lookup) = discover_assets(&site_root).expect("discover assets");
    let (tasks, _manifest) = plan_assets(&asset_index, true, blake3::hash(b"config").into());
    let security = SecurityConfig {
        svg: SvgSecurityConfig {
            mode: SvgSecurityMode::Off,
        },
    };
    execute_copy_tasks(&tasks, &out_dir, &lookup, &security).expect("copy assets");

    let css_dir = out_dir.join("artifacts/css");
    let entries = fs::read_dir(&css_dir).expect("read css dir");
    let files = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let common_files = files
        .iter()
        .filter(|name| name.starts_with("common.") && name.ends_with(".css"))
        .collect::<Vec<_>>();
    assert_eq!(common_files.len(), 1);
    let expected_hash = hash("site".as_bytes()).to_hex();
    let short = &expected_hash[..8];
    let expected_name = format!("common.{short}.css");
    assert_eq!(common_files[0].as_str(), expected_name);
    let contents = fs::read_to_string(css_dir.join(expected_name)).expect("read css");
    assert_eq!(contents, "site");
}

#[test]
fn stbl_theme_assets_override_embedded_assets() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().join("site");
    let out_dir = temp.path().join("out");
    write_file(
        &root.join("stbl/css/stbl/common.css"),
        "from stbl theme override",
    )
    .expect("write stbl override");

    let (asset_index, lookup) = discover_assets_for_theme(&root, "stbl").expect("discover assets");
    let (tasks, _manifest) = plan_assets(&asset_index, false, blake3::hash(b"config").into());
    let security = SecurityConfig {
        svg: SvgSecurityConfig {
            mode: SvgSecurityMode::Off,
        },
    };
    execute_copy_tasks(&tasks, &out_dir, &lookup, &security).expect("copy assets");

    let css = fs::read_to_string(out_dir.join("artifacts/css/common.css")).expect("read css");
    assert_eq!(css, "from stbl theme override");
}

fn write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
