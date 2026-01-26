use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::model::{
    BannerConfig, MenuItem, PeopleConfig, PersonEntry, PublishConfig, RssConfig, SeoConfig,
    SiteConfig, SiteMeta, SystemConfig,
};

#[derive(Debug, Deserialize)]
struct SiteConfigRaw {
    site: SiteMetaRaw,
    banner: Option<BannerConfig>,
    #[serde(default)]
    menu: Vec<MenuItem>,
    people: Option<PeopleConfigRaw>,
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
}

#[derive(Debug, Deserialize)]
struct PeopleConfigRaw {
    default: Option<String>,
    entries: Option<BTreeMap<String, PersonEntry>>,
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
                        bail!(
                            "people.default '{}' is missing from people.entries",
                            value
                        );
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
}
