use std::collections::BTreeSet;
use std::fs;

use image::{ImageBuffer, Rgb};
use stbl_cli::media::discover_images;
use stbl_core::config::load_site_config;
use stbl_core::header::Header;
use stbl_core::media::{collect_media_refs, plan_image_tasks};
use stbl_core::model::{DocId, Page, Project, SiteContent, TaskKind};
use tempfile::TempDir;

#[test]
fn hero_banner_background_and_repeated_video_posters_share_scaling_tasks() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("images")).unwrap();
    ImageBuffer::from_pixel(800, 450, Rgb([20u8, 40, 60]))
        .save(temp.path().join("images/shared.png"))
        .unwrap();
    fs::write(temp.path().join("stbl.yaml"), "site:\n  id: demo\n  title: Demo\n  base_url: https://example.com/\n  language: en\nmedia:\n  images:\n    widths: [128, 128, 360, 720, 720, 1440]\ntheme:\n  wide_background:\n    image: images/shared.png\n").unwrap();
    let config = load_site_config(&temp.path().join("stbl.yaml")).unwrap();
    let markdown = "![Hero](images/./shared.png;maxw=60%)\n\n![Introduction](video/intro.mp4;poster=images/shared.png)\n\n![Other video](video/other.mp4;poster=images/./shared.png)";
    let page = Page {
        id: DocId(blake3::hash(b"page")),
        source_path: "articles/index.md".into(),
        header: Header::default(),
        body_markdown: markdown.into(),
        banner_name: Some("shared.png".into()),
        media_refs: collect_media_refs(markdown),
        url_path: "index".into(),
        content_hash: blake3::hash(b"page"),
    };
    let mut second = page.clone();
    second.id = DocId(blake3::hash(b"second"));
    second.source_path = "articles/other.md".into();
    let mut project = Project {
        root: temp.path().into(),
        config,
        content: SiteContent {
            pages: vec![page, second],
            ..SiteContent::default()
        },
        image_alpha: Default::default(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    };
    let (images, _) = discover_images(&project).unwrap();
    assert_eq!(images.sources.len(), 1);
    let tasks = plan_image_tasks(
        &images,
        &project.config.media.images.widths,
        90,
        project.config.media.images.format_mode,
    );
    let ids = tasks.iter().map(|task| &task.id).collect::<BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        tasks.len(),
        "every image/width/format appears once"
    );
    let outputs = tasks
        .iter()
        .flat_map(|task| &task.outputs)
        .map(|output| &output.path)
        .collect::<BTreeSet<_>>();
    assert_eq!(outputs.len(), tasks.len());
    let expected_formats =
        stbl_core::media::image_output_formats(project.config.media.images.format_mode, false)
            .len();
    assert_eq!(tasks.len(), 1 + 3 * expected_formats);
    assert!(
        tasks
            .iter()
            .filter_map(|task| match task.kind {
                TaskKind::ResizeImage { width, .. } => Some(width),
                _ => None,
            })
            .all(|width| width <= 800)
    );
    // A poster used nowhere else must still enter the same image discovery path.
    project.config.theme.wide_background.image = None;
    project.content.pages.truncate(1);
    project.content.pages[0].banner_name = None;
    project.content.pages[0].media_refs =
        collect_media_refs("![Introduction](video/intro.mp4;poster=images/shared.png)");
    let (posters_only, _) = discover_images(&project).unwrap();
    assert_eq!(posters_only.sources.len(), 1);
    let before = stbl_core::plan::build_plan(
        &project,
        &stbl_core::assets::AssetIndex::default(),
        &stbl_core::media::ImagePlanInput::default(),
        &stbl_core::media::VideoPlanInput::default(),
    );
    project.image_alpha = posters_only.alpha.clone();
    project.image_variants = stbl_core::media::build_image_variant_index(
        &posters_only,
        &project.config.media.images.widths,
        project.config.media.images.format_mode,
    );
    let after = stbl_core::plan::build_plan(
        &project,
        &stbl_core::assets::AssetIndex::default(),
        &posters_only,
        &stbl_core::media::VideoPlanInput::default(),
    );
    let render_fingerprint = |plan: &stbl_core::model::BuildPlan| {
        plan.tasks
            .iter()
            .find(|task| matches!(task.kind, TaskKind::RenderPage { .. }))
            .unwrap()
            .inputs_fingerprint
    };
    assert_ne!(
        render_fingerprint(&before),
        render_fingerprint(&after),
        "available poster variants invalidate cached page HTML"
    );
    // Missing and unreadable optional posters fall back without aborting discovery.
    project.content.pages[0].media_refs =
        collect_media_refs("![Introduction](video/intro.mp4;poster=images/missing.png)");
    assert!(discover_images(&project).unwrap().0.sources.is_empty());
    fs::write(temp.path().join("images/broken.png"), b"not an image").unwrap();
    project.content.pages[0].media_refs =
        collect_media_refs("![Introduction](video/intro.mp4;poster=images/broken.png)");
    assert!(discover_images(&project).unwrap().0.sources.is_empty());
}
