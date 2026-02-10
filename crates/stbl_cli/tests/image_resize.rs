use std::fs;
use std::path::PathBuf;

use image::{ImageBuffer, Rgb};
use stbl_cli::media::{VideoSourceLookup, discover_images};
use stbl_core::assets::AssetManifest;
use stbl_core::config::load_site_config;
use stbl_core::media::{ImageRef, MediaPath, MediaRef, plan_image_tasks};
use stbl_core::model::{Page, Project, SiteContent};
use tempfile::TempDir;

#[test]
fn image_resize_generates_variants() {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().to_path_buf();
    let images_dir = root.join("images");
    fs::create_dir_all(&images_dir).expect("mkdir images");
    let src_path = images_dir.join("test.jpg");
    write_test_image(&src_path, 800, 400);

    let config_path = root.join("stbl.yaml");
    fs::write(
        &config_path,
        "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nmedia:\n  images:\n    widths: [200, 500]\n    quality: 90\n",
    )
    .expect("write config");
    let config = load_site_config(&config_path).expect("load config");
    let page = Page {
        id: stbl_core::model::DocId(blake3::hash(b"page")),
        source_path: "articles/page.md".to_string(),
        header: stbl_core::header::Header::default(),
        body_markdown: String::new(),
        banner_name: None,
        media_refs: vec![MediaRef::Image(ImageRef {
            path: MediaPath {
                raw: "images/test.jpg".to_string(),
            },
            alt: "Alt".to_string(),
            attrs: Vec::new(),
            maxw: None,
            maxh: None,
            has_args: false,
        })],
        url_path: "page".to_string(),
        content_hash: blake3::hash(b"page"),
    };
    let mut project = Project {
        root: root.clone(),
        config: config.clone(),
        content: SiteContent {
            pages: vec![page],
            series: Vec::new(),
            diagnostics: Vec::new(),
            write_back: Default::default(),
        },
        image_alpha: std::collections::BTreeMap::new(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    };

    let (image_plan, image_lookup) = discover_images(&project).expect("discover images");
    project.image_alpha = image_plan.alpha.clone();
    project.image_variants = stbl_core::media::build_image_variant_index(
        &image_plan,
        &project.config.media.images.widths,
        project.config.media.images.format_mode,
    );
    let tasks = plan_image_tasks(
        &image_plan,
        &config.media.images.widths,
        config.media.images.quality,
        config.media.images.format_mode,
    );
    let plan = stbl_core::model::BuildPlan {
        tasks,
        edges: Vec::new(),
    };
    let out_dir = root.join("out");
    stbl_cli::exec::execute_plan(
        &project,
        &plan,
        &out_dir,
        &stbl_core::assets::AssetIndex::default(),
        &stbl_cli::assets::AssetSourceLookup::default(),
        &image_lookup,
        &VideoSourceLookup::default(),
        &AssetManifest::default(),
        None,
        None,
        false,
    )
    .expect("execute plan");

    let original = out_dir.join("images/test.jpg");
    let scaled_200_avif = out_dir.join("images/_scale_200/test.avif");
    let scaled_200_webp = out_dir.join("images/_scale_200/test.webp");
    let scaled_200 = out_dir.join("images/_scale_200/test.jpg");
    let scaled_500_avif = out_dir.join("images/_scale_500/test.avif");
    let scaled_500_webp = out_dir.join("images/_scale_500/test.webp");
    let scaled_500 = out_dir.join("images/_scale_500/test.jpg");
    assert!(original.exists());
    assert!(scaled_200_avif.exists());
    assert!(scaled_200_webp.exists());
    assert!(scaled_200.exists());
    assert!(scaled_500_avif.exists());
    assert!(scaled_500_webp.exists());
    assert!(scaled_500.exists());

    let img_200 = image::open(&scaled_200).expect("open 200");
    let img_500 = image::open(&scaled_500).expect("open 500");
    assert_eq!(img_200.width(), 200);
    assert_eq!(img_500.width(), 500);
    assert_eq!(img_200.height(), 100);
    assert_eq!(img_500.height(), 250);
}

fn write_test_image(path: &PathBuf, width: u32, height: u32) {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgb([10, 20, 30]));
    img.save(path).expect("save image");
}
