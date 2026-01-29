use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::model::{
    BannerConfig, MenuItem, PeopleConfig, PersonEntry, PublishConfig, RssConfig, SeoConfig,
    SiteConfig, SiteMeta, SystemConfig, UrlStyle,
};

#[derive(Debug, Deserialize)]
struct SiteConfigRaw {
    site: SiteMetaRaw,
    banner: Option<BannerConfig>,
    #[serde(default)]
    menu: Vec<MenuItem>,
    people: Option<PeopleConfigRaw>,
    blog: Option<BlogConfigRaw>,
    system: Option<SystemConfig>,
    publish: Option<PublishConfig>,
    rss: Option<RssConfigRaw>,
    seo: Option<SeoConfig>,
    comments: Option<serde_yaml::Value>,
    chroma: Option<serde_yaml::Value>,
    plyr: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct SiteMetaRaw {
    id: Option<String>,
    title: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    base_url: Option<String>,
    language: Option<String>,
    timezone: Option<String>,
    url_style: Option<UrlStyle>,
}

#[derive(Debug, Deserialize)]
struct PeopleConfigRaw {
    default: Option<String>,
    entries: Option<BTreeMap<String, PersonEntry>>,
}

#[derive(Debug, Deserialize)]
struct BlogConfigRaw {
    #[serde(rename = "abstract")]
    abstract_cfg: Option<BlogAbstractConfigRaw>,
    page_size: Option<usize>,
    pagination: Option<BlogPaginationConfigRaw>,
    series: Option<BlogSeriesConfigRaw>,
}

#[derive(Debug, Deserialize)]
struct BlogAbstractConfigRaw {
    enabled: Option<bool>,
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct BlogPaginationConfigRaw {
    enabled: Option<bool>,
    page_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct BlogSeriesConfigRaw {
    latest_parts: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RssConfigRaw {
    enabled: Option<bool>,
    max_items: Option<usize>,
    ttl_days: Option<i64>,
}

pub fn load_site_config(path: &Path) -> Result<SiteConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let parsed: SiteConfigRaw = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse YAML config {}", path.display()))?;

    let site = SiteMeta {
        id: required_string(parsed.site.id, "site.id")?,
        title: required_string(parsed.site.title, "site.title")?,
        abstract_text: parsed.site.abstract_text,
        base_url: required_string(parsed.site.base_url, "site.base_url")?,
        language: required_string(parsed.site.language, "site.language")?,
        timezone: parsed.site.timezone,
        url_style: parsed.site.url_style.unwrap_or_default(),
    };

    let people = match parsed.people {
        None => None,
        Some(people_raw) => {
            let entries = match people_raw.entries {
                Some(entries) => entries,
                None => bail!("missing required field: people.entries"),
            };
            if entries.is_empty() {
                bail!("people.entries must not be empty");
            }
            let default = match people_raw.default {
                Some(value) if !value.trim().is_empty() => {
                    if !entries.contains_key(&value) {
                        bail!("people.default '{}' is missing from people.entries", value);
                    }
                    value
                }
                _ => entries
                    .keys()
                    .next()
                    .cloned()
                    .expect("entries should not be empty"),
            };
            Some(PeopleConfig { default, entries })
        }
    };

    let blog = match parsed.blog {
        None => None,
        Some(blog_raw) => {
            let (abstract_enabled, abstract_max_chars) = match blog_raw.abstract_cfg {
                Some(abstract_raw) => (
                    abstract_raw.enabled.unwrap_or(true),
                    abstract_raw.max_chars.unwrap_or(200),
                ),
                None => (true, 200),
            };
            if abstract_enabled && abstract_max_chars == 0 {
                bail!("blog.abstract.max_chars must be > 0 when abstract is enabled");
            }
            let (enabled, page_size) = match blog_raw.pagination {
                Some(pagination_raw) => (
                    pagination_raw.enabled.unwrap_or(false),
                    pagination_raw.page_size.unwrap_or(10),
                ),
                None => (
                    blog_raw.page_size.is_some(),
                    blog_raw.page_size.unwrap_or(10),
                ),
            };
            if enabled && page_size == 0 {
                bail!("blog.pagination.page_size must be > 0 when pagination is enabled");
            }
            Some(crate::model::BlogConfig {
                abstract_cfg: crate::model::BlogAbstractConfig {
                    enabled: abstract_enabled,
                    max_chars: abstract_max_chars,
                },
                pagination: crate::model::BlogPaginationConfig { enabled, page_size },
                series: crate::model::BlogSeriesConfig {
                    latest_parts: blog_raw
                        .series
                        .and_then(|series| series.latest_parts)
                        .unwrap_or(3),
                },
            })
        }
    };

    let rss = match parsed.rss {
        None => None,
        Some(rss_raw) => {
            let enabled = rss_raw.enabled.unwrap_or(false);
            if enabled && rss_raw.max_items.is_none() {
                bail!("rss.max_items required when rss.enabled is true");
            }
            let ttl_days = match rss_raw.ttl_days {
                None => None,
                Some(value) => {
                    if value <= 0 {
                        bail!("rss.ttl_days must be > 0");
                    }
                    Some(u32::try_from(value).context("rss.ttl_days out of range")?)
                }
            };
            Some(RssConfig {
                enabled,
                max_items: rss_raw.max_items,
                ttl_days,
            })
        }
    };

    Ok(SiteConfig {
        site,
        banner: parsed.banner,
        menu: parsed.menu,
        people,
        blog,
        system: parsed.system,
        publish: parsed.publish,
        rss,
        seo: parsed.seo,
        comments: parsed.comments,
        chroma: parsed.chroma,
        plyr: parsed.plyr,
    })
}

fn required_string(value: Option<String>, field: &str) -> Result<String> {
    match value {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => bail!("missing required field: {}", field),
    }
}

#[cfg(test)]
mod tests {
    use super::load_site_config;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn write_temp(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("stbl-config-{}.yaml", Uuid::new_v4()));
        fs::write(&path, contents).expect("write temp config");
        path
    }

    #[test]
    fn valid_minimal_config_parses() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        assert_eq!(config.site.id, "demo");
    }

