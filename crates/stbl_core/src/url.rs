use crate::model::{SiteConfig, UrlStyle};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlMapping {
    pub href: String,
    pub primary_output: PathBuf,
    pub fallback: Option<Redirect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub from: PathBuf,
    pub to_href: String,
}

#[derive(Debug, Clone, Copy)]
pub struct UrlMapper {
    style: UrlStyle,
}

impl UrlMapper {
    pub fn new(cfg: &SiteConfig) -> Self {
        Self {
            style: cfg.site.url_style,
        }
    }

    pub fn map(&self, logical_key: &str) -> UrlMapping {
        let logical = normalize_logical_key(logical_key);
        match self.style {
            UrlStyle::Html => UrlMapping {
                href: format!("{logical}.html"),
                primary_output: PathBuf::from(format!("{logical}.html")),
                fallback: None,
            },
            UrlStyle::Pretty => UrlMapping {
                href: format!("{logical}/"),
                primary_output: PathBuf::from(format!("{logical}/index.html")),
                fallback: None,
            },
            UrlStyle::PrettyWithFallback => UrlMapping {
                href: format!("{logical}/"),
                primary_output: PathBuf::from(format!("{logical}/index.html")),
                fallback: Some(Redirect {
                    from: PathBuf::from(format!("{logical}.html")),
                    to_href: format!("{logical}/"),
                }),
            },
        }
    }
}

pub fn map_series_index(logical_key: &str) -> UrlMapping {
    let logical = normalize_logical_key(logical_key);
    UrlMapping {
        href: format!("{logical}/"),
        primary_output: PathBuf::from(format!("{logical}/index.html")),
        fallback: None,
    }
}

fn normalize_logical_key(logical_key: &str) -> &str {
    let trimmed = logical_key.trim_matches('/');
    let normalized = trimmed.strip_suffix(".html").unwrap_or(trimmed);
    if normalized.is_empty() || normalized == "." {
        "index"
    } else {
        normalized
    }
}

pub fn logical_key_from_source_path(source_path: &str) -> String {
    let normalized = source_path.replace('\\', "/");
    let without_prefix = normalized
        .strip_prefix("articles/")
        .unwrap_or(normalized.as_str());
    logical_key_without_hidden_dirs(without_prefix)
}

fn without_extension(path: &str) -> String {
    let path = std::path::Path::new(path);
    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
        match path.parent().and_then(|value| value.to_str()) {
            Some(parent) if !parent.is_empty() => format!("{parent}/{stem}"),
            _ => stem.to_string(),
        }
    } else {
        path.to_string_lossy().to_string()
    }
}

fn logical_key_without_hidden_dirs(path: &str) -> String {
    let path = std::path::Path::new(path);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if stem.is_empty() {
        return without_extension(path.to_string_lossy().as_ref());
    }
    let parent = path.parent().and_then(|value| value.to_str()).unwrap_or("");
    let mut parts: Vec<&str> = parent.split('/').filter(|part| !part.is_empty()).collect();
    while matches!(parts.first(), Some(part) if part.starts_with('_')) {
        parts.remove(0);
    }
    if parts.is_empty() {
        stem.to_string()
    } else {
        format!("{}/{}", parts.join("/"), stem)
    }
}
