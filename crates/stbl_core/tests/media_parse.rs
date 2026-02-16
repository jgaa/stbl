use stbl_core::media::{
    ImageAttr, MediaRef, VideoAttr, collect_media_refs, collect_media_refs_with_errors,
    parse_media_destination,
};

#[test]
fn parse_image_with_attributes() {
    let media = parse_media_destination("images/foo.jpg;banner;70%", "Alt").expect("media");
    match media {
        MediaRef::Image(image) => {
            assert_eq!(image.path.raw, "images/foo.jpg");
            assert_eq!(image.alt, "Alt");
            assert!(image.attrs.contains(&ImageAttr::Banner));
            assert!(image.attrs.contains(&ImageAttr::WidthPercent(70)));
        }
        _ => panic!("expected image"),
    }
}

#[test]
fn parse_image_plain() {
    let media = parse_media_destination("images/foo.jpg", "Alt").expect("media");
    match media {
        MediaRef::Image(image) => {
            assert_eq!(image.path.raw, "images/foo.jpg");
            assert!(image.attrs.is_empty());
            assert!(image.maxw.is_none());
            assert!(image.maxh.is_none());
        }
        _ => panic!("expected image"),
    }
}

#[test]
fn parse_video_default_prefer() {
    let media = parse_media_destination("video/v.mp4", "Alt").expect("media");
    match media {
        MediaRef::Video(video) => {
            assert_eq!(video.path.raw, "video/v.mp4");
            assert_eq!(video.prefer_p, 720);
            assert!(video.maxw.is_none());
            assert!(video.maxh.is_none());
        }
        _ => panic!("expected video"),
    }
}

#[test]
fn parse_video_explicit_prefer() {
    let media = parse_media_destination("video/v.mp4;p360", "Alt").expect("media");
    match media {
        MediaRef::Video(video) => {
            assert_eq!(video.prefer_p, 360);
            assert!(video.attrs.contains(&VideoAttr::PreferP(360)));
        }
        _ => panic!("expected video"),
    }
}

#[test]
fn ignore_non_managed_paths() {
    let media = parse_media_destination("https://example.com/x.png;70%", "Alt");
    assert!(media.is_none());
}

#[test]
fn unknown_attrs_preserved() {
    let media = parse_media_destination("images/foo.jpg;left;shadow", "Alt").expect("media");
    match media {
        MediaRef::Image(image) => {
            assert!(
                image
                    .attrs
                    .contains(&ImageAttr::Unknown("left".to_string()))
            );
            assert!(
                image
                    .attrs
                    .contains(&ImageAttr::Unknown("shadow".to_string()))
            );
        }
        _ => panic!("expected image"),
    }
}

#[test]
fn parse_media_with_max_constraints() {
    let media =
        parse_media_destination("images/foo.jpg;maxw=50%;maxh=12.5rem", "Alt").expect("media");
    match media {
        MediaRef::Image(image) => {
            assert_eq!(image.maxw.as_deref(), Some("50%"));
            assert_eq!(image.maxh.as_deref(), Some("12.5rem"));
        }
        _ => panic!("expected image"),
    }
    let media =
        parse_media_destination("video/v.mp4;maxw=640px;maxh=37.5vh", "Alt").expect("media");
    match media {
        MediaRef::Video(video) => {
            assert_eq!(video.maxw.as_deref(), Some("640px"));
            assert_eq!(video.maxh.as_deref(), Some("37.5vh"));
        }
        _ => panic!("expected video"),
    }
}

#[test]
fn invalid_max_constraint_reports_error() {
    let (refs, errors) = collect_media_refs_with_errors("![A](images/a.jpg;maxw=900pt)");
    assert!(refs.is_empty());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("invalid maxw value"));
}

#[test]
fn collect_media_refs_from_markdown() {
    let md = "![A](images/a.jpg;70%)\n![B](video/b.mp4;p480)\n![C](https://example.com/c.png)";
    let refs = collect_media_refs(md);
    assert_eq!(refs.len(), 2);
    match &refs[0] {
        MediaRef::Image(image) => {
            assert!(image.attrs.contains(&ImageAttr::WidthPercent(70)));
        }
        _ => panic!("expected image"),
    }
    match &refs[1] {
        MediaRef::Video(video) => {
            assert_eq!(video.prefer_p, 480);
        }
        _ => panic!("expected video"),
    }
}
