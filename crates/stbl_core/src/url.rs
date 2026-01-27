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
        let logical = logical_key.trim_matches('/');
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
