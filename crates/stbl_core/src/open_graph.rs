use anyhow::{Result, bail};
use serde::Serialize;

use crate::model::{Page, Project};
use crate::url::{UrlMapper, logical_key_from_source_path};

#[derive(Debug, Serialize)]
pub struct OpenGraphView {
    pub kind: &'static str,
    pub image: Option<String>,
}

/// Normalize a managed image path or an external HTTP(S) image URL without IO.
pub fn normalize_social_image(value: &str) -> Result<String> {
    let value = value.trim();
    if let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    {
        if rest.is_empty() || rest.starts_with('/') || value.chars().any(char::is_whitespace) {
            bail!("expected an HTTP(S) image URL with a host");
        }
        return Ok(value.to_string());
    }
    if value.is_empty()
        || value.contains(['\\', ':', '?', '#'])
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || value.contains('/') && !value.starts_with("images/")
    {
        bail!("expected a filename, an images/ path, or an HTTP(S) image URL");
    }
    if value.starts_with("images/") {
        Ok(value.to_string())
    } else {
        Ok(format!("images/{value}"))
    }
}

pub fn frontpage_banner(project: &Project) -> Option<&str> {
    let enabled = project
        .config
        .seo
        .as_ref()?
        .open_graph
        .as_ref()?
        .use_frontpage_banner;
    if !enabled {
        return None;
    }
    project
        .content
        .pages
        .iter()
        .find(|page| {
            page.header.is_published && logical_key_from_source_path(&page.source_path) == "index"
        })?
        .banner_name
        .as_deref()
}

pub fn build_open_graph(project: &Project, page: Option<&Page>, article: bool) -> OpenGraphView {
    let default_image = project
        .config
        .seo
        .as_ref()
        .and_then(|seo| seo.open_graph.as_ref())
        .and_then(|config| config.default_image.as_deref());
    let image = page
        .and_then(|page| page.header.social_image.as_deref())
        .or_else(|| page.and_then(|page| page.banner_name.as_deref()))
        .or(default_image)
        .or_else(|| frontpage_banner(project))
        .and_then(|value| normalize_social_image(value).ok())
        .map(|path| {
            if path.starts_with("https://") || path.starts_with("http://") {
                path
            } else {
                UrlMapper::absolute_href(&project.config.site.base_url, &path)
            }
        });
    OpenGraphView {
        kind: if article { "article" } else { "website" },
        image,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_social_image;

    #[test]
    fn social_image_paths_and_urls_are_normalized() {
        for (input, expected) in [
            (" card.jpg ", "images/card.jpg"),
            ("images/social/card.png", "images/social/card.png"),
            (
                "https://cdn.example.com/card.jpg",
                "https://cdn.example.com/card.jpg",
            ),
        ] {
            assert_eq!(normalize_social_image(input).unwrap(), expected);
        }
        for input in [
            "",
            "../card.jpg",
            "images/../card.jpg",
            "/images/card.jpg",
            "images//card.jpg",
            "assets/card.jpg",
            "data:image/png;base64,abc",
            "https://",
            "images\\card.jpg",
        ] {
            assert!(normalize_social_image(input).is_err(), "{input}");
        }
    }
}
