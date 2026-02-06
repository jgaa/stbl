use std::path::PathBuf;

use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::UnknownKeyPolicy;
use stbl_core::media::VideoPlanInput;
use stbl_core::model::{ImageFormatMode, Project};
use tempfile::TempDir;

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
    let docs = stbl_cli::walk::walk_content(
        root,
        &root.join("articles"),
        UnknownKeyPolicy::Error,
        false,
    )
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

#[test]
fn precompress_writes_gzip_for_text_assets() {
    let root = fixture_root("site-media");
    let mut project = build_project(&root, ImageFormatMode::Fast);
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    execute_build(&mut project, &out_dir);

    stbl_cli::precompress::write_gzip_files(&out_dir, 1, false).expect("precompress");

    assert!(out_dir.join("index.html.gz").exists());
    assert!(out_dir.join("artifacts/css/common.css.gz").exists());
    assert!(!out_dir
        .join("artifacts/images/_scale_720/alpha.png.gz")
        .exists());
}

#[test]
fn brotli_writes_by_default() {
    let root = fixture_root("site-media");
    let mut project = build_project(&root, ImageFormatMode::Fast);
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    execute_build(&mut project, &out_dir);

    stbl_cli::precompress::write_gzip_files(&out_dir, 6, false).expect("gzip");
    stbl_cli::precompress::write_brotli_files(&out_dir, 5, false).expect("brotli");

    assert!(out_dir.join("index.html.br").exists());
}

#[test]
fn fast_compress_skips_brotli() {
    let root = fixture_root("site-media");
    let mut project = build_project(&root, ImageFormatMode::Fast);
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    execute_build(&mut project, &out_dir);

    stbl_cli::precompress::write_gzip_files(&out_dir, 1, false).expect("gzip");

    assert!(!out_dir.join("index.html.br").exists());
}
