use std::fs;
use std::path::{Path, PathBuf};

use stbl_cli::{assets, exec, media, walk};
use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::UnknownKeyPolicy;
use stbl_core::model::Project;
use tempfile::TempDir;
use walkdir::WalkDir;

#[test]
fn build_uses_embedded_assets_with_overrides() {
    let temp = TempDir::new().expect("tempdir");
    let source_dir = temp.path().join("source");
    copy_dir_all(&fixture_root("site1"), &source_dir).expect("copy fixture");

    let assets_dir = source_dir.join("assets");
    fs::create_dir_all(&assets_dir).expect("create assets dir");
    fs::write(assets_dir.join("README.md"), "embedded assets test\n").expect("write readme");

    let out_dir = temp.path().join("out");
    run_build(&source_dir, &out_dir);

    for path in [
        out_dir.join("artifacts/css/common.css"),
        out_dir.join("artifacts/css/desktop.css"),
        out_dir.join("artifacts/css/mobile.css"),
        out_dir.join("artifacts/css/wide-desktop.css"),
    ] {
        let contents = fs::read_to_string(path).expect("read css");
        assert!(!contents.trim().is_empty());
    }

    let override_css = "/* override */\n";
    let override_path = assets_dir.join("css/common.css");
    fs::create_dir_all(override_path.parent().expect("css parent")).expect("mkdir css");
    fs::write(&override_path, override_css).expect("write override");

    let out_dir_override = temp.path().join("out_override");
    run_build(&source_dir, &out_dir_override);

    let css = fs::read_to_string(out_dir_override.join("artifacts/css/common.css"))
        .expect("read override css");
    assert!(css.contains("override"));
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stbl_core")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn run_build(source_dir: &Path, out_dir: &Path) {
    let mut project = build_project(source_dir);
    let site_assets_root = source_dir.join("assets");
    let (asset_index, asset_lookup) =
        assets::discover_assets(&site_assets_root).expect("discover assets");
    let (image_plan, image_lookup) = media::discover_images(&project).expect("discover images");
    project.image_alpha = image_plan.alpha.clone();
    project.image_variants = stbl_core::media::build_image_variant_index(
        &image_plan,
        &project.config.media.images.widths,
        project.config.media.images.format_mode,
    );
    let (video_plan, video_lookup) = media::discover_videos(&project).expect("discover videos");
    project.video_variants = stbl_core::media::build_video_variant_index(
        &video_plan,
        &project.config.media.video.heights,
    );
    let asset_manifest =
        stbl_core::assets::build_asset_manifest(&asset_index, project.config.assets.cache_busting);
    let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);

    exec::execute_plan(
        &project,
        &plan,
        &out_dir.to_path_buf(),
        &asset_index,
        &asset_lookup,
        &image_lookup,
        &video_lookup,
        &asset_manifest,
        None,
        None,
        false,
    )
    .expect("execute plan");
}

fn build_project(root: &Path) -> Project {
    let config_path = root.join("stbl.yaml");
    let config = load_site_config(&config_path).expect("load config");
    let docs = walk::walk_content(root, &root.join("articles"), UnknownKeyPolicy::Error, false)
        .expect("walk content");
    let content = assemble_site(docs).expect("assemble site");
    Project {
        root: root.to_path_buf(),
        config,
        content,
        image_alpha: std::collections::BTreeMap::new(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(src).expect("strip prefix");
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}
