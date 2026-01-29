use stbl_core::model::{SiteConfig, SiteMeta, UrlStyle};
use stbl_core::url::{Redirect, UrlMapper, UrlMapping};

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
