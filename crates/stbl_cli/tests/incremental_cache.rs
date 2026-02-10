use std::fs;
use std::path::{Path, PathBuf};

use stbl_cache::SqliteCacheStore;
use stbl_cli::{assets, exec, media, walk};
use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::UnknownKeyPolicy;
use stbl_core::model::Project;
use tempfile::TempDir;
use walkdir::WalkDir;

#[test]
fn cache_skips_unchanged_tasks_and_rebuilds_missing_output() {
    let temp = TempDir::new().expect("tempdir");
    let source_dir = temp.path().join("source");
    copy_dir_all(&fixture_root("site1"), &source_dir).expect("copy fixture");
    let out_dir = temp.path().join("out");
    let cache_path = temp.path().join("cache.sqlite");

    let mut project = build_project(&source_dir);
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
    let asset_manifest = stbl_core::assets::build_asset_manifest(
        &asset_index,
        project.config.assets.cache_busting,
    );
    let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);

    let mut cache = SqliteCacheStore::open(&cache_path).expect("open cache");
    let report_first = exec::execute_plan(
        &project,
        &plan,
        &out_dir,
        &asset_index,
        &asset_lookup,
        &image_lookup,
        &video_lookup,
        &asset_manifest,
        Some(&mut cache),
        None,
        false,
    )
    .expect("execute plan");
    assert!(report_first.executed > 0);

    let report_second = exec::execute_plan(
        &project,
        &plan,
        &out_dir,
        &asset_index,
        &asset_lookup,
        &image_lookup,
        &video_lookup,
        &asset_manifest,
        Some(&mut cache),
        None,
        false,
    )
    .expect("execute plan");
    assert_eq!(report_second.executed, 0);
    assert!(report_second.skipped > 0);

    let output_path = first_output_path(&plan, &out_dir);
    assert!(output_path.exists());
    fs::remove_file(&output_path).expect("remove output");

    let report_third = exec::execute_plan(
        &project,
        &plan,
        &out_dir,
        &asset_index,
        &asset_lookup,
        &image_lookup,
        &video_lookup,
        &asset_manifest,
        Some(&mut cache),
        None,
        false,
    )
    .expect("execute plan");
    assert!(report_third.executed >= 1);
    assert!(output_path.exists());
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stbl_core")
        .join("tests")
        .join("fixtures")
        .join(name)
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

fn first_output_path(plan: &stbl_core::model::BuildPlan, out_dir: &Path) -> PathBuf {
    for task in &plan.tasks {
        if let Some(output) = task.outputs.first() {
            return out_dir.join(&output.path);
        }
    }
    panic!("no outputs in plan");
}
