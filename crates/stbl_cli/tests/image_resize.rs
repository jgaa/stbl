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

#[test]
fn landing_card_and_section_images_are_copied_scaled_and_rendered_from_nested_folders() {
    use std::process::Command;

    let temp = TempDir::new().expect("tempdir");
    let root = temp.path();
    fs::create_dir_all(root.join("articles")).expect("articles");
    fs::create_dir_all(root.join("images/cards")).expect("cards");
    fs::create_dir_all(root.join("images/icons")).expect("icons");
    write_test_image(&root.join("images/cards/photo.png"), 800, 400);
    fs::write(root.join("images/icons/check.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M0 0h16v16H0z"/></svg>"#
    ).expect("write SVG");
    fs::copy(
        root.join("images/icons/check.svg"),
        root.join("images/icons/blocks.svg"),
    )
    .expect("section SVG");
    write_test_image(&root.join("images/cards/section.png"), 800, 400);
    fs::write(root.join("stbl.yaml"),
        "site:\n  id: landing-card-images\n  title: Cards\n  base_url: https://example.com/\n  language: en\ntheme:\n  variant: product\nmedia:\n  images:\n    widths: [200, 500, 1000]\n    quality: 90\n"
    ).expect("config");
    fs::write(root.join("articles/index.md"),
        "---\ntitle: Cards\nlayout: landing\n---\n\n::: features\n::: card icon=images/cards/photo.png\n### Photo\n\nCard text.\n:::\n::: card icon=images/icons/check.svg\n### Icon\n:::\n:::\n\n::: section tone=subtle icon=\"images/icons/blocks.svg\" image-align=left\n## Privacy\n\n![Privacy illustration](images/cards/photo.png;maxw=60%)\n\nPrivate by design.\n\n::: features\n::: card\n### No tracking\nText\n:::\n:::\n:::\n\n::: section icon=images/cards/section.png image-align=center\n## Centered illustration\n\nBefore image.\n\n![Centered](images/cards/photo.png;maxw=50%)\n\nAfter image.\n:::\n"
    ).expect("content");
    let out = root.join("out");
    // Build twice to exercise both execution and incremental reuse.
    for _ in 0..2 {
        let result = Command::new(env!("CARGO_BIN_EXE_stbl_cli"))
            .args([
                "--no-writeback",
                "build",
                "--fast-images",
                "--precompress",
                "false",
                "-s",
            ])
            .arg(root)
            .arg("--out")
            .arg(&out)
            .output()
            .expect("build cards");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(
        fs::read(out.join("images/cards/photo.png")).unwrap(),
        fs::read(root.join("images/cards/photo.png")).unwrap()
    );
    assert!(out.join("images/icons/check.svg").exists());
    assert!(out.join("images/icons/blocks.svg").exists());
    assert!(out.join("images/cards/section.png").exists());
    assert!(out.join("images/_scale_200/cards/section.webp").exists());
    for width in [200, 500] {
        for extension in ["jpg", "webp"] {
            let path = out.join(format!("images/_scale_{width}/cards/photo.{extension}"));
            assert!(path.is_file(), "{}", path.display());
            let image = image::open(path).expect("scaled image");
            assert_eq!((image.width(), image.height()), (width, width / 2));
        }
    }
    assert!(!out.join("images/_scale_1000/cards/photo.webp").exists());
    let config = load_site_config(&root.join("stbl.yaml")).expect("config");
    let mapper = stbl_core::url::UrlMapper::new(&config);
    let html = fs::read_to_string(out.join(mapper.map("index").primary_output)).expect("HTML");
    assert!(html.contains("class=\"landing-section__icon\"><picture><img"));
    assert!(html.contains("images/icons/blocks.svg"));
    assert!(html.contains("class=\"landing-section__image\"><picture>"));
    assert!(html.contains("images/_scale_200/cards/section.webp 200w"));
    assert!(html.contains("class=\"landing-card__image\"><picture>"));
    assert!(html.contains("images/_scale_200/cards/photo.webp 200w"));
    assert!(html.contains("images/_scale_500/cards/photo.jpg 500w"));
    assert!(html.contains(
        "sizes=\"(min-width: 768px) calc((min(100vw, 72rem) - 4rem) / 3), calc(100vw - 4rem)\""
    ));
    assert!(html.contains("alt=\"\""));
    assert!(html.contains("loading=\"lazy\""));
    assert!(html.contains("class=\"landing-card__icon\"><picture><img"));
    assert!(
        html.contains("class=\"landing-section__intro landing-hero--split landing-hero--reverse\"")
    );
    assert!(html.contains("alt=\"Privacy illustration\""));
    assert!(html.contains("--hero-image-weight: 60fr; --hero-text-weight: 40fr"));
    assert!(!html.contains("--media-maxw: 60%"));
    assert!(html.contains("</div></div><section class=\"landing-features\">"));
    let centered = html
        .split("landing-section--image-center")
        .nth(1)
        .expect("centered section");
    assert!(!centered.contains("landing-hero--split"));
    assert!(centered.contains("--media-maxw: 50%"));
    assert!(centered.find("Before image.").unwrap() < centered.find("alt=\"Centered\"").unwrap());
    assert!(centered.find("alt=\"Centered\"").unwrap() < centered.find("After image.").unwrap());
}

fn write_test_image(path: &PathBuf, width: u32, height: u32) {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgb([10, 20, 30]));
    img.save(path).expect("save image");
}
