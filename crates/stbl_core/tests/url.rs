use stbl_core::model::{
    AssetsConfig, SiteConfig, SiteMeta, ThemeBreakpoints, ThemeConfig, ThemeColorOverrides,
    ThemeNavOverrides, ThemeWideBackgroundOverrides, UrlStyle, ImageFormatMode,
};
use stbl_core::url::{Redirect, UrlMapper, UrlMapping, logical_key_from_source_path};

fn base_config(style: UrlStyle) -> SiteConfig {
    SiteConfig {
        site: SiteMeta {
            id: "site".to_string(),
            title: "Site".to_string(),
            abstract_text: None,
            copyright: None,
            base_url: "https://example.com/".to_string(),
            language: "en".to_string(),
            timezone: None,
            url_style: style,
        },
        banner: None,
        menu: Vec::new(),
        nav: Vec::new(),
        theme: ThemeConfig {
            variant: "default".to_string(),
            max_body_width: "72rem".to_string(),
            breakpoints: ThemeBreakpoints {
                desktop_min: "768px".to_string(),
                wide_min: "1400px".to_string(),
            },
            colors: ThemeColorOverrides::default(),
            nav: ThemeNavOverrides::default(),
            wide_background: ThemeWideBackgroundOverrides::default(),
        },
        assets: AssetsConfig {
            cache_busting: false,
        },
        media: stbl_core::model::MediaConfig {
            images: stbl_core::model::ImageConfig {
                widths: vec![
                    94, 128, 248, 360, 480, 640, 720, 950, 1280, 1440, 1680, 1920, 2560,
                ],
                quality: 90,
                format_mode: ImageFormatMode::Normal,
            },
            video: stbl_core::model::VideoConfig {
                heights: vec![360, 480, 720, 1080],
                poster_time_sec: 1,
            },
        },
        footer: stbl_core::model::FooterConfig { show_stbl: true },
        people: None,
        blog: None,
        system: None,
        publish: None,
        rss: None,
        seo: None,
        comments: None,
        chroma: None,
        plyr: None,
    }
}

#[test]
fn maps_html_style_nested_key() {
    let config = base_config(UrlStyle::Html);
    let mapper = UrlMapper::new(&config);
    let mapping = mapper.map("a/b/c");
    assert_eq!(
        mapping,
        UrlMapping {
            href: "a/b/c.html".to_string(),
            primary_output: "a/b/c.html".into(),
            fallback: None,
        }
    );
}

#[test]
fn strips_html_suffix_for_html_style() {
    let config = base_config(UrlStyle::Html);
    let mapper = UrlMapper::new(&config);
    let mapping = mapper.map("download.html");
    assert_eq!(
        mapping,
        UrlMapping {
            href: "download.html".to_string(),
            primary_output: "download.html".into(),
            fallback: None,
        }
    );
}

#[test]
fn maps_dot_to_index_for_html_style() {
    let config = base_config(UrlStyle::Html);
    let mapper = UrlMapper::new(&config);
    let mapping = mapper.map("./");
    assert_eq!(
        mapping,
        UrlMapping {
            href: "index.html".to_string(),
            primary_output: "index.html".into(),
            fallback: None,
        }
    );
}

#[test]
fn maps_pretty_style_nested_key() {
    let config = base_config(UrlStyle::Pretty);
    let mapper = UrlMapper::new(&config);
    let mapping = mapper.map("a/b/c");
    assert_eq!(
        mapping,
        UrlMapping {
            href: "a/b/c/".to_string(),
            primary_output: "a/b/c/index.html".into(),
            fallback: None,
        }
    );
}

#[test]
fn strips_html_suffix_for_pretty_style() {
    let config = base_config(UrlStyle::Pretty);
    let mapper = UrlMapper::new(&config);
    let mapping = mapper.map("download.html");
    assert_eq!(
        mapping,
        UrlMapping {
            href: "download/".to_string(),
            primary_output: "download/index.html".into(),
            fallback: None,
        }
    );
}

#[test]
fn maps_pretty_with_fallback_style_nested_key() {
    let config = base_config(UrlStyle::PrettyWithFallback);
    let mapper = UrlMapper::new(&config);
    let mapping = mapper.map("a/b/c");
    assert_eq!(
        mapping,
        UrlMapping {
            href: "a/b/c/".to_string(),
            primary_output: "a/b/c/index.html".into(),
            fallback: Some(Redirect {
                from: "a/b/c.html".into(),
                to_href: "a/b/c/".to_string(),
            }),
        }
    );
}

#[test]
fn logical_key_strips_leading_hidden_dirs() {
    assert_eq!(
        logical_key_from_source_path("articles/_drafts/post.md"),
        "post"
    );
    assert_eq!(
        logical_key_from_source_path("articles/_hidden/real/post.md"),
        "real/post"
    );
    assert_eq!(
        logical_key_from_source_path("articles/real/_hidden/post.md"),
        "real/_hidden/post"
    );
    assert_eq!(
        logical_key_from_source_path("articles/_one/_two/post.md"),
        "post"
    );
}
