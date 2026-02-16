use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_yaml::Value;
use stbl_core::config::load_site_config;
use stbl_core::model::SiteConfig;

fn warn_legacy_theme_variant(config_path: &Path) -> Result<()> {
    let raw = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let doc: Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse YAML config {}", config_path.display()))?;
    let legacy_default = doc
        .as_mapping()
        .and_then(|root| root.get(&Value::String("theme".to_string())))
        .and_then(Value::as_mapping)
        .and_then(|theme| theme.get(&Value::String("variant".to_string())))
        .and_then(Value::as_str)
        .map(|value| value.trim() == "default")
        .unwrap_or(false);
    if legacy_default {
        eprintln!("warning: theme.variant \"default\" is deprecated; using \"stbl\"");
    }
    Ok(())
}

pub fn load_config(root: &Path) -> Result<SiteConfig> {
    let config_path = root.join("stbl.yaml");
    if !config_path.exists() {
        bail!(
            "Missing stbl.yaml in {}. Run `stbl_cli upgrade --source-dir {}` to generate it from stbl.conf (if present).",
            root.display(),
            root.display()
        );
    }
    warn_legacy_theme_variant(&config_path)?;
    load_site_config(&config_path)
}

pub fn load_config_for_build(root: &Path) -> Result<SiteConfig> {
    load_config(root)
}
