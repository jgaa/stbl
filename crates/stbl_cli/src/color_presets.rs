use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use stbl_core::model::{ThemeColorOverrides, ThemeNavOverrides, ThemeWideBackgroundOverrides};

#[derive(Debug, Clone, Deserialize)]
pub struct ColorPreset {
    pub variant: Option<String>,
    #[serde(default)]
    pub colors: ThemeColorOverrides,
    #[serde(default)]
    pub nav: ThemeNavOverrides,
    pub wide_background: Option<ThemeWideBackgroundOverrides>,
}

pub fn load_color_presets() -> Result<BTreeMap<String, ColorPreset>> {
    let yaml = include_str!("../../stbl_embedded_assets/assets/color-presets.yaml");
    let presets: BTreeMap<String, ColorPreset> =
        serde_yaml::from_str(yaml).context("failed to parse color-presets.yaml")?;
    Ok(presets)
}

pub fn validate_preset(name: &str, preset: &ColorPreset) -> Result<()> {
    fn require(value: &Option<String>, name: &str, field: &str) -> Result<()> {
        if value.as_ref().is_some_and(|v| !v.trim().is_empty()) {
            Ok(())
        } else {
            bail!("preset '{name}' is missing colors.{field}");
        }
    }

    require(&preset.colors.bg, name, "bg")?;
    require(&preset.colors.fg, name, "fg")?;
    require(&preset.colors.heading, name, "heading")?;
    require(&preset.colors.accent, name, "accent")?;
    require(&preset.colors.link, name, "link")?;
    require(&preset.colors.muted, name, "muted")?;
    require(&preset.colors.surface, name, "surface")?;
    require(&preset.colors.border, name, "border")?;
    require(&preset.colors.link_hover, name, "link_hover")?;
    require(&preset.colors.code_bg, name, "code_bg")?;
    require(&preset.colors.code_fg, name, "code_fg")?;
    require(&preset.colors.quote_bg, name, "quote_bg")?;
    require(&preset.colors.quote_border, name, "quote_border")?;
    require(&preset.colors.wide_bg, name, "wide_bg")?;

    require(&preset.nav.bg, name, "nav.bg")?;
    require(&preset.nav.fg, name, "nav.fg")?;
    require(&preset.nav.border, name, "nav.border")?;

    if let Some(wide) = preset.wide_background.as_ref() {
        if wide
            .color
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            bail!("preset '{name}' is missing wide_background.color");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_color_presets, validate_preset};

    #[test]
    fn presets_are_complete() {
        let presets = load_color_presets().expect("load presets");
        for (name, preset) in presets {
            validate_preset(&name, &preset).expect("preset should be complete");
        }
    }
}
