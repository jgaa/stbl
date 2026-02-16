use std::fs;
use std::path::PathBuf;

use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::UnknownKeyPolicy;
use stbl_core::media::VideoPlanInput;
use stbl_core::model::{ImageFormatMode, Project};
use tempfile::TempDir;
use walkdir::WalkDir;

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stbl_core")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn build_project(root: &PathBuf, format_mode: ImageFormatMode) -> Project {
    let config = load_site_config(&root.join("stbl.yaml")).expect("load config");
    let mut config = config;
    config.media.images.format_mode = format_mode;
    let docs =
        stbl_cli::walk::walk_content(root, &root.join("articles"), UnknownKeyPolicy::Error, false)
            .expect("walk content");
    let content = assemble_site(docs).expect("assemble site");
    Project {
        root: root.clone(),
        config,
        content,
        image_alpha: std::collections::BTreeMap::new(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    }
}

fn execute_build(project: &mut Project, out_dir: &PathBuf) {
    let site_assets_root = project.root.join("assets");
    let (asset_index, asset_lookup) =
        stbl_cli::assets::discover_assets(&site_assets_root).expect("discover assets");
    let (image_plan, image_lookup) =
        stbl_cli::media::discover_images(project).expect("discover images");
    project.image_alpha = image_plan.alpha.clone();
    project.image_variants = stbl_core::media::build_image_variant_index(
        &image_plan,
        &project.config.media.images.widths,
        project.config.media.images.format_mode,
    );
    let video_plan = VideoPlanInput::default();
    let video_lookup = stbl_cli::media::VideoSourceLookup::default();
    let asset_manifest =
        stbl_core::assets::build_asset_manifest(&asset_index, project.config.assets.cache_busting);
    let plan = stbl_core::plan::build_plan(project, &asset_index, &image_plan, &video_plan);
    stbl_cli::exec::execute_plan(
        project,
        &plan,
        out_dir,
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

fn read_page(out_dir: &PathBuf, name: &str) -> String {
    fs::read_to_string(out_dir.join(name)).expect("read page")
}

fn has_avif(out_dir: &PathBuf) -> bool {
    WalkDir::new(out_dir)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("avif"))
        })
}

#[test]
fn normal_mode_emits_avif_sources() {
    let root = fixture_root("site-media");
    let mut project = build_project(&root, ImageFormatMode::Normal);
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    execute_build(&mut project, &out_dir);

    assert!(has_avif(&out_dir), "expected avif outputs in normal mode");
    let html = read_page(&out_dir, "banner-page.html");
    assert!(html.contains("type=\"image/avif\""));
    assert!(html.contains("type=\"image/webp\""));
}

#[test]
fn fast_mode_skips_avif_sources() {
    let root = fixture_root("site-media");
    let mut project = build_project(&root, ImageFormatMode::Fast);
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    execute_build(&mut project, &out_dir);

    assert!(
        !has_avif(&out_dir),
        "did not expect avif outputs in fast mode"
    );
    let html = read_page(&out_dir, "banner-page.html");
    assert!(!html.contains("type=\"image/avif\""));
    assert!(html.contains("type=\"image/webp\""));
}

#[test]
fn alpha_fallback_uses_png() {
    let root = fixture_root("site-media");
    let mut project = build_project(&root, ImageFormatMode::Fast);
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    execute_build(&mut project, &out_dir);

    let original = out_dir.join("images/alpha.png");
    assert!(original.exists(), "missing original alpha.png fallback");
    let html = read_page(&out_dir, "index.html");
    assert!(html.contains("images/alpha.png"));
    assert!(!html.contains("alpha.jpg"));
}
