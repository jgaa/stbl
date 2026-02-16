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
fn edit_one_article_triggers_minimal_rebuild() {
    let temp = TempDir::new().expect("tempdir");
    let source_dir = temp.path().join("source");
    copy_dir_all(&fixture_root("site1-rss"), &source_dir).expect("copy fixture");
    let out_dir = temp.path().join("out");
    let cache_path = temp.path().join("cache.sqlite");
    let mut cache = SqliteCacheStore::open(&cache_path).expect("open cache");

    let first = run_build(&source_dir, &out_dir, &mut cache);
    assert!(first.executed > 0);
    let skipped_non_assets = first
        .skipped_ids
        .iter()
        .filter(|id| !id.starts_with("copy_asset:"))
        .count();
    assert_eq!(
        skipped_non_assets, 0,
        "first build skipped: {:?}",
        first.skipped_ids
    );

    let second = run_build(&source_dir, &out_dir, &mut cache);
    let second_non_assets = second
        .executed_ids
        .iter()
        .filter(|id| !id.starts_with("copy_asset:"))
        .count();
    assert_eq!(
        second_non_assets, 0,
        "second build executed: {:?}",
        second.executed_ids
    );
    assert!(second.skipped > 0);

    let edited_path = source_dir.join("articles").join("page1.md");
    let mut contents = fs::read_to_string(&edited_path).expect("read page1");
    contents.push_str("\n\nEdit for incremental test.\n");
    fs::write(&edited_path, contents).expect("write page1");

    let third = run_build(&source_dir, &out_dir, &mut cache);
    let must_include = [
        "render_page:page1",
        "render_tag:rust",
        "render_tag:cli",
        "render_blog_index:",
        "generate_rss",
        "generate_sitemap",
    ];
    let must_exclude = ["render_page:page2"];

    let executed_pages: Vec<&String> = third
        .executed_ids
        .iter()
        .filter(|id| id.starts_with("render_page:"))
        .collect();
    assert_eq!(
        executed_pages.len(),
        1,
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        executed_pages[0].contains("page1"),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        third
            .executed_ids
            .iter()
            .any(|id| id.starts_with("render_tag:rust")),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        third
            .executed_ids
            .iter()
            .any(|id| id.starts_with("render_tag:cli")),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        third
            .executed_ids
            .iter()
            .any(|id| id.starts_with("render_blog_index:")),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        third.executed_ids.iter().any(|id| id == "generate_rss"),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        third.executed_ids.iter().any(|id| id == "generate_sitemap"),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
    assert!(
        third
            .skipped_ids
            .iter()
            .any(|id| id.starts_with("render_page:") && id.contains("page2")),
        "{}",
        rebuild_diff(&third, &must_include, &must_exclude)
    );
}

fn rebuild_diff(
    summary: &exec::ExecSummary,
    must_include: &[&str],
    must_exclude: &[&str],
) -> String {
    let mut out = String::new();
    out.push_str("rebuild diff\\n");
    out.push_str("executed_ids:\\n");
    for id in &summary.executed_ids {
        out.push_str("  - ");
        out.push_str(id);
        out.push('\n');
    }
    out.push_str("skipped_ids:\\n");
    for id in &summary.skipped_ids {
        out.push_str("  - ");
        out.push_str(id);
        out.push('\n');
    }
    out.push_str("must_include:\\n");
    for id in must_include {
        out.push_str("  - ");
        out.push_str(id);
        out.push('\n');
    }
    out.push_str("must_exclude:\\n");
    for id in must_exclude {
        out.push_str("  - ");
        out.push_str(id);
        out.push('\n');
    }
    out
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stbl_core")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn run_build(source_dir: &Path, out_dir: &Path, cache: &mut SqliteCacheStore) -> exec::ExecSummary {
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
        Some(cache),
        None,
        false,
    )
    .expect("execute plan")
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
