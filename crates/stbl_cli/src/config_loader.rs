use std::path::Path;

use anyhow::{Result, bail};
use stbl_core::config::load_site_config;
use stbl_core::model::SiteConfig;

pub fn load_config_for_build(root: &Path) -> Result<SiteConfig> {
    let config_path = root.join("stbl.yaml");
    if !config_path.exists() {
        bail!(
            "Missing stbl.yaml in {}. Run `stbl_cli upgrade --source-dir {}` to generate it from stbl.conf (if present).",
            root.display(),
            root.display()
        );
    }
    load_site_config(&config_path)
}