    #[test]
    fn missing_required_field_fails() {
        let path = write_temp(
            "site:\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(err.to_string().contains("site.id"));
    }

    #[test]
    fn rss_enabled_without_required_fields_fails() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nrss:\n  enabled: true\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(err.to_string().contains("rss.max_items"));
    }

    #[test]
    fn invalid_ttl_days_fails() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nrss:\n  enabled: true\n  max_items: 10\n  ttl_days: 0\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(err.to_string().contains("rss.ttl_days"));
    }

    #[test]
    fn people_default_falls_back_to_first_entry() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\npeople:\n  entries:\n    alice:\n      name: \"Alice\"\n    bob:\n      name: \"Bob\"\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let people = config.people.expect("people should be present");
        assert_eq!(people.default, "alice");
    }

    #[test]
    fn blog_defaults_apply() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog: {}\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let blog = config.blog.expect("blog should be present");
        assert!(blog.abstract_cfg.enabled);
        assert_eq!(blog.abstract_cfg.max_chars, 200);
        assert!(!blog.pagination.enabled);
        assert_eq!(blog.pagination.page_size, 10);
        assert_eq!(blog.series.latest_parts, 3);
    }

    #[test]
    fn blog_pagination_requires_page_size_when_enabled() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  pagination:\n    enabled: true\n    page_size: 0\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("blog.pagination.page_size must be > 0")
        );
    }

    #[test]
    fn blog_page_size_enables_pagination_for_legacy_config() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  page_size: 5\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let blog = config.blog.expect("blog should be present");
        assert!(blog.pagination.enabled);
        assert_eq!(blog.pagination.page_size, 5);
    }

    #[test]
    fn blog_abstract_requires_max_chars_when_enabled() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  abstract:\n    enabled: true\n    max_chars: 0\n",
        );
        let err = load_site_config(&path).expect_err("expected error");
        assert!(
            err.to_string()
                .contains("blog.abstract.max_chars must be > 0")
        );
    }

    #[test]
    fn blog_abstract_can_be_disabled() {
        let path = write_temp(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\nblog:\n  abstract:\n    enabled: false\n    max_chars: 10\n",
        );
        let config = load_site_config(&path).expect("config should load");
        let blog = config.blog.expect("blog should be present");
        assert!(!blog.abstract_cfg.enabled);
        assert_eq!(blog.abstract_cfg.max_chars, 10);
    }
}
