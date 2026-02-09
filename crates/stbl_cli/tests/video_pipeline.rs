use std::fs;
use std::path::PathBuf;
use std::process::Command;

use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::UnknownKeyPolicy;
use stbl_core::model::Project;
use tempfile::TempDir;

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stbl_core")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
}

#[test]
fn video_pipeline_generates_variants_and_posters() {
    if !ffmpeg_available() {
        eprintln!("ffmpeg not available; skipping video pipeline test");
        return;
    }

    let root = fixture_root("site-media");
    let config = load_site_config(&root.join("stbl.yaml")).expect("load config");
    let docs = stbl_cli::walk::walk_content(
        &root,
        &root.join("articles"),
        UnknownKeyPolicy::Error,
        false,
    )
        .expect("walk content");
    let content = assemble_site(docs).expect("assemble site");
    let mut project = Project {
        root: root.clone(),
        config,
        content,
        image_alpha: std::collections::BTreeMap::new(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    };

    let site_assets_root = root.join("assets");
    let (asset_index, asset_lookup) =
        stbl_cli::assets::discover_assets(&site_assets_root).expect("discover assets");
    let (image_plan, image_lookup) =
        stbl_cli::media::discover_images(&project).expect("discover images");
    project.image_alpha = image_plan.alpha.clone();
    project.image_variants = stbl_core::media::build_image_variant_index(
        &image_plan,
        &project.config.media.images.widths,
        project.config.media.images.format_mode,
    );
    let (video_plan, video_lookup) =
        stbl_cli::media::discover_videos(&project).expect("discover videos");
    project.video_variants = stbl_core::media::build_video_variant_index(
        &video_plan,
        &project.config.media.video.heights,
    );
    let asset_manifest =
        stbl_core::assets::build_asset_manifest(&asset_index, project.config.assets.cache_busting);
    let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);

    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    stbl_cli::exec::execute_plan(
        &project,
        &plan,
        &out_dir,
        &asset_lookup,
        &image_lookup,
        &video_lookup,
        &asset_manifest,
        None,
        None,
        false,
    )
    .expect("execute plan");

    let videos = [
        "5786143-hd_1920_1080_30fps.mp4",
        "13029714_1080_1920_60fps.mp4",
    ];

    for video in videos {
        let original = out_dir.join("video").join(video);
        let scale_360 = out_dir.join("video/_scale_360").join(video);
        let scale_480 = out_dir.join("video/_scale_480").join(video);
        let poster = out_dir
            .join("video/_poster_")
            .join(video.replace(".mp4", ".jpg"));

        assert!(original.exists(), "missing original {video}");
        assert!(scale_360.exists(), "missing scale_360 {video}");
        assert!(scale_480.exists(), "missing scale_480 {video}");
        assert!(poster.exists(), "missing poster {video}");

        let bytes = fs::read(&poster).expect("read poster");
        assert!(bytes.len() > 2, "poster too small for {video}");
        assert_eq!(bytes[0], 0xFF, "poster not jpeg for {video}");
        assert_eq!(bytes[1], 0xD8, "poster not jpeg for {video}");
    }
}
