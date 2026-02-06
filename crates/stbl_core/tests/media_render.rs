use stbl_core::render::{RenderOptions, render_markdown_to_html_with_media};
use stbl_core::model::ImageFormatMode;

#[test]
fn managed_video_renders_html5_video_markup() {
    let md = "![Landscape sample](video/5786143-hd_1920_1080_30fps.mp4;p360)\n\n![Portrait sample](video/13029714_1080_1920_60fps.mp4)";
    let heights = vec![360, 480, 720];
    let widths = vec![360, 720];
    let options = RenderOptions {
        rel_prefix: "",
        video_heights: &heights,
        image_widths: &widths,
        max_body_width: "72rem",
        desktop_min: "768px",
        wide_min: "1400px",
        image_format_mode: ImageFormatMode::Normal,
        image_alpha: None,
        image_variants: None,
        video_variants: None,
    };
    let html = render_markdown_to_html_with_media(md, &options);

    assert!(html.contains("<video"));
    assert!(html.contains("controls"));
    assert!(html.contains("preload=\"metadata\""));
    assert!(html.contains("data-stbl-video"));
    assert!(html.contains("data-prefer=\"p360\""));
    assert!(html.contains("data-prefer=\"p720\""));
    assert!(html.contains("aria-label=\"Landscape sample\""));
    assert!(html.contains("poster=\"artifacts/video/_poster_/5786143-hd_1920_1080_30fps.jpg\""));
    assert!(html.contains("href=\"artifacts/video/5786143-hd_1920_1080_30fps.mp4\""));

    let first = html
        .find("artifacts/video/_scale_360/5786143-hd_1920_1080_30fps.mp4")
        .expect("preferred source missing");
    let second = html
        .find("artifacts/video/_scale_480/5786143-hd_1920_1080_30fps.mp4")
        .expect("secondary source missing");
    assert!(first < second, "preferred source should be first");
}

#[test]
fn non_managed_video_links_render_as_images() {
    let md = "![External](https://example.com/x.mp4)";
    let heights = vec![360, 480];
    let widths = vec![360, 720];
    let options = RenderOptions {
        rel_prefix: "",
        video_heights: &heights,
        image_widths: &widths,
        max_body_width: "72rem",
        desktop_min: "768px",
        wide_min: "1400px",
        image_format_mode: ImageFormatMode::Normal,
        image_alpha: None,
        image_variants: None,
        video_variants: None,
    };
    let html = render_markdown_to_html_with_media(md, &options);
    assert!(html.contains("<img"));
    assert!(html.contains("https://example.com/x.mp4"));
    assert!(!html.contains("data-stbl-video"));
}
